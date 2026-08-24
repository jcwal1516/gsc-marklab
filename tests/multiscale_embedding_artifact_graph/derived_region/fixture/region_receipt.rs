use super::*;
use marklab::{
    publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
    verify_region_embedding_table_arrow_bytes, verify_region_embedding_table_arrow_from_store,
    verify_region_embedding_table_parquet_bytes, verify_region_embedding_table_parquet_from_store,
    write_region_embedding_table_arrow, write_region_embedding_table_parquet, ArtifactStoreError,
    DerivedRegionEmbeddingTableCandidate, MultiscaleColumnarError, VerifiedReaderError,
};

fn finalized_candidate(fixture: &DerivedFixture) -> DerivedRegionEmbeddingTableCandidate {
    finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        validate_graph(fixture),
        EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, BUDGET as u64, BUDGET as u64),
    )
    .expect("derived region candidate")
}

#[test]
fn arrow_region_receipt_matches_borrowed_and_managed_paths() {
    let fixture = derived_fixture();
    let candidate = finalized_candidate(&fixture);
    let record =
        publish_region_embedding_table_arrow(&fixture._direct.store, candidate.table(), budgets())
            .expect("publish region Arrow")
            .into_record();
    let managed = verify_region_embedding_table_arrow_from_store(
        &fixture._direct.store,
        &record,
        &candidate,
        budgets(),
    )
    .expect("verify managed region Arrow");
    let mut bytes = Vec::new();
    write_region_embedding_table_arrow(&mut bytes, candidate.table(), budgets())
        .expect("write region Arrow");
    let borrowed =
        verify_region_embedding_table_arrow_bytes(&bytes, &record, &candidate, budgets())
            .expect("verify borrowed region Arrow");

    assert_eq!(borrowed, managed);
    assert_eq!(borrowed.artifact_id(), record.id());
    assert_eq!(borrowed.logical_digest(), candidate.logical_digest());
    assert_eq!(borrowed.qc_summary(), candidate.qc_summary());
    assert_eq!(borrowed.row_count(), 2);
    assert_eq!(borrowed.dimension(), 3);
    assert_eq!(
        borrowed.expected_regions_artifact_id(),
        fixture.expected_region_record.id()
    );
    assert_eq!(
        borrowed.expected_regions_logical_digest(),
        fixture.expected_regions.logical_digest()
    );
    assert_eq!(
        borrowed.region_support_artifact_id(),
        fixture.region_support_record.id()
    );
    assert_eq!(
        borrowed.region_support_logical_digest(),
        fixture.region_support_logical_digest
    );
    assert_eq!(
        borrowed.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(
        borrowed.provenance_logical_digest(),
        fixture.provenance.logical_digest()
    );
    assert_eq!(
        borrowed.source_patch_table_artifact_id(),
        fixture.source_patch_table_record.id()
    );
    assert_eq!(
        borrowed.source_patch_table_logical_digest(),
        fixture.source_patch_table_value.logical_digest()
    );
    assert_eq!(
        borrowed.patch_region_link_artifact_id(),
        fixture.link_receipt.artifact_id()
    );
    assert_eq!(
        borrowed.patch_region_link_logical_digest(),
        fixture.link.logical_digest()
    );
    assert_eq!(
        borrowed.derivation_artifact_id(),
        fixture.derivation_record.id()
    );
    assert_eq!(
        borrowed.derivation_logical_digest(),
        fixture.derivation.logical_digest()
    );
    let debug = format!("{borrowed:?}");
    assert!(!debug.contains("derived-region-"));
    assert!(!debug.contains(&fixture.provenance_artifact_id.to_string()));
}

#[test]
fn parquet_region_receipt_matches_borrowed_and_managed_paths() {
    let fixture = derived_fixture_with_options(DerivedFixtureOptions {
        patch_format: PhysicalFormat::Parquet,
        link_format: PhysicalFormat::Parquet,
        direct_parquet_physical: true,
        ..DerivedFixtureOptions::default()
    });
    let candidate = finalized_candidate(&fixture);
    let record = publish_region_embedding_table_parquet(
        &fixture._direct.store,
        candidate.table(),
        budgets(),
    )
    .expect("publish region Parquet")
    .into_record();
    let managed = verify_region_embedding_table_parquet_from_store(
        &fixture._direct.store,
        &record,
        &candidate,
        budgets(),
    )
    .expect("verify managed region Parquet");
    let mut bytes = Vec::new();
    write_region_embedding_table_parquet(&mut bytes, candidate.table(), budgets())
        .expect("write region Parquet");
    let borrowed =
        verify_region_embedding_table_parquet_bytes(&bytes, &record, &candidate, budgets())
            .expect("verify borrowed region Parquet");

    assert_eq!(borrowed, managed);
}

#[test]
fn region_receipt_requires_the_exact_candidate_and_preserves_failure_precedence() {
    let fixture = derived_fixture();
    let candidate = finalized_candidate(&fixture);
    let record =
        publish_region_embedding_table_arrow(&fixture._direct.store, candidate.table(), budgets())
            .expect("publish region Arrow")
            .into_record();
    let mut bytes = Vec::new();
    write_region_embedding_table_arrow(&mut bytes, candidate.table(), budgets())
        .expect("write region Arrow");

    let different = derived_fixture_with_options(DerivedFixtureOptions {
        region_count: 1,
        ..DerivedFixtureOptions::default()
    });
    assert!(verify_region_embedding_table_arrow_bytes(
        &bytes,
        &record,
        &finalized_candidate(&different),
        budgets(),
    )
    .is_err());

    bytes[0] ^= 1;
    assert!(matches!(
        verify_region_embedding_table_arrow_bytes(&bytes, &record, &candidate, budgets()),
        Err(MultiscaleColumnarError::Arrow { .. })
    ));

    corrupt_managed(&fixture._direct._root, record.id());
    assert!(matches!(
        verify_region_embedding_table_arrow_from_store(
            &fixture._direct.store,
            &record,
            &candidate,
            budgets(),
        ),
        Err(VerifiedReaderError::Store(
            ArtifactStoreError::ContentIntegrity { .. }
        ))
    ));
}
