#![cfg(feature = "parquet")]

#[allow(dead_code)]
#[path = "support/cell_patch_columnar.rs"]
mod cell_patch;
#[allow(dead_code)]
#[path = "support/multiscale_matrix_columnar.rs"]
mod matrices;
#[allow(dead_code, unused_imports)]
#[path = "patch_region_link/support.rs"]
mod patch_region;

use marklab::embeddings::*;
use marklab::workflow::{LocalArtifactStore, StoreId, VerifiedReaderError};
use std::io::Cursor;
use tempfile::TempDir;

// Budget enforcement has its own resource tests; these round trips need room for the
// two-row-group matrix.
fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(64 << 20, 64 << 20, 64 << 20, 64 << 20)
}

fn round_trip<F: ColumnarFormat<E>, E: ColumnarEntity>(
    write: &E::WriteContext<'_>,
    read: &E::ReadContext<'_>,
    check: impl Fn(&E::Value),
) {
    let root = TempDir::new().expect("store directory");
    let store =
        LocalArtifactStore::open(root.path(), StoreId::new("read-roundtrip").unwrap()).unwrap();
    let record = publish_columnar::<F, E>(&store, write, budgets())
        .unwrap()
        .into_record();
    let bytes = store
        .with_verified_reader(&record, |source| {
            let mut bytes = Vec::new();
            source.read_to_end(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        })
        .unwrap();
    let value = read_columnar::<F, E>(Cursor::new(&bytes), &record, read, budgets()).unwrap();
    check(&value);
    let managed = with_verified_columnar_reader(&store, &record, budgets(), |source| {
        source.read::<F, E>(read)
    })
    .unwrap();
    check(&managed);
}

#[test]
fn all_matrix_families_load_values_and_statuses_in_both_formats() {
    let fixture = matrices::matrix_fixture();
    macro_rules! matrix {
        ($entity:ty, $context:ident, $table:expr, $expected:expr) => {{
            let table = $table;
            let context = $context {
                expected: $expected,
                bindings: MultiscaleEmbeddingTableReadBindings::new(
                    MultiscaleArtifactBinding::new(
                        table.expected_entities_artifact_id(),
                        table.expected_entities_logical_digest(),
                    ),
                    MultiscaleArtifactBinding::new(
                        table.support_artifact_id(),
                        table.support_logical_digest(),
                    ),
                    MultiscaleArtifactBinding::new(
                        table.provenance_artifact_id(),
                        table.provenance_logical_digest(),
                    ),
                )
                .unwrap(),
            };
            let check = |loaded: &<$entity as ColumnarEntity>::Value| {
                assert_eq!(loaded.dimension(), table.dimension());
                assert_eq!(loaded.row_count(), table.row_count());
                assert_eq!(loaded.qc_summary(), table.qc_summary());
                for index in 0..table.row_count() {
                    let actual = loaded.row(index).unwrap();
                    let expected = table.row(index).unwrap();
                    assert_eq!(actual.status(), expected.status());
                    assert_eq!(actual.vector(), expected.vector());
                }
            };
            round_trip::<ArrowIpc, $entity>(&table, &context, check);
            round_trip::<Parquet, $entity>(&table, &context, check);
        }};
    }
    matrix!(
        PatchEmbeddingTableEntity,
        PatchEmbeddingTableReadContext,
        &fixture.patch_table,
        &fixture.expected_patches
    );
    matrix!(
        RegionEmbeddingTableEntity,
        RegionEmbeddingTableReadContext,
        &fixture.region_table,
        &fixture.expected_regions
    );
    matrix!(
        SlideEmbeddingTableEntity,
        SlideEmbeddingTableReadContext,
        &fixture.slide_table,
        &fixture.expected_slides
    );
    // 8,193 rows span two Arrow record batches and two Parquet row groups of 8,192 rows.
    let (expected_patches, large_patch_table) = matrices::patch_fixture_with_rows(8_193, 3);
    matrix!(
        PatchEmbeddingTableEntity,
        PatchEmbeddingTableReadContext,
        &large_patch_table,
        &expected_patches
    );
}

fn common(fixture: &cell_patch::CellPatchFixture) -> CellPatchReadContext<'_> {
    CellPatchReadContext {
        hierarchy: &fixture.hierarchy,
        expected_cells: &fixture.expected_cells,
        expected_patches: &fixture.expected_patches,
        context: &fixture.context,
        footprints: &fixture.footprints,
        bindings: &fixture.bindings,
        mode: fixture.link.mode(),
        maximum_candidate_checks: 100,
        maximum_working_bytes: 8 << 20,
    }
}

#[test]
fn spatial_and_split_cell_patch_artifacts_load_with_explicit_support() {
    for mode in [
        cell_patch::CellPatchFixtureMode::Contained,
        cell_patch::CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch::cell_patch_fixture(mode);
        let assignments = CellPatchAssignmentReadContext {
            common: common(&fixture),
            edges: fixture.link.edges(),
        };
        let edges = CellPatchEdgeReadContext {
            common: common(&fixture),
            assignments: fixture.link.assignments(),
        };
        let check = |loaded: &CellPatchLink| {
            assert_eq!(loaded.mode(), fixture.link.mode());
            assert_eq!(loaded.assignments(), fixture.link.assignments());
            assert_eq!(loaded.edges(), fixture.link.edges());
        };
        round_trip::<ArrowIpc, CellPatchAssignmentEntity>(&&fixture.link, &assignments, check);
        round_trip::<Parquet, CellPatchAssignmentEntity>(&&fixture.link, &assignments, check);
        round_trip::<ArrowIpc, CellPatchEdgeEntity>(&&fixture.link, &edges, check);
        round_trip::<Parquet, CellPatchEdgeEntity>(&&fixture.link, &edges, check);
    }
    for fixture in [
        cell_patch::cell_patch_fixture(cell_patch::CellPatchFixtureMode::Contained),
        cell_patch::interpolation_scale_fixture(3),
    ] {
        let read = PatchFootprintReadContext {
            hierarchy: &fixture.hierarchy,
            expected: &fixture.expected_patches,
            expected_patches_artifact_id: fixture.footprints.expected_patches_artifact_id(),
            context: &fixture.context,
            patch_context_artifact_id: fixture.footprints.patch_context_artifact_id(),
        };
        let write = PatchFootprintWriteContext {
            expected: &fixture.expected_patches,
            context: &fixture.context,
            footprints: &fixture.footprints,
        };
        let check = |loaded: &PatchFootprintSet| {
            assert_eq!(loaded.footprints(), fixture.footprints.footprints())
        };
        round_trip::<ArrowIpc, PatchFootprintEntity>(&write, &read, check);
        round_trip::<Parquet, PatchFootprintEntity>(&write, &read, check);
        let overlap = PatchOverlapGraph::derive(
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            fixture.link.patch_footprints_artifact_id(),
            8 << 20,
            8 << 20,
        )
        .unwrap();
        let read = PatchOverlapReadContext {
            expected: &fixture.expected_patches,
            context: &fixture.context,
            footprints: &fixture.footprints,
            patch_footprints_artifact_id: fixture.link.patch_footprints_artifact_id(),
            maximum_working_bytes: 8 << 20,
        };
        let write = PatchOverlapWriteContext {
            expected: &fixture.expected_patches,
            context: &fixture.context,
            footprints: &fixture.footprints,
            overlap: &overlap,
        };
        let check = |loaded: &PatchOverlapGraph| {
            assert_eq!(loaded.edges(), overlap.edges());
            assert_eq!(loaded.patch_count(), fixture.expected_patches.ids().len());
            assert_eq!(loaded.component_count(), overlap.component_count());
            for patch in fixture.expected_patches.ids() {
                assert_eq!(loaded.component_id(patch), overlap.component_id(patch));
            }
        };
        round_trip::<ArrowIpc, PatchOverlapEntity>(&write, &read, check);
        round_trip::<Parquet, PatchOverlapEntity>(&write, &read, check);
    }
}

#[test]
fn row_link_load_requires_explicit_statuses_and_preserves_source_rows() {
    let fixture = cell_patch::cell_patch_fixture(cell_patch::CellPatchFixtureMode::Contained);
    let ids = fixture.expected_cells.cells();
    let read = CellEmbeddingRowLinkReadContext {
        expected: &fixture.expected_cells,
        hierarchy: &fixture.hierarchy,
        statuses: &[
            EmbeddingStatus::Present,
            EmbeddingStatus::MissingVector,
            EmbeddingStatus::QcRejected,
        ],
        source_cells_artifact_id: matrices::artifact_id(b"read-source-cells"),
        source_vectors_artifact_id: matrices::artifact_id(b"read-source-vectors"),
        expected_cells_artifact_id: matrices::artifact_id(b"read-expected-cells"),
        identity_map_artifact_id: matrices::artifact_id(b"read-identity-map"),
        converter_artifact_id: matrices::artifact_id(b"read-converter"),
    };
    let link = CellEmbeddingRowLink::new(
        read.source_cells_artifact_id,
        read.source_vectors_artifact_id,
        read.expected_cells_artifact_id,
        read.identity_map_artifact_id,
        read.converter_artifact_id,
        read.expected,
        read.hierarchy,
        vec![
            CellEmbeddingRowLinkEntry::present(ids[0].clone(), 2, 1),
            CellEmbeddingRowLinkEntry::missing_vector(ids[1].clone(), 0),
            CellEmbeddingRowLinkEntry::qc_rejected(ids[2].clone(), 1, 0),
        ],
        8 << 20,
    )
    .unwrap();
    let check = |loaded: &CellEmbeddingRowLink| assert_eq!(loaded.entries(), link.entries());
    round_trip::<ArrowIpc, CellEmbeddingRowLinkEntity>(&&link, &read, check);
    round_trip::<Parquet, CellEmbeddingRowLinkEntity>(&&link, &read, check);
    let root = TempDir::new().unwrap();
    let store =
        LocalArtifactStore::open(root.path(), StoreId::new("row-statuses").unwrap()).unwrap();
    let record =
        publish_columnar::<ArrowIpc, CellEmbeddingRowLinkEntity>(&store, &&link, budgets())
            .unwrap()
            .into_record();
    let mut canonical = Vec::new();
    write_cell_embedding_row_link_arrow(&mut canonical, &link, budgets()).unwrap();
    let wrong_digest = marklab::workflow::ArtifactRecord::new(
        marklab::workflow::ArtifactRef::new(
            record.content().kind(),
            marklab::workflow::ContentDigest::from_bytes(b"different-content"),
            record.content().byte_len(),
        )
        .unwrap(),
        record.schema().clone(),
        record.table().cloned(),
        record.dependencies().to_vec(),
        record.semantic_metadata().clone(),
        record.locations().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        read_columnar::<ArrowIpc, CellEmbeddingRowLinkEntity>(
            Cursor::new(&canonical),
            &wrong_digest,
            &read,
            budgets()
        ),
        Err(ColumnarError::ArtifactBindingMismatch)
    ));
    assert!(matches!(
        verify_columnar::<ArrowIpc, CellEmbeddingRowLinkEntity>(
            Cursor::new(&canonical),
            &wrong_digest,
            &CellEmbeddingRowLinkVerifyContext {
                expected: read.expected,
                row_link: &link
            },
            budgets()
        ),
        Err(ColumnarError::ArtifactBindingMismatch)
    ));
    let bad = CellEmbeddingRowLinkReadContext {
        statuses: &[EmbeddingStatus::Present; 3],
        ..read
    };
    assert!(matches!(
        with_verified_columnar_reader(&store, &record, budgets(), |source| {
            source.read::<ArrowIpc, CellEmbeddingRowLinkEntity>(&bad)
        }),
        Err(VerifiedReaderError::Callback(_))
    ));
}

#[test]
fn sparse_patch_region_rows_load_with_their_assessment_bindings() {
    let fixture = patch_region::fixture();
    for declarations in [patch_region::declarations(), Vec::new()] {
        let assessment = PatchRegionAssessment::new(
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            declarations,
            8 << 20,
            8 << 20,
        )
        .unwrap();
        let link = PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            matrices::artifact_id(b"read-assessment"),
            marklab::workflow::ContentDigest::from_bytes(&assessment.to_canonical_json().unwrap()),
            8 << 20,
            8 << 20,
        )
        .unwrap();
        let read = PatchRegionReadContext {
            expected_patches: &fixture.expected_patches,
            expected_regions: &fixture.expected_regions,
            context: &fixture.context,
            footprints: &fixture.footprints,
            bindings: &fixture.bindings,
            assessment_artifact_id: link.assessment_artifact_id(),
            assessment_content_digest: link.assessment_content_digest(),
            maximum_working_bytes: 8 << 20,
        };
        let check = |loaded: &PatchRegionLink| {
            assert_eq!(loaded.nonzero_relations(), link.nonzero_relations());
            assert_eq!(loaded.assessed_pair_count(), link.assessed_pair_count());
        };
        round_trip::<ArrowIpc, PatchRegionEntity>(&&link, &read, check);
        round_trip::<Parquet, PatchRegionEntity>(&&link, &read, check);
    }
}
