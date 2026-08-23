use super::support::*;

#[test]
fn typed_tables_keep_status_vectors_and_logical_domains_separate() {
    let (hierarchy, slide) = hierarchy();
    let patches = expected_patches(&hierarchy, &slide);
    let regions = ExpectedRegionSet::new(
        &hierarchy,
        slide.clone(),
        "all_regions.v1",
        vec![region("region-a")],
        RETAINED_BUDGET,
    )
    .expect("expected regions");
    let slides = ExpectedSlideSet::new(
        &hierarchy,
        slide.clone(),
        "owning_slide.v1",
        vec![slide.clone()],
        RETAINED_BUDGET,
    )
    .expect("expected slide");

    let expected_patch_artifact = artifact_id(b"expected-patches");
    let expected_region_artifact = artifact_id(b"expected-regions");
    let expected_slide_artifact = artifact_id(b"expected-slide");
    let patch_support = artifact_id(b"patch-support");
    let region_support = artifact_id(b"region-support");
    let slide_support = artifact_id(b"slide-support");
    let patch_provenance = artifact_id(b"patch-provenance");
    let region_provenance = artifact_id(b"region-provenance");
    let slide_provenance = artifact_id(b"slide-provenance");

    let patch_table = PatchEmbeddingTable::from_rows(
        3,
        &patches,
        expected_patch_artifact,
        patch_support,
        ContentDigest::from_bytes(b"patch-support-logical"),
        patch_provenance,
        ContentDigest::from_bytes(b"patch-provenance-logical"),
        vec![
            PatchEmbeddingRow::present(patch("patch-a"), vec![1.0, -0.0, 3.0]),
            PatchEmbeddingRow::non_present(patch("patch-b"), EmbeddingStatus::MissingVector)
                .expect("missing patch row"),
        ],
        RETAINED_BUDGET,
    )
    .expect("patch table");
    assert_eq!(patch_table.entity_kind(), EmbeddingEntityKind::Patch);
    assert_eq!(patch_table.owning_slide_id(), &slide);
    assert_eq!(patch_table.row_count(), 2);
    assert_eq!(patch_table.dimension(), 3);
    assert_eq!(
        patch_table.row(0).expect("present patch").vector(),
        Some(&[1.0, 0.0, 3.0][..])
    );
    assert_eq!(
        patch_table.row(1).expect("missing patch").status(),
        EmbeddingStatus::MissingVector
    );
    assert!(patch_table
        .row(1)
        .expect("missing patch")
        .vector()
        .is_none());
    assert_eq!(patch_table.qc_summary().present_count(), 1);
    assert_eq!(patch_table.qc_summary().missing_vector_count(), 1);

    let region_table = RegionEmbeddingTable::from_rows(
        3,
        &regions,
        expected_region_artifact,
        region_support,
        ContentDigest::from_bytes(b"region-support-logical"),
        region_provenance,
        ContentDigest::from_bytes(b"region-provenance-logical"),
        vec![RegionEmbeddingRow::present(
            region("region-a"),
            vec![1.0, 0.0, 3.0],
        )],
        RETAINED_BUDGET,
    )
    .expect("region table");
    let slide_table = SlideEmbeddingTable::from_rows(
        3,
        &slides,
        expected_slide_artifact,
        slide_support,
        ContentDigest::from_bytes(b"slide-support-logical"),
        slide_provenance,
        ContentDigest::from_bytes(b"slide-provenance-logical"),
        vec![SlideEmbeddingRow::present(
            slide.clone(),
            vec![1.0, 0.0, 3.0],
        )],
        RETAINED_BUDGET,
    )
    .expect("slide table");

    assert_eq!(region_table.entity_kind(), EmbeddingEntityKind::Region);
    assert_eq!(slide_table.entity_kind(), EmbeddingEntityKind::Slide);
    assert_eq!(
        patch_table.logical_digest().to_string(),
        "75900a854f2d38ee7efda34fbf718b56243b478bcdf7777be1d8b06f223dcc1f"
    );
    assert_eq!(
        region_table.logical_digest().to_string(),
        "cd9f420606692f0cc9c9f48627275569b0344334dcc3b7f1c752d5b26ec394b3"
    );
    assert_eq!(
        slide_table.logical_digest().to_string(),
        "9c7780049309b07eb3e7bbd2da9fefe7a31c427d7dfa8df5ad24fbd6d0422739"
    );
    assert_ne!(
        patch_table.qc_summary().logical_digest(),
        region_table.qc_summary().logical_digest()
    );
    assert_ne!(
        region_table.qc_summary().logical_digest(),
        slide_table.qc_summary().logical_digest()
    );
    for debug in [
        format!("{patch_table:?}"),
        format!("{region_table:?}"),
        format!("{slide_table:?}"),
    ] {
        assert!(!debug.contains("1.0"));
        assert!(!debug.contains("3.0"));
        assert!(!debug.contains("values"));
    }
}

#[test]
fn table_blocks_recompute_all_statuses_with_partition_invariant_identity() {
    let (hierarchy, slide) = hierarchy();
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide,
        "four_statuses.v1",
        vec![
            patch("patch-a"),
            patch("patch-b"),
            patch("patch-c"),
            patch("patch-d"),
        ],
        RETAINED_BUDGET,
    )
    .expect("four expected patches");
    let table = PatchEmbeddingTable::from_rows(
        2,
        &expected,
        artifact_id(b"four-status-expected"),
        artifact_id(b"four-status-support"),
        ContentDigest::from_bytes(b"four-status-support-logical"),
        artifact_id(b"four-status-provenance"),
        ContentDigest::from_bytes(b"four-status-provenance-logical"),
        vec![
            PatchEmbeddingRow::present(patch("patch-a"), vec![-0.0, 0.0]),
            PatchEmbeddingRow::non_present(patch("patch-b"), EmbeddingStatus::MissingVector)
                .expect("missing row"),
            PatchEmbeddingRow::non_present(patch("patch-c"), EmbeddingStatus::ExtractionFailed)
                .expect("failed row"),
            PatchEmbeddingRow::non_present(patch("patch-d"), EmbeddingStatus::QcRejected)
                .expect("rejected row"),
        ],
        RETAINED_BUDGET,
    )
    .expect("four-status table");
    let summary = table.qc_summary();
    assert_eq!(summary.present_count(), 1);
    assert_eq!(summary.missing_vector_count(), 1);
    assert_eq!(summary.extraction_failed_count(), 1);
    assert_eq!(summary.qc_rejected_count(), 1);
    assert_eq!(summary.all_zero_present_count(), 1);
    for maximum_block_rows in 1..=4 {
        assert_eq!(
            table
                .scan_qc(maximum_block_rows)
                .expect("partitioned QC scan"),
            summary
        );
    }
    assert!(table.scan_qc(0).is_err());
    let block = table.block(1, 2).expect("middle status block");
    assert_eq!(block.row_count(), 2);
    assert_eq!(block.dimension(), 2);
    assert_eq!(
        block.row(0).expect("missing block row").status(),
        EmbeddingStatus::MissingVector
    );
    assert_eq!(
        block.row(1).expect("failed block row").status(),
        EmbeddingStatus::ExtractionFailed
    );
    assert!(block.rows().all(|row| row.vector().is_none()));
    assert!(table.block(3, 2).is_err());
}

#[test]
fn typed_tables_reject_expected_support_and_provenance_role_aliases() {
    let (hierarchy, slide) = hierarchy();
    let expected = expected_patches(&hierarchy, &slide);
    let expected_id = artifact_id(b"matrix-alias-expected");
    let support_id = artifact_id(b"matrix-alias-support");
    let provenance_id = artifact_id(b"matrix-alias-provenance");
    let rows = || {
        vec![
            PatchEmbeddingRow::present(patch("patch-a"), vec![1.0]),
            PatchEmbeddingRow::non_present(patch("patch-b"), EmbeddingStatus::MissingVector)
                .expect("missing row"),
        ]
    };
    for (expected_role, support_role, provenance_role) in [
        (expected_id, expected_id, provenance_id),
        (expected_id, support_id, expected_id),
        (expected_id, support_id, support_id),
    ] {
        assert!(matches!(
            PatchEmbeddingTable::from_rows(
                1,
                &expected,
                expected_role,
                support_role,
                ContentDigest::from_bytes(b"matrix-alias-support-logical"),
                provenance_role,
                ContentDigest::from_bytes(b"matrix-alias-provenance-logical"),
                rows(),
                RETAINED_BUDGET,
            ),
            Err(MultiscaleEmbeddingError::DuplicateMultiscaleTableArtifactDependency)
        ));
    }
}
