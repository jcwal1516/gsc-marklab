use std::str::FromStr;

use marklab::{
    ArtifactId, CohortHierarchy, ContentDigest, EmbeddingStatus, ExpectedPatchSet,
    ExpectedRegionSet, ExpectedSlideSet, HierarchyId, HierarchyNode, PatchEmbeddingRow,
    PatchEmbeddingTable, PatchId, PatientId, RegionEmbeddingRow, RegionEmbeddingTable, RegionId,
    ReplicationRole, SlideEmbeddingRow, SlideEmbeddingTable, SlideId,
};

pub const LARGE: usize = 256 * 1024 * 1024;

pub struct MatrixFixture {
    pub expected_patches: ExpectedPatchSet,
    pub expected_regions: ExpectedRegionSet,
    pub expected_slides: ExpectedSlideSet,
    pub patch_table: PatchEmbeddingTable,
    pub region_table: RegionEmbeddingTable,
    pub slide_table: SlideEmbeddingTable,
}

pub fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn patch(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

fn region(value: &str) -> RegionId {
    RegionId::new(value).expect("region ID")
}

pub fn matrix_fixture() -> MatrixFixture {
    let patient = HierarchyId::from(PatientId::new("matrix-patient").expect("patient ID"));
    let slide = SlideId::new("matrix-slide").expect("slide ID");
    let slide_node = HierarchyId::from(slide.clone());
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
    for id in ["patch-a", "patch-b", "patch-c", "patch-d"] {
        nodes.push(HierarchyNode::new(
            HierarchyId::from(patch(id)),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        ));
    }
    for id in ["region-a", "region-b"] {
        nodes.push(HierarchyNode::new(
            HierarchyId::from(region(id)),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        ));
    }
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("matrix hierarchy");
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "matrix_patches.v1",
        vec![
            patch("patch-a"),
            patch("patch-b"),
            patch("patch-c"),
            patch("patch-d"),
        ],
        LARGE,
    )
    .expect("expected patches");
    let expected_regions = ExpectedRegionSet::new(
        &hierarchy,
        slide.clone(),
        "matrix_regions.v1",
        vec![region("region-a"), region("region-b")],
        LARGE,
    )
    .expect("expected regions");
    let expected_slides = ExpectedSlideSet::new(
        &hierarchy,
        slide.clone(),
        "matrix_slide.v1",
        vec![slide.clone()],
        LARGE,
    )
    .expect("expected slide");

    let patch_table = PatchEmbeddingTable::from_rows(
        3,
        &expected_patches,
        artifact_id(b"matrix-expected-patches"),
        artifact_id(b"matrix-patch-support"),
        ContentDigest::from_bytes(b"matrix-patch-support-logical"),
        artifact_id(b"matrix-patch-provenance"),
        ContentDigest::from_bytes(b"matrix-patch-provenance-logical"),
        vec![
            PatchEmbeddingRow::present(patch("patch-a"), vec![1.0, -0.0, 3.5]),
            PatchEmbeddingRow::non_present(patch("patch-b"), EmbeddingStatus::MissingVector)
                .expect("missing patch"),
            PatchEmbeddingRow::non_present(patch("patch-c"), EmbeddingStatus::ExtractionFailed)
                .expect("failed patch"),
            PatchEmbeddingRow::non_present(patch("patch-d"), EmbeddingStatus::QcRejected)
                .expect("rejected patch"),
        ],
        LARGE,
    )
    .expect("patch table");
    let region_table = RegionEmbeddingTable::from_rows(
        3,
        &expected_regions,
        artifact_id(b"matrix-expected-regions"),
        artifact_id(b"matrix-region-support"),
        ContentDigest::from_bytes(b"matrix-region-support-logical"),
        artifact_id(b"matrix-region-provenance"),
        ContentDigest::from_bytes(b"matrix-region-provenance-logical"),
        vec![
            RegionEmbeddingRow::present(region("region-a"), vec![0.0, 0.0, 0.0]),
            RegionEmbeddingRow::present(region("region-b"), vec![4.0, 5.0, 6.0]),
        ],
        LARGE,
    )
    .expect("region table");
    let slide_table = SlideEmbeddingTable::from_rows(
        3,
        &expected_slides,
        artifact_id(b"matrix-expected-slide"),
        artifact_id(b"matrix-slide-support"),
        ContentDigest::from_bytes(b"matrix-slide-support-logical"),
        artifact_id(b"matrix-slide-provenance"),
        ContentDigest::from_bytes(b"matrix-slide-provenance-logical"),
        vec![SlideEmbeddingRow::present(slide, vec![7.0, 8.0, 9.0])],
        LARGE,
    )
    .expect("slide table");

    MatrixFixture {
        expected_patches,
        expected_regions,
        expected_slides,
        patch_table,
        region_table,
        slide_table,
    }
}

pub fn patch_table_with_rows(row_count: usize, dimension: u32) -> PatchEmbeddingTable {
    let patient = HierarchyId::from(PatientId::new("scale-patient").expect("patient ID"));
    let slide = SlideId::new("scale-slide").expect("slide ID");
    let slide_node = HierarchyId::from(slide.clone());
    let patch_ids = (0..row_count)
        .map(|index| patch(&format!("scale-patch-{index:08}")))
        .collect::<Vec<_>>();
    let mut nodes = Vec::with_capacity(row_count + 2);
    nodes.push(HierarchyNode::new(
        patient.clone(),
        None,
        ReplicationRole::BiologicalUnit,
    ));
    nodes.push(HierarchyNode::new(
        slide_node.clone(),
        None,
        ReplicationRole::TechnicalReplicate {
            biological_source: patient,
        },
    ));
    nodes.extend(patch_ids.iter().cloned().map(|patch_id| {
        HierarchyNode::new(
            HierarchyId::from(patch_id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("scale hierarchy");
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide,
        "scale_patches.v1",
        patch_ids.clone(),
        LARGE,
    )
    .expect("scale expected patches");
    let dimension_usize = dimension as usize;
    let rows = patch_ids
        .into_iter()
        .enumerate()
        .map(|(index, patch_id)| match index % 4 {
            0 => PatchEmbeddingRow::present(
                patch_id,
                (0..dimension_usize)
                    .map(|column| (index + column) as f32)
                    .collect(),
            ),
            1 => PatchEmbeddingRow::non_present(patch_id, EmbeddingStatus::MissingVector)
                .expect("missing scale patch"),
            2 => PatchEmbeddingRow::non_present(patch_id, EmbeddingStatus::ExtractionFailed)
                .expect("failed scale patch"),
            _ => PatchEmbeddingRow::non_present(patch_id, EmbeddingStatus::QcRejected)
                .expect("rejected scale patch"),
        })
        .collect();
    PatchEmbeddingTable::from_rows(
        dimension,
        &expected,
        artifact_id(b"scale-expected-patches"),
        artifact_id(b"scale-patch-support"),
        ContentDigest::from_bytes(b"scale-patch-support-logical"),
        artifact_id(b"scale-patch-provenance"),
        ContentDigest::from_bytes(b"scale-patch-provenance-logical"),
        rows,
        LARGE,
    )
    .expect("scale patch table")
}
