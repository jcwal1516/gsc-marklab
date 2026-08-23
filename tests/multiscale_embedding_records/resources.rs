use super::support::*;

fn source_input() -> (String, Vec<PatchSourceEntityEntry>) {
    let mut profile = String::with_capacity(64);
    profile.push_str("resource_profile.v1");
    let mut entries = Vec::with_capacity(8);
    entries.extend([
        PatchSourceEntityEntry::new("source-a", 1).expect("source a"),
        PatchSourceEntityEntry::new("source-b", 0).expect("source b"),
    ]);
    (profile, entries)
}

fn source_required(profile: &String, entries: &Vec<PatchSourceEntityEntry>) -> usize {
    size_of::<PatchSourceEntitySet>()
        + profile.capacity()
        + entries.capacity() * size_of::<PatchSourceEntityEntry>()
        + entries
            .iter()
            .map(|entry| entry.source_patch_id().len())
            .sum::<usize>()
}

fn map_input() -> Vec<PatchIdentityMapEntry> {
    let mut entries = Vec::with_capacity(8);
    entries.extend([
        PatchIdentityMapEntry::new("source-a", patch("patch-c")).expect("map a"),
        PatchIdentityMapEntry::new("source-b", patch("patch-a")).expect("map b"),
        PatchIdentityMapEntry::new("source-c", patch("patch-d")).expect("map c"),
        PatchIdentityMapEntry::new("source-d", patch("patch-b")).expect("map d"),
    ]);
    entries
}

fn map_required(entries: &Vec<PatchIdentityMapEntry>) -> usize {
    size_of::<PatchIdentityMap>()
        + entries.capacity() * size_of::<PatchIdentityMapEntry>()
        + entries
            .iter()
            .map(|entry| entry.source_patch_id().len() + entry.patch_id().as_str().len())
            .sum::<usize>()
        + entries.len() * size_of::<u64>()
}

fn link_input() -> Vec<PatchEmbeddingSourceRowLinkEntry> {
    let mut entries = Vec::with_capacity(8);
    entries.extend([
        PatchEmbeddingSourceRowLinkEntry::present(patch("patch-a"), 0, 1),
        PatchEmbeddingSourceRowLinkEntry::missing_vector(patch("patch-b"), 1),
        PatchEmbeddingSourceRowLinkEntry::qc_rejected(patch("patch-c"), 2, 0),
        PatchEmbeddingSourceRowLinkEntry::extraction_failed(patch("patch-d"), 3),
    ]);
    entries
}

fn link_required(entries: &Vec<PatchEmbeddingSourceRowLinkEntry>) -> usize {
    size_of::<PatchEmbeddingSourceRowLink>()
        + entries.capacity() * size_of::<PatchEmbeddingSourceRowLinkEntry>()
        + entries
            .iter()
            .map(|entry| entry.patch_id().as_str().len())
            .sum::<usize>()
}

#[test]
fn source_chain_constructors_charge_overcapacity_and_exact_retained_storage() {
    let (profile, entries) = source_input();
    let required = source_required(&profile, &entries);
    PatchSourceEntitySet::new(profile, entries, required).expect("exact source budget");
    let (profile, entries) = source_input();
    assert!(matches!(
        PatchSourceEntitySet::new(profile, entries, required - 1),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: observed,
            maximum,
        }) if observed == required && maximum == required - 1
    ));

    let fixture = fixture();
    let entries = map_input();
    let required = map_required(&entries);
    PatchIdentityMap::new(
        &fixture.source_entities,
        artifact(b"source-entities"),
        &fixture.expected,
        artifact(b"expected-patches"),
        entries,
        required,
    )
    .expect("exact map budget");
    assert!(matches!(
        PatchIdentityMap::new(
            &fixture.source_entities,
            artifact(b"source-entities"),
            &fixture.expected,
            artifact(b"expected-patches"),
            map_input(),
            required - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: observed,
            maximum,
        }) if observed == required && maximum == required - 1
    ));

    let entries = link_input();
    let required = link_required(&entries);
    PatchEmbeddingSourceRowLink::new(
        &fixture.source_entities,
        artifact(b"source-entities"),
        artifact(b"source-vectors"),
        &fixture.expected,
        artifact(b"expected-patches"),
        &fixture.identity_map,
        artifact(b"identity-map"),
        artifact(b"converter"),
        entries,
        required,
    )
    .expect("exact link budget");
    assert!(matches!(
        PatchEmbeddingSourceRowLink::new(
            &fixture.source_entities,
            artifact(b"source-entities"),
            artifact(b"source-vectors"),
            &fixture.expected,
            artifact(b"expected-patches"),
            &fixture.identity_map,
            artifact(b"identity-map"),
            artifact(b"converter"),
            link_input(),
            required - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: observed,
            maximum,
        }) if observed == required && maximum == required - 1
    ));

    let normalization = normalization();
    let required = size_of::<PatchEmbeddingInputNormalization>()
        + normalization
            .channel_mean()
            .iter()
            .chain(normalization.channel_std())
            .map(|value| value.as_str().len())
            .sum::<usize>();
    let decimal = |value| PatchNormalizationDecimal::new(value).expect("decimal");
    PatchEmbeddingInputNormalization::he_srgb(
        [decimal("0.485"), decimal("0.456"), decimal("0.406")],
        [decimal("0.229"), decimal("0.224"), decimal("0.225")],
        required,
    )
    .expect("exact normalization budget");
    assert!(matches!(
        PatchEmbeddingInputNormalization::he_srgb(
            [decimal("0.485"), decimal("0.456"), decimal("0.406")],
            [decimal("0.229"), decimal("0.224"), decimal("0.225")],
            required - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: observed,
            maximum,
        }) if observed == required && maximum == required - 1
    ));
}

fn decoded_required(error: MultiscaleEmbeddingError) -> usize {
    match error {
        MultiscaleEmbeddingError::DecodedByteBudgetExceeded {
            required,
            maximum: 0,
        } => required,
        unexpected => panic!("expected decoded-byte boundary, observed {unexpected:?}"),
    }
}

#[test]
fn source_chain_decoders_enforce_exact_encoded_decoded_and_retained_boundaries() {
    let fixture = fixture();
    let source_bytes = fixture
        .source_entities
        .to_canonical_json()
        .expect("source JSON");
    let decoded = decoded_required(
        PatchSourceEntitySet::from_canonical_json(&source_bytes, BUDGET, 0, BUDGET)
            .expect_err("zero decoded budget"),
    );
    let retained = size_of::<PatchSourceEntitySet>()
        + fixture.source_entities.source_profile().len()
        + size_of_val(fixture.source_entities.entries())
        + fixture
            .source_entities
            .entries()
            .iter()
            .map(|entry| entry.source_patch_id().len())
            .sum::<usize>();
    assert!(matches!(
        PatchSourceEntitySet::from_canonical_json(
            &source_bytes,
            BUDGET,
            decoded,
            retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required == retained && maximum == retained - 1
    ));
    assert!(matches!(
        PatchSourceEntitySet::from_canonical_json(
            &source_bytes,
            BUDGET,
            decoded - 1,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded {
            required,
            maximum,
        }) if required == decoded && maximum == decoded - 1
    ));
    PatchSourceEntitySet::from_canonical_json(&source_bytes, source_bytes.len(), decoded, BUDGET)
        .expect("exact source decode budgets");

    let map_bytes = fixture.identity_map.to_canonical_json().expect("map JSON");
    let decoded = decoded_required(
        PatchIdentityMap::from_canonical_json(
            &map_bytes,
            &fixture.source_entities,
            &fixture.expected,
            BUDGET,
            0,
            BUDGET,
        )
        .expect_err("zero map decoded budget"),
    );
    let retained = size_of::<PatchIdentityMap>()
        + size_of_val(fixture.identity_map.entries())
        + fixture
            .identity_map
            .entries()
            .iter()
            .map(|entry| entry.source_patch_id().len() + entry.patch_id().as_str().len())
            .sum::<usize>()
        + fixture.identity_map.entries().len() * size_of::<u64>();
    assert!(matches!(
        PatchIdentityMap::from_canonical_json(
            &map_bytes,
            &fixture.source_entities,
            &fixture.expected,
            BUDGET,
            decoded,
            retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required == retained && maximum == retained - 1
    ));
    PatchIdentityMap::from_canonical_json(
        &map_bytes,
        &fixture.source_entities,
        &fixture.expected,
        map_bytes.len(),
        decoded,
        BUDGET,
    )
    .expect("exact map decode budgets");

    let link = row_link(&fixture);
    let link_bytes = link.to_canonical_json().expect("link JSON");
    let decoded = decoded_required(
        PatchEmbeddingSourceRowLink::from_canonical_json(
            &link_bytes,
            &fixture.source_entities,
            &fixture.expected,
            &fixture.identity_map,
            BUDGET,
            0,
            BUDGET,
        )
        .expect_err("zero link decoded budget"),
    );
    let retained = size_of::<PatchEmbeddingSourceRowLink>()
        + size_of_val(link.entries())
        + link
            .entries()
            .iter()
            .map(|entry| entry.patch_id().as_str().len())
            .sum::<usize>();
    assert!(matches!(
        PatchEmbeddingSourceRowLink::from_canonical_json(
            &link_bytes,
            &fixture.source_entities,
            &fixture.expected,
            &fixture.identity_map,
            BUDGET,
            decoded,
            retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required == retained && maximum == retained - 1
    ));
    PatchEmbeddingSourceRowLink::from_canonical_json(
        &link_bytes,
        &fixture.source_entities,
        &fixture.expected,
        &fixture.identity_map,
        link_bytes.len(),
        decoded,
        BUDGET,
    )
    .expect("exact link decode budgets");

    let normalization = normalization();
    let bytes = normalization
        .to_canonical_json()
        .expect("normalization JSON");
    let decoded = decoded_required(
        PatchEmbeddingInputNormalization::from_canonical_json(&bytes, BUDGET, 0, BUDGET)
            .expect_err("zero normalization decoded budget"),
    );
    let retained = size_of::<PatchEmbeddingInputNormalization>()
        + normalization
            .channel_mean()
            .iter()
            .chain(normalization.channel_std())
            .map(|value| value.as_str().len())
            .sum::<usize>();
    assert!(matches!(
        PatchEmbeddingInputNormalization::from_canonical_json(
            &bytes,
            BUDGET,
            decoded,
            retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required == retained && maximum == retained - 1
    ));
    PatchEmbeddingInputNormalization::from_canonical_json(&bytes, bytes.len(), decoded, BUDGET)
        .expect("exact normalization decode budgets");
}

#[test]
fn empty_source_chain_is_valid_and_round_trips_without_dummy_rows() {
    let patient = HierarchyId::from(PatientId::new("empty-records-patient").expect("patient"));
    let slide = SlideId::new("empty-records-slide").expect("slide");
    let hierarchy = CohortHierarchy::new(
        vec![
            HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                HierarchyId::from(slide.clone()),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient,
                },
            ),
        ],
        Vec::new(),
    )
    .expect("empty hierarchy");
    let expected = ExpectedPatchSet::new(&hierarchy, slide, "no_patches.v1", Vec::new(), BUDGET)
        .expect("empty expected patches");
    let source = PatchSourceEntitySet::new("empty_profile.v1", Vec::new(), BUDGET)
        .expect("empty source entities");
    let map = PatchIdentityMap::new(
        &source,
        artifact(b"empty-source"),
        &expected,
        artifact(b"empty-expected"),
        Vec::new(),
        BUDGET,
    )
    .expect("empty identity map");
    let link = PatchEmbeddingSourceRowLink::new(
        &source,
        artifact(b"empty-source"),
        artifact(b"empty-vectors"),
        &expected,
        artifact(b"empty-expected"),
        &map,
        artifact(b"empty-map"),
        artifact(b"empty-converter"),
        Vec::new(),
        BUDGET,
    )
    .expect("empty source-row link");
    assert!(source.entries().is_empty());
    assert!(map.entries().is_empty());
    assert!(link.entries().is_empty());
    assert_eq!(
        PatchSourceEntitySet::from_canonical_json(
            &source.to_canonical_json().expect("source JSON"),
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .expect("empty source round trip"),
        source
    );
}
