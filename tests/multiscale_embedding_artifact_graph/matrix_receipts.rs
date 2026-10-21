#![cfg(feature = "parquet")]

use super::support::*;
use marklab::embeddings::{
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    verify_patch_embedding_table_arrow_bytes, verify_patch_embedding_table_arrow_from_store,
    verify_patch_embedding_table_parquet_bytes, verify_patch_embedding_table_parquet_from_store,
    verify_patch_footprint_set_parquet_from_store, verify_patch_overlap_graph_parquet_from_store,
    write_patch_embedding_table_arrow, write_patch_embedding_table_parquet, ColumnarError,
    EmbeddingStatus, PatchEmbeddingRow, PatchEmbeddingTable,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchEmbeddingSupportArtifact,
};

fn table(
    fixture: &Fixture,
    support: VerifiedPatchEmbeddingSupportArtifact,
    first_status: Option<EmbeddingStatus>,
    provenance_digest: ContentDigest,
) -> PatchEmbeddingTable {
    table_with_bindings(
        fixture,
        first_status,
        fixture.provenance.output_dimension(),
        fixture.source_row_link.expected_patches_artifact_id(),
        support.artifact_id(),
        support.logical_digest(),
        fixture.provenance_artifact_id,
        provenance_digest,
    )
}

#[allow(clippy::too_many_arguments)]
fn table_with_bindings(
    fixture: &Fixture,
    first_status: Option<EmbeddingStatus>,
    dimension: u32,
    expected_artifact_id: ArtifactId,
    support_artifact_id: ArtifactId,
    support_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
) -> PatchEmbeddingTable {
    let dimension_usize = usize::try_from(dimension).expect("test dimension");
    let rows = fixture
        .expected_patches
        .ids()
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let status = if index == 0 {
                first_status.unwrap_or_else(|| fixture.source_row_link.entries()[index].status())
            } else {
                fixture.source_row_link.entries()[index].status()
            };
            if status == EmbeddingStatus::Present {
                PatchEmbeddingRow::present(id.clone(), vec![index as f32 + 1.0; dimension_usize])
            } else {
                PatchEmbeddingRow::non_present(id.clone(), status).expect("non-present patch row")
            }
        })
        .collect();
    PatchEmbeddingTable::from_rows(
        dimension,
        &fixture.expected_patches,
        expected_artifact_id,
        support_artifact_id,
        support_logical_digest,
        provenance_artifact_id,
        provenance_logical_digest,
        rows,
        BUDGET,
    )
    .expect("patch table")
}

#[test]
fn patch_support_receipt_composes_exact_physical_components_and_redacts_debug() {
    let arrow = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let arrow_graph = arrow
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&arrow, arrow_graph, false, false);
    assert_eq!(support.logical_digest(), arrow.support.logical_digest());
    assert_eq!(support.row_count(), 2);

    let parquet = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        ..FixtureOptions::default()
    });
    let parquet_graph = parquet
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let parquet_support = patch_support(&parquet, parquet_graph, true, true);
    assert_ne!(support.logical_digest(), parquet_support.logical_digest());
    assert!(matches!(
        VerifiedPatchEmbeddingSupportArtifact::from_verified_components(
            arrow_graph,
            verify_patch_footprint_set_parquet_from_store(
                &parquet.store,
                &parquet.footprint_record,
                &parquet.expected_patches,
                &parquet.context,
                &parquet.footprints,
                parquet_graph,
                budgets(),
            )
            .expect("verified Parquet footprints"),
            verify_patch_overlap_graph_parquet_from_store(
                &parquet.store,
                &parquet.overlap_record,
                &parquet.expected_patches,
                &parquet.context,
                &parquet.footprints,
                &parquet.overlap,
                verify_patch_footprint_set_parquet_from_store(
                    &parquet.store,
                    &parquet.footprint_record,
                    &parquet.expected_patches,
                    &parquet.context,
                    &parquet.footprints,
                    parquet_graph,
                    budgets(),
                )
                .expect("verified Parquet footprints"),
                parquet_graph,
                budgets(),
            )
            .expect("verified Parquet overlap"),
        ),
        Err(ColumnarError::ArtifactBindingMismatch)
    ));

    let debug = format!("{support:?}");
    assert!(!debug.contains("graph-slide"));
    assert!(!debug.contains(&support.artifact_id().to_string()));
}

#[test]
fn patch_support_receipt_composes_mixed_physical_formats() {
    let arrow_footprint = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        overlap_parquet_physical: Some(true),
        ..FixtureOptions::default()
    });
    let arrow_footprint_graph = arrow_footprint
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let arrow_footprint_support =
        patch_support(&arrow_footprint, arrow_footprint_graph, false, true);
    assert_eq!(
        arrow_footprint_support.logical_digest(),
        arrow_footprint.support.logical_digest()
    );

    let parquet_footprint = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        overlap_parquet_physical: Some(false),
        ..FixtureOptions::default()
    });
    let parquet_footprint_graph = parquet_footprint
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let parquet_footprint_support =
        patch_support(&parquet_footprint, parquet_footprint_graph, true, false);
    assert_eq!(
        parquet_footprint_support.logical_digest(),
        parquet_footprint.support.logical_digest()
    );
}

#[test]
fn arrow_patch_matrix_receipts_match_borrowed_and_managed_paths() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let graph = fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&fixture, graph, false, false);
    let table = table(&fixture, support, None, fixture.provenance.logical_digest());
    let record = publish_patch_embedding_table_arrow(&fixture.store, &table, budgets())
        .expect("publish patch Arrow")
        .into_record();
    let managed = verify_patch_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &table,
        &fixture.source_row_link,
        support,
        graph,
        budgets(),
    )
    .expect("verify managed patch Arrow");
    let mut bytes = Vec::new();
    write_patch_embedding_table_arrow(&mut bytes, &table, budgets()).expect("write patch Arrow");
    let borrowed = verify_patch_embedding_table_arrow_bytes(
        &bytes,
        &record,
        &table,
        &fixture.source_row_link,
        support,
        graph,
        budgets(),
    )
    .expect("verify borrowed patch Arrow");
    assert_eq!(borrowed, managed);
    assert_eq!(borrowed.artifact_id(), record.id());
    assert_eq!(borrowed.logical_digest(), table.logical_digest());
    assert_eq!(borrowed.qc_summary(), table.qc_summary());
    assert_eq!(borrowed.row_count(), 2);
    assert_eq!(borrowed.dimension(), 1_024);
}

#[test]
fn parquet_patch_matrix_receipts_match_borrowed_and_managed_paths() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        ..FixtureOptions::default()
    });
    let graph = fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&fixture, graph, true, true);
    let table = table(&fixture, support, None, fixture.provenance.logical_digest());
    let record = publish_patch_embedding_table_parquet(&fixture.store, &table, budgets())
        .expect("publish patch Parquet")
        .into_record();
    let managed = verify_patch_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &table,
        &fixture.source_row_link,
        support,
        graph,
        budgets(),
    )
    .expect("verify managed patch Parquet");
    let mut bytes = Vec::new();
    write_patch_embedding_table_parquet(&mut bytes, &table, budgets())
        .expect("write patch Parquet");
    let borrowed = verify_patch_embedding_table_parquet_bytes(
        &bytes,
        &record,
        &table,
        &fixture.source_row_link,
        support,
        graph,
        budgets(),
    )
    .expect("verify borrowed patch Parquet");
    assert_eq!(borrowed, managed);
}

#[test]
fn patch_matrix_receipts_are_format_neutral_and_preserve_all_statuses() {
    let arrow_support_fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        entity_count: 4,
        source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
        ..FixtureOptions::default()
    });
    let arrow_graph = arrow_support_fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let arrow_support = patch_support(&arrow_support_fixture, arrow_graph, false, false);
    let parquet_table = table(
        &arrow_support_fixture,
        arrow_support,
        None,
        arrow_support_fixture.provenance.logical_digest(),
    );
    let parquet_record = publish_patch_embedding_table_parquet(
        &arrow_support_fixture.store,
        &parquet_table,
        budgets(),
    )
    .expect("publish Parquet with Arrow support")
    .into_record();
    let parquet_receipt = verify_patch_embedding_table_parquet_from_store(
        &arrow_support_fixture.store,
        &parquet_record,
        &parquet_table,
        &arrow_support_fixture.source_row_link,
        arrow_support,
        arrow_graph,
        budgets(),
    )
    .expect("verify Parquet with Arrow support");
    let summary = parquet_receipt.qc_summary();
    assert_eq!(summary.present_count(), 1);
    assert_eq!(summary.missing_vector_count(), 1);
    assert_eq!(summary.extraction_failed_count(), 1);
    assert_eq!(summary.qc_rejected_count(), 1);

    let parquet_support_fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        entity_count: 4,
        source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
        ..FixtureOptions::default()
    });
    let parquet_graph = parquet_support_fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let parquet_support = patch_support(&parquet_support_fixture, parquet_graph, true, true);
    let arrow_table = table(
        &parquet_support_fixture,
        parquet_support,
        None,
        parquet_support_fixture.provenance.logical_digest(),
    );
    let arrow_record = publish_patch_embedding_table_arrow(
        &parquet_support_fixture.store,
        &arrow_table,
        budgets(),
    )
    .expect("publish Arrow with Parquet support")
    .into_record();
    verify_patch_embedding_table_arrow_from_store(
        &parquet_support_fixture.store,
        &arrow_record,
        &arrow_table,
        &parquet_support_fixture.source_row_link,
        parquet_support,
        parquet_graph,
        budgets(),
    )
    .expect("verify Arrow with Parquet support");
}

#[test]
fn patch_matrix_receipts_cover_empty_and_maximum_dimension_boundaries() {
    let empty = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        entity_count: 0,
        ..FixtureOptions::default()
    });
    let empty_graph = empty
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let empty_support = patch_support(&empty, empty_graph, false, false);
    let empty_table = table(
        &empty,
        empty_support,
        None,
        empty.provenance.logical_digest(),
    );
    let empty_record = publish_patch_embedding_table_arrow(&empty.store, &empty_table, budgets())
        .expect("publish empty patch Arrow")
        .into_record();
    let empty_receipt = verify_patch_embedding_table_arrow_from_store(
        &empty.store,
        &empty_record,
        &empty_table,
        &empty.source_row_link,
        empty_support,
        empty_graph,
        budgets(),
    )
    .expect("verify empty patch Arrow");
    assert_eq!(empty_receipt.row_count(), 0);

    let maximum = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        entity_count: 1,
        output_dimension: 65_536,
        ..FixtureOptions::default()
    });
    let maximum_graph = maximum
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let maximum_support = patch_support(&maximum, maximum_graph, true, true);
    let maximum_table = table(
        &maximum,
        maximum_support,
        None,
        maximum.provenance.logical_digest(),
    );
    let maximum_record =
        publish_patch_embedding_table_parquet(&maximum.store, &maximum_table, budgets())
            .expect("publish maximum-dimension patch Parquet")
            .into_record();
    let maximum_receipt = verify_patch_embedding_table_parquet_from_store(
        &maximum.store,
        &maximum_record,
        &maximum_table,
        &maximum.source_row_link,
        maximum_support,
        maximum_graph,
        budgets(),
    )
    .expect("verify maximum-dimension patch Parquet");
    assert_eq!(maximum_receipt.dimension(), 65_536);
}

#[test]
fn patch_matrix_receipt_crosses_the_public_arrow_batch_boundary() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        entity_count: 8_193,
        output_dimension: 3,
        source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
        ..FixtureOptions::default()
    });
    let graph = fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&fixture, graph, false, false);
    let table = table(&fixture, support, None, fixture.provenance.logical_digest());
    let record = publish_patch_embedding_table_arrow(&fixture.store, &table, budgets())
        .expect("publish multi-batch patch Arrow")
        .into_record();
    let receipt = verify_patch_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &table,
        &fixture.source_row_link,
        support,
        graph,
        budgets(),
    )
    .expect("verify multi-batch patch Arrow");
    assert_eq!(receipt.row_count(), 8_193);
    assert_eq!(receipt.dimension(), 3);
}

fn assert_arrow_receipt_rejected(
    fixture: &Fixture,
    table: &PatchEmbeddingTable,
    support: VerifiedPatchEmbeddingSupportArtifact,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
) {
    let record = publish_patch_embedding_table_arrow(&fixture.store, table, budgets())
        .expect("publish binding-drift patch Arrow")
        .into_record();
    assert!(matches!(
        verify_patch_embedding_table_arrow_from_store(
            &fixture.store,
            &record,
            table,
            &fixture.source_row_link,
            support,
            graph,
            budgets(),
        ),
        Err(marklab::workflow::VerifiedReaderError::Callback(
            ColumnarError::ArtifactBindingMismatch
        ))
    ));
}

#[test]
fn patch_matrix_receipt_rejects_every_table_graph_binding_drift() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let graph = fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&fixture, graph, false, false);
    let expected_id = fixture.source_row_link.expected_patches_artifact_id();
    let provenance_digest = fixture.provenance.logical_digest();
    let cases = [
        table_with_bindings(
            &fixture,
            None,
            1_024,
            artifact(b"wrong-expected-artifact"),
            support.artifact_id(),
            support.logical_digest(),
            fixture.provenance_artifact_id,
            provenance_digest,
        ),
        table_with_bindings(
            &fixture,
            None,
            1_024,
            expected_id,
            artifact(b"wrong-support-artifact"),
            support.logical_digest(),
            fixture.provenance_artifact_id,
            provenance_digest,
        ),
        table_with_bindings(
            &fixture,
            None,
            1_024,
            expected_id,
            support.artifact_id(),
            ContentDigest::from_bytes(b"wrong-support-digest"),
            fixture.provenance_artifact_id,
            provenance_digest,
        ),
        table_with_bindings(
            &fixture,
            None,
            1_024,
            expected_id,
            support.artifact_id(),
            support.logical_digest(),
            artifact(b"wrong-provenance-artifact"),
            provenance_digest,
        ),
        table_with_bindings(
            &fixture,
            None,
            1_023,
            expected_id,
            support.artifact_id(),
            support.logical_digest(),
            fixture.provenance_artifact_id,
            provenance_digest,
        ),
    ];
    for table in &cases {
        assert_arrow_receipt_rejected(&fixture, table, support, graph);
    }
}

#[test]
fn physical_failure_and_managed_integrity_precede_receipt_binding_checks() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let graph = fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&fixture, graph, false, false);
    let wrong_provenance = table(
        &fixture,
        support,
        None,
        ContentDigest::from_bytes(b"wrong-provenance-for-precedence"),
    );
    let record = publish_patch_embedding_table_arrow(&fixture.store, &wrong_provenance, budgets())
        .expect("publish precedence patch Arrow")
        .into_record();
    let mut bytes = Vec::new();
    write_patch_embedding_table_arrow(&mut bytes, &wrong_provenance, budgets())
        .expect("write precedence patch Arrow");
    bytes[0] ^= 1;
    assert!(matches!(
        verify_patch_embedding_table_arrow_bytes(
            &bytes,
            &record,
            &wrong_provenance,
            &fixture.source_row_link,
            support,
            graph,
            budgets(),
        ),
        Err(ColumnarError::Arrow { .. })
    ));

    let artifact = record.id().to_string();
    let managed_path = fixture
        ._root
        .path()
        .join("objects/sha256")
        .join(&artifact[..2])
        .join(&artifact);
    std::fs::write(
        &managed_path,
        vec![0_u8; record.content().byte_len() as usize],
    )
    .expect("corrupt temporary managed matrix");
    let error = verify_patch_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &wrong_provenance,
        &fixture.source_row_link,
        support,
        graph,
        budgets(),
    )
    .expect_err("managed integrity must fail before receipt binding");
    assert!(matches!(
        error,
        marklab::workflow::VerifiedReaderError::Store(
            marklab::workflow::ArtifactStoreError::ContentIntegrity { .. }
        )
    ));
    let rendered = error.to_string();
    assert!(!rendered.contains("private-source"));
    assert!(!rendered.contains(&fixture._root.path().display().to_string()));
}

#[test]
fn patch_matrix_receipt_rejects_provenance_digest_and_source_status_drift() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let graph = fixture
        .direct_graph_result()
        .expect("valid direct-patch graph");
    let support = patch_support(&fixture, graph, false, false);

    let wrong_provenance = table(
        &fixture,
        support,
        None,
        ContentDigest::from_bytes(b"wrong-provenance-logical-digest"),
    );
    let wrong_provenance_record =
        publish_patch_embedding_table_arrow(&fixture.store, &wrong_provenance, budgets())
            .expect("publish wrong-provenance patch Arrow")
            .into_record();
    assert!(matches!(
        verify_patch_embedding_table_arrow_from_store(
            &fixture.store,
            &wrong_provenance_record,
            &wrong_provenance,
            &fixture.source_row_link,
            support,
            graph,
            budgets(),
        ),
        Err(marklab::workflow::VerifiedReaderError::Callback(
            ColumnarError::ArtifactBindingMismatch
        ))
    ));

    let wrong_status = table(
        &fixture,
        support,
        Some(EmbeddingStatus::MissingVector),
        fixture.provenance.logical_digest(),
    );
    let wrong_status_record =
        publish_patch_embedding_table_arrow(&fixture.store, &wrong_status, budgets())
            .expect("publish wrong-status patch Arrow")
            .into_record();
    assert!(matches!(
        verify_patch_embedding_table_arrow_from_store(
            &fixture.store,
            &wrong_status_record,
            &wrong_status,
            &fixture.source_row_link,
            support,
            graph,
            budgets(),
        ),
        Err(marklab::workflow::VerifiedReaderError::Callback(
            ColumnarError::ArtifactBindingMismatch
        ))
    ));
}
