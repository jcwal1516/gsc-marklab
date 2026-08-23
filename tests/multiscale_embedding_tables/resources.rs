use super::support::*;

#[test]
fn strict_budgets_and_invalid_expected_or_table_rows_are_rejected() {
    let (hierarchy, slide) = hierarchy();
    let expected_required = match ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        vec![patch("patch-a"), patch("patch-b")],
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("expected retained-budget error, observed {other:?}"),
    };
    let expected_oracle = size_of::<ExpectedPatchSet>()
        + "slide".len()
        + "all_patches.v1".len()
        + 2 * size_of::<PatchId>()
        + "patch-a".len()
        + "patch-b".len();
    assert_eq!(expected_required, expected_oracle);
    assert!(ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        vec![patch("patch-a"), patch("patch-b")],
        expected_required - 1,
    )
    .is_err());
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        vec![patch("patch-a"), patch("patch-b")],
        expected_required,
    )
    .expect("exact expected-set budget");
    let mut roomy_rule = String::with_capacity(RETAINED_BUDGET * 4);
    roomy_rule.push('r');
    let normalized_rule = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        roomy_rule,
        Vec::new(),
        size_of::<ExpectedPatchSet>() + slide.as_str().len() + 1,
    )
    .expect("selection-rule capacity is normalized to retained length");
    assert_eq!(normalized_rule.selection_rule(), "r");
    let encoded = expected
        .to_canonical_json()
        .expect("canonical expected set");
    let decoded_required = match ExpectedPatchSet::from_canonical_json(
        &encoded,
        &hierarchy,
        encoded.len(),
        0,
        RETAINED_BUDGET,
    ) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
        other => panic!("expected decoded-budget error, observed {other:?}"),
    };
    assert!(ExpectedPatchSet::from_canonical_json(
        &encoded,
        &hierarchy,
        encoded.len(),
        decoded_required - 1,
        RETAINED_BUDGET,
    )
    .is_err());
    assert_eq!(
        ExpectedPatchSet::from_canonical_json(
            &encoded,
            &hierarchy,
            encoded.len(),
            decoded_required,
            expected_required,
        )
        .expect("exact decoded and retained budgets"),
        expected
    );
    let with_leading_space = [b" ".as_slice(), encoded.as_slice()].concat();
    assert!(ExpectedPatchSet::from_canonical_json(
        &with_leading_space,
        &hierarchy,
        with_leading_space.len(),
        RETAINED_BUDGET,
        RETAINED_BUDGET,
    )
    .is_err());
    let with_unknown_field = String::from_utf8(encoded.clone())
        .expect("UTF-8 JSON")
        .replace("\"ids\":", "\"unknown\":0,\"ids\":");
    assert!(ExpectedPatchSet::from_canonical_json(
        with_unknown_field.as_bytes(),
        &hierarchy,
        with_unknown_field.len(),
        RETAINED_BUDGET,
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(ExpectedPatchSet::new(
        &hierarchy,
        SlideId::new("absent-slide").expect("absent slide ID"),
        "all_patches.v1",
        vec![patch("patch-a"), patch("patch-b")],
        RETAINED_BUDGET,
    )
    .is_err());

    let expected_artifact = artifact_id(b"expected-patches-budget");
    let support_artifact = artifact_id(b"patch-support-budget");
    let provenance_artifact = artifact_id(b"patch-provenance-budget");
    let support_digest = ContentDigest::from_bytes(b"support-logical-budget");
    let provenance_digest = ContentDigest::from_bytes(b"provenance-logical-budget");
    let rows = || {
        vec![
            PatchEmbeddingRow::non_present(patch("patch-a"), EmbeddingStatus::MissingVector)
                .expect("missing row"),
            PatchEmbeddingRow::non_present(patch("patch-b"), EmbeddingStatus::QcRejected)
                .expect("rejected row"),
        ]
    };
    let budget_rows = rows();
    let table_oracle = size_of::<PatchEmbeddingTable>()
        + slide.as_str().len()
        + 2 * 2 * size_of::<f32>()
        + 2 * (size_of::<PatchId>() + size_of::<EmbeddingStatus>())
        + budget_rows.capacity() * size_of::<PatchEmbeddingRow>()
        + "patch-a".len()
        + "patch-b".len();
    let table_required = match PatchEmbeddingTable::from_rows(
        2,
        &expected,
        expected_artifact,
        support_artifact,
        support_digest,
        provenance_artifact,
        provenance_digest,
        budget_rows,
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("expected table budget error, observed {other:?}"),
    };
    assert_eq!(table_required, table_oracle);
    assert!(PatchEmbeddingTable::from_rows(
        2,
        &expected,
        expected_artifact,
        support_artifact,
        support_digest,
        provenance_artifact,
        provenance_digest,
        rows(),
        table_required - 1,
    )
    .is_err());
    let exact = PatchEmbeddingTable::from_rows(
        2,
        &expected,
        expected_artifact,
        support_artifact,
        support_digest,
        provenance_artifact,
        provenance_digest,
        rows(),
        table_required,
    )
    .expect("exact table budget");
    assert_eq!(exact.expected_entities_artifact_id(), expected_artifact);
    assert_eq!(exact.support_artifact_id(), support_artifact);
    assert_eq!(exact.provenance_artifact_id(), provenance_artifact);
    assert!(PatchEmbeddingTable::from_rows(
        65_537,
        &expected,
        expected_artifact,
        support_artifact,
        support_digest,
        provenance_artifact,
        provenance_digest,
        rows(),
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(PatchEmbeddingTable::from_rows(
        2,
        &expected,
        expected_artifact,
        support_artifact,
        support_digest,
        provenance_artifact,
        provenance_digest,
        vec![
            PatchEmbeddingRow::present(patch("patch-b"), vec![1.0, 2.0]),
            PatchEmbeddingRow::present(patch("patch-a"), vec![1.0, 2.0]),
        ],
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(PatchEmbeddingTable::from_rows(
        2,
        &expected,
        expected_artifact,
        support_artifact,
        support_digest,
        provenance_artifact,
        provenance_digest,
        vec![
            PatchEmbeddingRow::present(patch("patch-a"), vec![f32::NAN, 2.0]),
            PatchEmbeddingRow::non_present(patch("patch-b"), EmbeddingStatus::ExtractionFailed,)
                .expect("failed row"),
        ],
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(PatchEmbeddingRow::non_present(patch("patch-a"), EmbeddingStatus::Present).is_err());

    let empty_expected = ExpectedPatchSet::new(
        &hierarchy,
        slide,
        "no_patches.v1",
        Vec::new(),
        RETAINED_BUDGET,
    )
    .expect("empty expected patches");
    let empty = PatchEmbeddingTable::from_rows(
        2,
        &empty_expected,
        artifact_id(b"empty-expected"),
        artifact_id(b"empty-support"),
        ContentDigest::from_bytes(b"empty-support-logical"),
        artifact_id(b"empty-provenance"),
        ContentDigest::from_bytes(b"empty-provenance-logical"),
        Vec::new(),
        RETAINED_BUDGET,
    )
    .expect("empty patch table");
    assert_eq!(empty.row_count(), 0);
    assert_eq!(empty.qc_summary().row_count(), 0);
}
