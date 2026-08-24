use super::*;
use marklab::{
    publish_slide_embedding_table_arrow, publish_slide_embedding_table_parquet,
    verify_slide_embedding_table_arrow_bytes, verify_slide_embedding_table_arrow_from_store,
    verify_slide_embedding_table_parquet_bytes, verify_slide_embedding_table_parquet_from_store,
    write_slide_embedding_table_arrow, write_slide_embedding_table_parquet, ArtifactStoreError,
    MultiscaleColumnarError, VerifiedReaderError, VerifiedSlideEmbeddingTableArtifact,
};

fn assert_receipt_lineage(
    fixture: &SlideFixture,
    candidate: &DerivedSlideEmbeddingTableCandidate,
    receipt: VerifiedSlideEmbeddingTableArtifact,
) {
    assert_eq!(receipt.logical_digest(), candidate.logical_digest());
    assert_eq!(receipt.qc_summary(), candidate.qc_summary());
    assert_eq!(receipt.row_count(), 1);
    assert_eq!(receipt.dimension(), candidate.dimension());
    assert_eq!(
        receipt.expected_slides_artifact_id(),
        fixture.expected_slide_record.id()
    );
    assert_eq!(
        receipt.expected_slides_logical_digest(),
        fixture.expected_slides.logical_digest()
    );
    assert_eq!(
        receipt.slide_support_artifact_id(),
        fixture.support_record.id()
    );
    assert_eq!(
        receipt.slide_support_logical_digest(),
        fixture.support_value.logical_digest()
    );
    assert_eq!(
        receipt.provenance_artifact_id(),
        fixture.provenance_record.id()
    );
    assert_eq!(
        receipt.provenance_logical_digest(),
        fixture.provenance.logical_digest()
    );
    assert_eq!(
        receipt.source_table_artifact_id(),
        source_table_record(fixture).id()
    );
    assert_eq!(
        receipt.source_table_logical_digest(),
        match fixture.source_level {
            SlideSourceLevel::Patches => fixture.lower.source_patch_table_value.logical_digest(),
            SlideSourceLevel::Regions => source_region_candidate(fixture).logical_digest(),
        }
    );
    assert_eq!(
        receipt.derivation_artifact_id(),
        fixture.derivation_record.id()
    );
    assert_eq!(
        receipt.derivation_logical_digest(),
        fixture.derivation.logical_digest()
    );
}

#[test]
fn slide_receipts_match_arrow_parquet_borrowed_managed_for_both_paths() {
    for (source_level, format) in [
        (SlideSourceLevel::Patches, PhysicalFormat::Arrow),
        (SlideSourceLevel::Patches, PhysicalFormat::Parquet),
        (SlideSourceLevel::Regions, PhysicalFormat::Arrow),
        (SlideSourceLevel::Regions, PhysicalFormat::Parquet),
    ] {
        let fixture = slide_fixture_with_options(SlideFixtureOptions {
            source_level,
            lower_format: format,
            ..SlideFixtureOptions::default()
        });
        let candidate = finalize_slide(&fixture);
        let (borrowed, managed) = match format {
            PhysicalFormat::Arrow => {
                let record = publish_slide_embedding_table_arrow(
                    &fixture.lower._direct.store,
                    candidate.table(),
                    budgets(),
                )
                .expect("publish slide Arrow")
                .into_record();
                let managed = verify_slide_embedding_table_arrow_from_store(
                    &fixture.lower._direct.store,
                    &record,
                    &candidate,
                    budgets(),
                )
                .expect("verify managed slide Arrow");
                let mut bytes = Vec::new();
                write_slide_embedding_table_arrow(&mut bytes, candidate.table(), budgets())
                    .expect("write slide Arrow");
                let borrowed = verify_slide_embedding_table_arrow_bytes(
                    &bytes,
                    &record,
                    &candidate,
                    budgets(),
                )
                .expect("verify borrowed slide Arrow");
                (borrowed, managed)
            }
            PhysicalFormat::Parquet => {
                let record = publish_slide_embedding_table_parquet(
                    &fixture.lower._direct.store,
                    candidate.table(),
                    budgets(),
                )
                .expect("publish slide Parquet")
                .into_record();
                let managed = verify_slide_embedding_table_parquet_from_store(
                    &fixture.lower._direct.store,
                    &record,
                    &candidate,
                    budgets(),
                )
                .expect("verify managed slide Parquet");
                let mut bytes = Vec::new();
                write_slide_embedding_table_parquet(&mut bytes, candidate.table(), budgets())
                    .expect("write slide Parquet");
                let borrowed = verify_slide_embedding_table_parquet_bytes(
                    &bytes,
                    &record,
                    &candidate,
                    budgets(),
                )
                .expect("verify borrowed slide Parquet");
                (borrowed, managed)
            }
        };
        assert_eq!(borrowed, managed);
        assert_receipt_lineage(&fixture, &candidate, borrowed);
        let debug = format!("{candidate:?} {borrowed:?}");
        assert!(!debug.contains("derived-slide"));
        assert!(!debug.contains(&fixture.provenance_record.id().to_string()));
    }
}

#[test]
fn slide_receipts_preserve_failure_precedence_and_privacy() {
    let fixture = slide_fixture(SlideSourceLevel::Patches);
    let candidate = finalize_slide(&fixture);
    let record = publish_slide_embedding_table_arrow(
        &fixture.lower._direct.store,
        candidate.table(),
        budgets(),
    )
    .expect("publish slide Arrow")
    .into_record();
    let mut bytes = Vec::new();
    write_slide_embedding_table_arrow(&mut bytes, candidate.table(), budgets())
        .expect("write slide Arrow");
    let different = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Patches,
        entity_count: 3,
        ..SlideFixtureOptions::default()
    });
    let different_candidate = finalize_slide(&different);

    assert!(matches!(
        verify_slide_embedding_table_arrow_bytes(&bytes, &record, &different_candidate, budgets(),),
        Err(MultiscaleColumnarError::ArtifactBindingMismatch)
    ));

    bytes[0] ^= 1;
    assert!(matches!(
        verify_slide_embedding_table_arrow_bytes(&bytes, &record, &candidate, budgets(),),
        Err(MultiscaleColumnarError::Arrow { .. })
    ));

    corrupt_managed(&fixture.lower._direct._root, record.id());
    let error = verify_slide_embedding_table_arrow_from_store(
        &fixture.lower._direct.store,
        &record,
        &different_candidate,
        budgets(),
    )
    .expect_err("managed integrity must precede candidate binding");
    assert!(matches!(
        error,
        VerifiedReaderError::Store(ArtifactStoreError::ContentIntegrity { .. })
    ));
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("objects/sha256"));
    assert!(!rendered.contains("private-derived-region-corruption"));
    assert!(!rendered.contains(&fixture.lower._direct._root.path().display().to_string()));
}
