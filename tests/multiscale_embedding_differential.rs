#![cfg(feature = "parquet")]

#[path = "support/multiscale_matrix_columnar.rs"]
mod support;

use marklab::{
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
    publish_slide_embedding_table_arrow, publish_slide_embedding_table_parquet,
    validate_patch_embedding_table_arrow_bytes, validate_patch_embedding_table_arrow_from_store,
    validate_patch_embedding_table_parquet_bytes,
    validate_patch_embedding_table_parquet_from_store, validate_region_embedding_table_arrow_bytes,
    validate_region_embedding_table_arrow_from_store,
    validate_region_embedding_table_parquet_bytes,
    validate_region_embedding_table_parquet_from_store, validate_slide_embedding_table_arrow_bytes,
    validate_slide_embedding_table_arrow_from_store, validate_slide_embedding_table_parquet_bytes,
    validate_slide_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
    write_patch_embedding_table_parquet, write_region_embedding_table_arrow,
    write_region_embedding_table_parquet, write_slide_embedding_table_arrow,
    write_slide_embedding_table_parquet, ArtifactId, ContentDigest, EmbeddingColumnarBudgets,
    EmbeddingEntityKind, EmbeddingStatus, LocalArtifactStore, MultiscaleEmbeddingQcSummary,
    SlideId, StoreId,
};
use tempfile::TempDir;

use support::{matrix_fixture, patch_table_with_rows, LARGE};

#[derive(Clone, Copy)]
struct ReferenceRow {
    id: &'static str,
    status: EmbeddingStatus,
    vector: Option<&'static [f32]>,
}

#[derive(Clone, Copy)]
struct MatrixCase {
    kind: EmbeddingEntityKind,
    kind_wire: &'static str,
    logical_domain: &'static [u8],
    dimension: u32,
    rows: &'static [ReferenceRow],
}

const PATCH_ROWS: &[ReferenceRow] = &[
    ReferenceRow {
        id: "patch-a",
        status: EmbeddingStatus::Present,
        vector: Some(&[1.0, -0.0, 3.5]),
    },
    ReferenceRow {
        id: "patch-b",
        status: EmbeddingStatus::MissingVector,
        vector: None,
    },
    ReferenceRow {
        id: "patch-c",
        status: EmbeddingStatus::ExtractionFailed,
        vector: None,
    },
    ReferenceRow {
        id: "patch-d",
        status: EmbeddingStatus::QcRejected,
        vector: None,
    },
];

const REGION_ROWS: &[ReferenceRow] = &[
    ReferenceRow {
        id: "region-a",
        status: EmbeddingStatus::Present,
        vector: Some(&[0.0, 0.0, 0.0]),
    },
    ReferenceRow {
        id: "region-b",
        status: EmbeddingStatus::Present,
        vector: Some(&[4.0, 5.0, 6.0]),
    },
];

const SLIDE_ROWS: &[ReferenceRow] = &[ReferenceRow {
    id: "matrix-slide",
    status: EmbeddingStatus::Present,
    vector: Some(&[7.0, 8.0, 9.0]),
}];

const CASES: [MatrixCase; 3] = [
    MatrixCase {
        kind: EmbeddingEntityKind::Patch,
        kind_wire: "patch",
        logical_domain: b"marklab-patch-embedding-logical-v1",
        dimension: 3,
        rows: PATCH_ROWS,
    },
    MatrixCase {
        kind: EmbeddingEntityKind::Region,
        kind_wire: "region",
        logical_domain: b"marklab-region-embedding-logical-v1",
        dimension: 3,
        rows: REGION_ROWS,
    },
    MatrixCase {
        kind: EmbeddingEntityKind::Slide,
        kind_wire: "slide",
        logical_domain: b"marklab-slide-embedding-logical-v1",
        dimension: 3,
        rows: SLIDE_ROWS,
    },
];

#[derive(Debug, Eq, PartialEq)]
struct ReferenceQc {
    kind: EmbeddingEntityKind,
    row_count: u64,
    present: u64,
    missing: u64,
    failed: u64,
    rejected: u64,
    all_zero_present: u64,
    dimension: u32,
    logical_digest: ContentDigest,
}

struct ReferenceBindings<'a> {
    owning_slide_id: &'a SlideId,
    expected_artifact_id: ArtifactId,
    expected_logical_digest: ContentDigest,
    support_artifact_id: ArtifactId,
    support_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
}

fn canonical_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

fn reference_qc(case: MatrixCase, bindings: ReferenceBindings<'_>) -> ReferenceQc {
    let mut fields = vec![
        case.logical_domain.to_vec(),
        case.kind_wire.as_bytes().to_vec(),
        bindings.owning_slide_id.as_str().as_bytes().to_vec(),
        bindings.expected_artifact_id.digest().as_bytes().to_vec(),
        bindings.expected_logical_digest.as_bytes().to_vec(),
        bindings.support_artifact_id.digest().as_bytes().to_vec(),
        bindings.support_logical_digest.as_bytes().to_vec(),
        bindings.provenance_artifact_id.digest().as_bytes().to_vec(),
        bindings.provenance_logical_digest.as_bytes().to_vec(),
        case.dimension.to_be_bytes().to_vec(),
        u64::try_from(case.rows.len())
            .expect("reference row count")
            .to_be_bytes()
            .to_vec(),
    ];
    let mut reference = ReferenceQc {
        kind: case.kind,
        row_count: u64::try_from(case.rows.len()).expect("reference row count"),
        present: 0,
        missing: 0,
        failed: 0,
        rejected: 0,
        all_zero_present: 0,
        dimension: case.dimension,
        logical_digest: ContentDigest::from_bytes(&[]),
    };

    for row in case.rows {
        fields.push(row.id.as_bytes().to_vec());
        let status_wire = match row.status {
            EmbeddingStatus::Present => {
                reference.present += 1;
                "present"
            }
            EmbeddingStatus::MissingVector => {
                reference.missing += 1;
                "missing_vector"
            }
            EmbeddingStatus::ExtractionFailed => {
                reference.failed += 1;
                "extraction_failed"
            }
            EmbeddingStatus::QcRejected => {
                reference.rejected += 1;
                "qc_rejected"
            }
        };
        fields.push(status_wire.as_bytes().to_vec());
        match (row.status, row.vector) {
            (EmbeddingStatus::Present, Some(vector)) => {
                assert_eq!(vector.len(), case.dimension as usize);
                reference.all_zero_present +=
                    u64::from(vector.iter().all(|value| canonical_f32_bits(*value) == 0));
                fields.extend(
                    vector
                        .iter()
                        .map(|value| canonical_f32_bits(*value).to_be_bytes().to_vec()),
                );
            }
            (EmbeddingStatus::Present, None) | (_, Some(_)) => {
                panic!("invalid declarative reference row")
            }
            (_, None) => {}
        }
    }
    reference.logical_digest = ContentDigest::from_framed(fields.iter().map(Vec::as_slice));
    reference
}

fn assert_summary(summary: MultiscaleEmbeddingQcSummary, reference: &ReferenceQc) {
    assert_eq!(summary.entity_kind(), reference.kind);
    assert_eq!(summary.row_count(), reference.row_count);
    assert_eq!(summary.present_count(), reference.present);
    assert_eq!(summary.missing_vector_count(), reference.missing);
    assert_eq!(summary.extraction_failed_count(), reference.failed);
    assert_eq!(summary.qc_rejected_count(), reference.rejected);
    assert_eq!(summary.all_zero_present_count(), reference.all_zero_present);
    assert_eq!(summary.dimension(), reference.dimension);
    assert_eq!(summary.logical_digest(), reference.logical_digest);
}

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, LARGE as u64)
}

macro_rules! assert_table_reference {
    ($table:expr, $case:expr, $id_method:ident) => {{
        let table = $table;
        let case = $case;
        let reference = reference_qc(
            case,
            ReferenceBindings {
                owning_slide_id: table.owning_slide_id(),
                expected_artifact_id: table.expected_entities_artifact_id(),
                expected_logical_digest: table.expected_entities_logical_digest(),
                support_artifact_id: table.support_artifact_id(),
                support_logical_digest: table.support_logical_digest(),
                provenance_artifact_id: table.provenance_artifact_id(),
                provenance_logical_digest: table.provenance_logical_digest(),
            },
        );
        assert_eq!(table.entity_kind(), case.kind);
        assert_eq!(table.row_count(), case.rows.len());
        assert_eq!(table.dimension(), case.dimension);
        for (index, expected) in case.rows.iter().enumerate() {
            let observed = table.row(index).expect("typed reference row");
            assert_eq!(observed.$id_method().as_str(), expected.id);
            assert_eq!(observed.status(), expected.status);
            assert_eq!(
                observed.vector().map(|values| {
                    values
                        .iter()
                        .map(|value| canonical_f32_bits(*value))
                        .collect::<Vec<_>>()
                }),
                expected.vector.map(|values| {
                    values
                        .iter()
                        .map(|value| canonical_f32_bits(*value))
                        .collect::<Vec<_>>()
                })
            );
        }
        assert_summary(table.qc_summary(), &reference);
        for maximum_block_rows in [1, 2, 3, 5, 8_192] {
            assert_summary(
                table
                    .scan_qc(maximum_block_rows)
                    .expect("partitioned table scan"),
                &reference,
            );
        }
        reference
    }};
}

macro_rules! assert_physical_reference {
    ($store:expr, $table:expr, $reference:expr, $writer:path, $publisher:path,
        $borrowed:path, $managed:path) => {{
        let table = $table;
        let mut bytes = Vec::new();
        $writer(&mut bytes, table, budgets()).expect("differential physical write");
        let record = $publisher($store, table, budgets())
            .expect("differential physical publication")
            .into_record();
        let borrowed =
            $borrowed(&bytes, &record, table, budgets()).expect("borrowed differential validation");
        let managed =
            $managed($store, &record, table, budgets()).expect("managed differential validation");
        assert_eq!(borrowed, managed);
        assert_summary(borrowed.qc_summary(), $reference);
    }};
}

#[test]
fn declarative_reference_matches_all_typed_partition_and_physical_paths() {
    let fixture = matrix_fixture();
    assert_eq!(
        fixture.expected_patches.ids().len(),
        fixture.patch_table.row_count()
    );
    assert_eq!(
        fixture.expected_regions.ids().len(),
        fixture.region_table.row_count()
    );
    assert_eq!(
        fixture.expected_slides.ids().len(),
        fixture.slide_table.row_count()
    );
    let empty_patch_table = patch_table_with_rows(0, 3);
    assert_eq!(
        empty_patch_table
            .scan_qc(1)
            .expect("empty partitioned scan"),
        empty_patch_table.qc_summary()
    );
    let patch_reference = assert_table_reference!(&fixture.patch_table, CASES[0], patch_id);
    let region_reference = assert_table_reference!(&fixture.region_table, CASES[1], region_id);
    let slide_reference = assert_table_reference!(&fixture.slide_table, CASES[2], slide_id);

    let root = TempDir::new().expect("differential store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("multiscale-embedding-differential").expect("store ID"),
    )
    .expect("differential store");

    assert_physical_reference!(
        &store,
        &fixture.patch_table,
        &patch_reference,
        write_patch_embedding_table_arrow,
        publish_patch_embedding_table_arrow,
        validate_patch_embedding_table_arrow_bytes,
        validate_patch_embedding_table_arrow_from_store
    );
    assert_physical_reference!(
        &store,
        &fixture.patch_table,
        &patch_reference,
        write_patch_embedding_table_parquet,
        publish_patch_embedding_table_parquet,
        validate_patch_embedding_table_parquet_bytes,
        validate_patch_embedding_table_parquet_from_store
    );
    assert_physical_reference!(
        &store,
        &fixture.region_table,
        &region_reference,
        write_region_embedding_table_arrow,
        publish_region_embedding_table_arrow,
        validate_region_embedding_table_arrow_bytes,
        validate_region_embedding_table_arrow_from_store
    );
    assert_physical_reference!(
        &store,
        &fixture.region_table,
        &region_reference,
        write_region_embedding_table_parquet,
        publish_region_embedding_table_parquet,
        validate_region_embedding_table_parquet_bytes,
        validate_region_embedding_table_parquet_from_store
    );
    assert_physical_reference!(
        &store,
        &fixture.slide_table,
        &slide_reference,
        write_slide_embedding_table_arrow,
        publish_slide_embedding_table_arrow,
        validate_slide_embedding_table_arrow_bytes,
        validate_slide_embedding_table_arrow_from_store
    );
    assert_physical_reference!(
        &store,
        &fixture.slide_table,
        &slide_reference,
        write_slide_embedding_table_parquet,
        publish_slide_embedding_table_parquet,
        validate_slide_embedding_table_parquet_bytes,
        validate_slide_embedding_table_parquet_from_store
    );
}
