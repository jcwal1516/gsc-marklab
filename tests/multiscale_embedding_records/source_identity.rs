use super::support::*;

#[test]
fn source_identity_chain_is_exact_private_and_digest_bound() {
    let fixture = fixture();
    assert_eq!(fixture.source_entities.entries()[0].source_entity_row(), 2);
    assert_eq!(
        fixture.source_entities.logical_digest(),
        reference_source_digest(&fixture.source_entities)
    );
    assert_eq!(
        fixture.source_entities.logical_digest().to_string(),
        "1b7ef7d5899d89b17e2309455dd4c07df4c7211924f53b6036552e7d3e6e76e2"
    );
    assert_eq!(
        fixture.identity_map.logical_digest(),
        reference_identity_digest(&fixture.identity_map)
    );
    assert_eq!(
        fixture.identity_map.logical_digest().to_string(),
        "cea2e3570e1d10cfd1a7de08110eca3c6d5c210164c5b6022e4b5568971638e6"
    );

    let link = row_link(&fixture);
    assert_eq!(link.entries().len(), 4);
    assert_eq!(link.entries()[0].source_entity_row(), 0);
    assert_eq!(link.entries()[0].source_vector_row(), Some(1));
    assert_eq!(link.entries()[1].status(), EmbeddingStatus::MissingVector);
    assert_eq!(link.entries()[2].status(), EmbeddingStatus::QcRejected);
    assert_eq!(
        link.entries()[3].status(),
        EmbeddingStatus::ExtractionFailed
    );
    assert_eq!(link.logical_digest(), reference_row_link_digest(&link));
    assert_eq!(
        link.logical_digest().to_string(),
        "37ef6645c0a1ab664be6182eb00b9953dcdc79890ee682014427fae60920da76"
    );
    assert_eq!(link.direct_dependencies().len(), 5);
    assert!(link
        .direct_dependencies()
        .windows(2)
        .all(|pair| pair[0] < pair[1]));
    assert!(fixture
        .identity_map
        .direct_dependencies()
        .windows(2)
        .all(|pair| pair[0] < pair[1]));
    assert!(!format!("{:?}", fixture.source_entities.entries()[0]).contains("source-a"));
    assert!(!format!("{:?}", fixture.identity_map.entries()[0]).contains("source-a"));
    assert!(!format!("{:?}", link.entries()[0]).contains("patch-a"));
}

#[test]
fn source_identity_chain_rejects_domain_range_row_status_and_alias_drift() {
    assert!(PatchSourceEntitySet::new("-invalid", Vec::new(), BUDGET).is_err());
    assert!(PatchSourceEntitySet::new("x".repeat(129), Vec::new(), BUDGET).is_err());
    assert!(PatchSourceEntityEntry::new(" source-a", 0).is_err());
    assert!(PatchSourceEntityEntry::new("source-a\n", 0).is_err());
    assert!(PatchSourceEntityEntry::new("x".repeat(256), 0).is_err());
    assert!(PatchSourceEntitySet::new(
        "synthetic.v1",
        vec![
            PatchSourceEntityEntry::new("source-b", 0).expect("b"),
            PatchSourceEntityEntry::new("source-a", 1).expect("a"),
        ],
        BUDGET,
    )
    .is_err());
    assert!(PatchSourceEntitySet::new(
        "synthetic.v1",
        vec![
            PatchSourceEntityEntry::new("source-a", 0).expect("a"),
            PatchSourceEntityEntry::new("source-b", 2).expect("b"),
        ],
        BUDGET,
    )
    .is_err());

    let fixture = fixture();
    let invalid_maps = [
        vec![
            PatchIdentityMapEntry::new("source-z", patch("patch-a")).expect("z"),
            PatchIdentityMapEntry::new("source-b", patch("patch-b")).expect("b"),
            PatchIdentityMapEntry::new("source-c", patch("patch-c")).expect("c"),
            PatchIdentityMapEntry::new("source-d", patch("patch-d")).expect("d"),
        ],
        vec![
            PatchIdentityMapEntry::new("source-a", patch("patch-a")).expect("a"),
            PatchIdentityMapEntry::new("source-b", patch("patch-a")).expect("b"),
            PatchIdentityMapEntry::new("source-c", patch("patch-c")).expect("c"),
            PatchIdentityMapEntry::new("source-d", patch("patch-d")).expect("d"),
        ],
    ];
    for entries in invalid_maps {
        assert!(PatchIdentityMap::new(
            &fixture.source_entities,
            artifact(b"source-entities"),
            &fixture.expected,
            artifact(b"expected-patches"),
            entries,
            BUDGET,
        )
        .is_err());
    }
    assert!(PatchIdentityMap::new(
        &fixture.source_entities,
        artifact(b"same"),
        &fixture.expected,
        artifact(b"same"),
        fixture.identity_map.entries().to_vec(),
        BUDGET,
    )
    .is_err());

    assert!(PatchEmbeddingSourceRowLinkEntry::new(
        patch("patch-a"),
        EmbeddingStatus::MissingVector,
        0,
        Some(0),
    )
    .is_err());
    let invalid_links = [
        vec![
            PatchEmbeddingSourceRowLinkEntry::present(patch("patch-a"), 1, 0),
            PatchEmbeddingSourceRowLinkEntry::missing_vector(patch("patch-b"), 0),
            PatchEmbeddingSourceRowLinkEntry::qc_rejected(patch("patch-c"), 2, 1),
            PatchEmbeddingSourceRowLinkEntry::extraction_failed(patch("patch-d"), 3),
        ],
        vec![
            PatchEmbeddingSourceRowLinkEntry::present(patch("patch-a"), 0, 1),
            PatchEmbeddingSourceRowLinkEntry::missing_vector(patch("patch-b"), 1),
            PatchEmbeddingSourceRowLinkEntry::qc_rejected(patch("patch-c"), 2, 2),
            PatchEmbeddingSourceRowLinkEntry::extraction_failed(patch("patch-d"), 3),
        ],
        vec![
            PatchEmbeddingSourceRowLinkEntry::present(patch("patch-b"), 1, 1),
            PatchEmbeddingSourceRowLinkEntry::missing_vector(patch("patch-a"), 0),
            PatchEmbeddingSourceRowLinkEntry::qc_rejected(patch("patch-c"), 2, 0),
            PatchEmbeddingSourceRowLinkEntry::extraction_failed(patch("patch-d"), 3),
        ],
    ];
    for entries in invalid_links {
        assert!(PatchEmbeddingSourceRowLink::new(
            &fixture.source_entities,
            artifact(b"source-entities"),
            artifact(b"source-vectors"),
            &fixture.expected,
            artifact(b"expected-patches"),
            &fixture.identity_map,
            artifact(b"identity-map"),
            artifact(b"converter"),
            entries,
            BUDGET,
        )
        .is_err());
    }
    assert!(PatchEmbeddingSourceRowLink::new(
        &fixture.source_entities,
        artifact(b"source-entities"),
        artifact(b"same-role"),
        &fixture.expected,
        artifact(b"expected-patches"),
        &fixture.identity_map,
        artifact(b"identity-map"),
        artifact(b"same-role"),
        link_input_for_alias(),
        BUDGET,
    )
    .is_err());
}

fn link_input_for_alias() -> Vec<PatchEmbeddingSourceRowLinkEntry> {
    vec![
        PatchEmbeddingSourceRowLinkEntry::present(patch("patch-a"), 0, 1),
        PatchEmbeddingSourceRowLinkEntry::missing_vector(patch("patch-b"), 1),
        PatchEmbeddingSourceRowLinkEntry::qc_rejected(patch("patch-c"), 2, 0),
        PatchEmbeddingSourceRowLinkEntry::extraction_failed(patch("patch-d"), 3),
    ]
}
