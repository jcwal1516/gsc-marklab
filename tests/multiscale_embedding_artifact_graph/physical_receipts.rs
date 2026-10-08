#![cfg(feature = "parquet")]

use super::support::*;
use marklab::{
    verify_patch_footprint_set_arrow_bytes, verify_patch_footprint_set_arrow_from_store,
    verify_patch_footprint_set_parquet_bytes, verify_patch_footprint_set_parquet_from_store,
    verify_patch_overlap_graph_arrow_bytes, verify_patch_overlap_graph_arrow_from_store,
    verify_patch_overlap_graph_parquet_bytes, verify_patch_overlap_graph_parquet_from_store,
    write_patch_footprint_set_arrow, write_patch_footprint_set_parquet,
    write_patch_overlap_graph_arrow, write_patch_overlap_graph_parquet, EmbeddingColumnarBudgets,
    MultiscaleColumnarError, VerifiedDirectPatchEmbeddingArtifactGraph,
    VerifiedPatchFootprintArtifact,
};

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    )
}

fn validate_graph(fixture: &Fixture) -> VerifiedDirectPatchEmbeddingArtifactGraph {
    fixture
        .provenance
        .validate_direct_patch_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_patches,
            &fixture.source_entities,
            &fixture.identity_map,
            &fixture.source_row_link,
            &fixture.input_normalization,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            &fixture.support,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("valid direct-patch graph")
}

fn arrow_footprint_receipt(
    fixture: &Fixture,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
) -> VerifiedPatchFootprintArtifact {
    verify_patch_footprint_set_arrow_from_store(
        &fixture.store,
        &fixture.footprint_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        graph,
        budgets(),
    )
    .expect("verify managed footprint Arrow")
}

fn parquet_footprint_receipt(
    fixture: &Fixture,
    graph: VerifiedDirectPatchEmbeddingArtifactGraph,
) -> VerifiedPatchFootprintArtifact {
    verify_patch_footprint_set_parquet_from_store(
        &fixture.store,
        &fixture.footprint_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        graph,
        budgets(),
    )
    .expect("verify managed footprint Parquet")
}

#[test]
fn arrow_physical_receipts_require_full_decode_and_match_borrowed_and_managed_paths() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let graph = validate_graph(&fixture);
    let managed_footprint = arrow_footprint_receipt(&fixture, graph);
    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_arrow(
        &mut footprint_bytes,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        budgets(),
    )
    .expect("write footprint Arrow");
    let borrowed_footprint = verify_patch_footprint_set_arrow_bytes(
        &footprint_bytes,
        &fixture.footprint_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        graph,
        budgets(),
    )
    .expect("verify borrowed footprint Arrow");
    assert_eq!(borrowed_footprint, managed_footprint);
    assert_eq!(
        borrowed_footprint.artifact_id(),
        fixture.footprint_record.id()
    );
    assert_eq!(
        borrowed_footprint.logical_digest(),
        fixture.footprints.logical_digest()
    );
    assert_eq!(borrowed_footprint.row_count(), 2);

    let managed_overlap = verify_patch_overlap_graph_arrow_from_store(
        &fixture.store,
        &fixture.overlap_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        managed_footprint,
        graph,
        budgets(),
    )
    .expect("verify managed overlap Arrow");
    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap_bytes,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        budgets(),
    )
    .expect("write overlap Arrow");
    let borrowed_overlap = verify_patch_overlap_graph_arrow_bytes(
        &overlap_bytes,
        &fixture.overlap_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        borrowed_footprint,
        graph,
        budgets(),
    )
    .expect("verify borrowed overlap Arrow");
    assert_eq!(borrowed_overlap, managed_overlap);
    assert_eq!(borrowed_overlap.artifact_id(), fixture.overlap_record.id());
    assert_eq!(
        borrowed_overlap.logical_digest(),
        fixture.overlap.logical_digest()
    );
    assert_eq!(borrowed_overlap.row_count(), 1);

    let debug = format!("{borrowed_footprint:?} {borrowed_overlap:?}");
    assert!(!debug.contains("graph-slide"));
    assert!(!debug.contains(&fixture.footprint_record.id().to_string()));
    assert!(!debug.contains(&fixture.overlap_record.id().to_string()));
    assert!(!debug.contains("private-source"));
}

#[test]
fn parquet_physical_receipts_require_full_decode_and_match_borrowed_and_managed_paths() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        ..FixtureOptions::default()
    });
    let graph = validate_graph(&fixture);
    let managed_footprint = parquet_footprint_receipt(&fixture, graph);
    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_parquet(
        &mut footprint_bytes,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        budgets(),
    )
    .expect("write footprint Parquet");
    let borrowed_footprint = verify_patch_footprint_set_parquet_bytes(
        &footprint_bytes,
        &fixture.footprint_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        graph,
        budgets(),
    )
    .expect("verify borrowed footprint Parquet");
    assert_eq!(borrowed_footprint, managed_footprint);

    let managed_overlap = verify_patch_overlap_graph_parquet_from_store(
        &fixture.store,
        &fixture.overlap_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        managed_footprint,
        graph,
        budgets(),
    )
    .expect("verify managed overlap Parquet");
    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap_bytes,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        budgets(),
    )
    .expect("write overlap Parquet");
    let borrowed_overlap = verify_patch_overlap_graph_parquet_bytes(
        &overlap_bytes,
        &fixture.overlap_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        borrowed_footprint,
        graph,
        budgets(),
    )
    .expect("verify borrowed overlap Parquet");
    assert_eq!(borrowed_overlap, managed_overlap);
    assert_eq!(borrowed_overlap.row_count(), 1);
}

#[test]
fn overlap_receipt_rejects_a_format_distinct_footprint_receipt() {
    let arrow = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let arrow_receipt = arrow_footprint_receipt(&arrow, validate_graph(&arrow));
    let parquet = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: true,
        ..FixtureOptions::default()
    });
    let parquet_graph = validate_graph(&parquet);
    assert_eq!(
        arrow.footprints.logical_digest(),
        parquet.footprints.logical_digest()
    );
    assert_ne!(arrow.footprint_record.id(), parquet.footprint_record.id());
    assert!(matches!(
        verify_patch_overlap_graph_parquet_from_store(
            &parquet.store,
            &parquet.overlap_record,
            &parquet.expected_patches,
            &parquet.context,
            &parquet.footprints,
            &parquet.overlap,
            arrow_receipt,
            parquet_graph,
            budgets(),
        ),
        Err(marklab::VerifiedReaderError::Callback(
            MultiscaleColumnarError::ArtifactBindingMismatch
        ))
    ));
}

#[test]
fn structural_graph_receipt_does_not_decode_placeholder_physical_bytes() {
    let fixture = fixture();
    let graph = validate_graph(&fixture);
    assert!(verify_patch_footprint_set_arrow_from_store(
        &fixture.store,
        &fixture.footprint_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        graph,
        budgets(),
    )
    .is_err());
}

#[test]
fn managed_integrity_failure_precedes_physical_decode_and_remains_source_private() {
    let fixture = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        ..FixtureOptions::default()
    });
    let graph = validate_graph(&fixture);
    let artifact = fixture.footprint_record.id().to_string();
    let managed_path = fixture
        ._root
        .path()
        .join("objects/sha256")
        .join(&artifact[..2])
        .join(&artifact);
    let corrupt = vec![0_u8; fixture.footprint_record.content().byte_len() as usize];
    std::fs::write(&managed_path, corrupt).expect("corrupt temporary managed footprint");
    let error = verify_patch_footprint_set_arrow_from_store(
        &fixture.store,
        &fixture.footprint_record,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        graph,
        budgets(),
    )
    .expect_err("managed integrity must fail");
    assert!(matches!(error, marklab::VerifiedReaderError::Store(_)));
    let rendered = error.to_string();
    assert!(!rendered.contains("private-source"));
    assert!(!rendered.contains(&fixture._root.path().display().to_string()));
}
