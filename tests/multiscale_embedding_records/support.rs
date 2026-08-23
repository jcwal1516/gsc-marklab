pub(super) use std::{io::Write, mem::size_of, mem::size_of_val, str::FromStr};

pub(super) use marklab::{
    ArtifactId, CohortHierarchy, ContentDigest, EmbeddingStatus, ExpectedPatchSet, HierarchyId,
    HierarchyNode, MultiscaleEmbeddingError, PatchEmbeddingInputNormalization,
    PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry, PatchId, PatchIdentityMap,
    PatchIdentityMapEntry, PatchNormalizationDecimal, PatchSourceEntityEntry, PatchSourceEntitySet,
    PatientId, ReplicationRole, SlideId,
};

pub(super) const BUDGET: usize = 1 << 20;

pub(super) fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(super) fn patch(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

pub(super) struct Fixture {
    pub(super) expected: ExpectedPatchSet,
    pub(super) source_entities: PatchSourceEntitySet,
    pub(super) identity_map: PatchIdentityMap,
}

pub(super) fn fixture() -> Fixture {
    let patient = HierarchyId::from(PatientId::new("records-patient").expect("patient"));
    let slide = SlideId::new("records-slide").expect("slide");
    let slide_node = HierarchyId::from(slide.clone());
    let patch_ids = ["patch-a", "patch-b", "patch-c", "patch-d"].map(patch);
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide_node.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(patch_ids.iter().cloned().map(|id| {
        HierarchyNode::new(
            HierarchyId::from(id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide,
        "all_source_patches.v1",
        patch_ids.into(),
        BUDGET,
    )
    .expect("expected patches");
    let source_entities = PatchSourceEntitySet::new(
        "synthetic_patch_profile.v1",
        vec![
            PatchSourceEntityEntry::new("source-a", 2).expect("source a"),
            PatchSourceEntityEntry::new("source-b", 0).expect("source b"),
            PatchSourceEntityEntry::new("source-c", 3).expect("source c"),
            PatchSourceEntityEntry::new("source-d", 1).expect("source d"),
        ],
        BUDGET,
    )
    .expect("source entities");
    let identity_map = PatchIdentityMap::new(
        &source_entities,
        artifact(b"source-entities"),
        &expected,
        artifact(b"expected-patches"),
        vec![
            PatchIdentityMapEntry::new("source-a", patch("patch-c")).expect("map a"),
            PatchIdentityMapEntry::new("source-b", patch("patch-a")).expect("map b"),
            PatchIdentityMapEntry::new("source-c", patch("patch-d")).expect("map c"),
            PatchIdentityMapEntry::new("source-d", patch("patch-b")).expect("map d"),
        ],
        BUDGET,
    )
    .expect("identity map");
    Fixture {
        expected,
        source_entities,
        identity_map,
    }
}

pub(super) fn row_link(fixture: &Fixture) -> PatchEmbeddingSourceRowLink {
    PatchEmbeddingSourceRowLink::new(
        &fixture.source_entities,
        artifact(b"source-entities"),
        artifact(b"source-vectors"),
        &fixture.expected,
        artifact(b"expected-patches"),
        &fixture.identity_map,
        artifact(b"identity-map"),
        artifact(b"converter"),
        vec![
            PatchEmbeddingSourceRowLinkEntry::present(patch("patch-a"), 0, 1),
            PatchEmbeddingSourceRowLinkEntry::missing_vector(patch("patch-b"), 1),
            PatchEmbeddingSourceRowLinkEntry::qc_rejected(patch("patch-c"), 2, 0),
            PatchEmbeddingSourceRowLinkEntry::extraction_failed(patch("patch-d"), 3),
        ],
        BUDGET,
    )
    .expect("source row link")
}

pub(super) fn normalization() -> PatchEmbeddingInputNormalization {
    let decimal = |value| PatchNormalizationDecimal::new(value).expect("decimal");
    PatchEmbeddingInputNormalization::he_srgb(
        [decimal("0.485"), decimal("0.456"), decimal("0.406")],
        [decimal("0.229"), decimal("0.224"), decimal("0.225")],
        BUDGET,
    )
    .expect("normalization")
}

fn frame(writer: &mut impl Write, value: &[u8]) {
    writer
        .write_all(&(value.len() as u128).to_be_bytes())
        .expect("reference frame length");
    writer.write_all(value).expect("reference frame value");
}

fn artifact_frame(writer: &mut impl Write, value: ArtifactId) {
    frame(writer, value.digest().as_bytes());
}

pub(super) fn reference_source_digest(source: &PatchSourceEntitySet) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-source-entity-set-logical-v1");
    frame(&mut writer, b"marklab.patch_source_entity_set");
    frame(&mut writer, &1_u32.to_be_bytes());
    frame(&mut writer, source.source_profile().as_bytes());
    frame(
        &mut writer,
        &u64::try_from(source.entries().len())
            .expect("bounded rows")
            .to_be_bytes(),
    );
    for entry in source.entries() {
        frame(&mut writer, entry.source_patch_id().as_bytes());
        frame(&mut writer, &entry.source_entity_row().to_be_bytes());
    }
    writer.finish().0
}

pub(super) fn reference_identity_digest(map: &PatchIdentityMap) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-identity-map-logical-v1");
    frame(&mut writer, b"marklab.patch_identity_map");
    frame(&mut writer, &1_u32.to_be_bytes());
    artifact_frame(&mut writer, map.source_entities_artifact_id());
    frame(&mut writer, map.source_entities_logical_digest().as_bytes());
    artifact_frame(&mut writer, map.expected_patches_artifact_id());
    frame(
        &mut writer,
        map.expected_patches_logical_digest().as_bytes(),
    );
    frame(
        &mut writer,
        &u64::try_from(map.entries().len())
            .expect("bounded rows")
            .to_be_bytes(),
    );
    for entry in map.entries() {
        frame(&mut writer, entry.source_patch_id().as_bytes());
        frame(&mut writer, entry.patch_id().as_str().as_bytes());
    }
    writer.finish().0
}

pub(super) fn reference_row_link_digest(link: &PatchEmbeddingSourceRowLink) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-source-row-link-logical-v1");
    frame(&mut writer, b"marklab.patch_embedding_source_row_link");
    frame(&mut writer, &1_u32.to_be_bytes());
    artifact_frame(&mut writer, link.source_entities_artifact_id());
    frame(
        &mut writer,
        link.source_entities_logical_digest().as_bytes(),
    );
    artifact_frame(&mut writer, link.source_vectors_artifact_id());
    artifact_frame(&mut writer, link.expected_patches_artifact_id());
    frame(
        &mut writer,
        link.expected_patches_logical_digest().as_bytes(),
    );
    artifact_frame(&mut writer, link.identity_map_artifact_id());
    frame(&mut writer, link.identity_map_logical_digest().as_bytes());
    artifact_frame(&mut writer, link.converter_artifact_id());
    frame(
        &mut writer,
        &u64::try_from(link.entries().len())
            .expect("bounded rows")
            .to_be_bytes(),
    );
    for entry in link.entries() {
        frame(&mut writer, entry.patch_id().as_str().as_bytes());
        frame(
            &mut writer,
            match entry.status() {
                EmbeddingStatus::Present => b"present",
                EmbeddingStatus::MissingVector => b"missing_vector",
                EmbeddingStatus::ExtractionFailed => b"extraction_failed",
                EmbeddingStatus::QcRejected => b"qc_rejected",
            },
        );
        frame(&mut writer, &entry.source_entity_row().to_be_bytes());
        match entry.source_vector_row() {
            Some(row) => {
                frame(&mut writer, &[1]);
                frame(&mut writer, &row.to_be_bytes());
            }
            None => frame(&mut writer, &[0]),
        }
    }
    writer.finish().0
}

pub(super) fn reference_normalization_digest(
    normalization: &PatchEmbeddingInputNormalization,
) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-input-normalization-logical-v1");
    frame(&mut writer, b"marklab.patch_embedding_input_normalization");
    frame(&mut writer, &1_u32.to_be_bytes());
    frame(&mut writer, b"he_brightfield");
    frame(&mut writer, b"srgb");
    frame(&mut writer, &3_u64.to_be_bytes());
    for channel in ["r", "g", "b"] {
        frame(&mut writer, channel.as_bytes());
    }
    frame(&mut writer, b"0");
    frame(&mut writer, b"1");
    frame(&mut writer, &3_u64.to_be_bytes());
    for value in normalization.channel_mean() {
        frame(&mut writer, value.as_str().as_bytes());
    }
    frame(&mut writer, &3_u64.to_be_bytes());
    for value in normalization.channel_std() {
        frame(&mut writer, value.as_str().as_bytes());
    }
    frame(&mut writer, b"none");
    writer.finish().0
}
