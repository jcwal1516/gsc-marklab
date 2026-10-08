#![cfg(feature = "parquet")]

#[path = "support/cell_patch_columnar.rs"]
mod support;

use std::{fs, io, io::Write, process::Command};

use marklab::{
    preflight_cell_patch_assignment_table_arrow_bytes,
    preflight_cell_patch_assignment_table_parquet_bytes,
    preflight_cell_patch_edge_table_arrow_bytes, preflight_cell_patch_edge_table_parquet_bytes,
    publish_cell_patch_assignment_table_arrow, publish_cell_patch_assignment_table_parquet,
    publish_cell_patch_edge_table_arrow, publish_cell_patch_edge_table_parquet,
    validate_cell_patch_assignment_table_arrow_bytes,
    validate_cell_patch_assignment_table_arrow_from_store,
    validate_cell_patch_assignment_table_parquet_bytes,
    validate_cell_patch_assignment_table_parquet_from_store,
    validate_cell_patch_edge_table_arrow_bytes, validate_cell_patch_edge_table_arrow_from_store,
    validate_cell_patch_edge_table_parquet_bytes,
    validate_cell_patch_edge_table_parquet_from_store, write_cell_patch_assignment_table_arrow,
    write_cell_patch_assignment_table_parquet, write_cell_patch_edge_table_arrow,
    write_cell_patch_edge_table_parquet, ArtifactRecord, ContentDigest, EmbeddingColumnarBudgets,
    LocalArtifactStore, PublicationDisposition, StoreId, TableColumnType, TableFormat,
    TableScalarType,
};
use support::{cell_patch_fixture, CellPatchFixtureMode};
use tempfile::TempDir;

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

#[derive(Default)]
struct FragmentingWriter {
    bytes: Vec<u8>,
    calls: usize,
}

impl Write for FragmentingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        const FRAGMENTS: [usize; 7] = [1, 3, 2, 7, 5, 11, 4];
        let maximum = FRAGMENTS[self.calls % FRAGMENTS.len()];
        self.calls += 1;
        let written = buffer.len().min(maximum);
        self.bytes.extend_from_slice(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn assert_exact_manifests(
    assignments: &ArtifactRecord,
    edges: &ArtifactRecord,
    format: TableFormat,
    assignment_encoding: &str,
    edge_encoding: &str,
) {
    assert_eq!(
        assignments.schema().id(),
        "marklab.cell_patch_assignment_table"
    );
    assert_eq!(assignments.schema().version(), 1);
    let assignment = assignments.table().expect("assignment manifest");
    assert_eq!(assignment.format(), format);
    assert_eq!(assignment.encoding_version(), assignment_encoding);
    assert_eq!(assignment.row_count(), 3);
    assert_eq!(assignment.columns().len(), 6);
    for (index, (name, scalar)) in [
        ("cell_id", TableScalarType::Utf8),
        ("assignment_status", TableScalarType::Utf8),
        ("anchor_x_bits", TableScalarType::U64),
        ("anchor_y_bits", TableScalarType::U64),
        ("edge_start", TableScalarType::U64),
        ("edge_count", TableScalarType::U64),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(assignment.columns()[index].name(), name);
        assert_eq!(
            assignment.columns()[index].column_type(),
            &TableColumnType::Scalar(scalar)
        );
        assert!(!assignment.columns()[index].nullable());
    }
    assert_eq!(assignment.primary_key(), ["cell_id"]);

    assert_eq!(edges.schema().id(), "marklab.cell_patch_edge_table");
    assert_eq!(edges.schema().version(), 1);
    let edge = edges.table().expect("edge manifest");
    assert_eq!(edge.format(), format);
    assert_eq!(edge.encoding_version(), edge_encoding);
    assert_eq!(edge.row_count(), 3);
    assert_eq!(edge.columns().len(), 4);
    for (index, (name, nullable)) in [
        ("assignment_row", false),
        ("patch_id", false),
        ("weight_numerator", true),
        ("weight_denominator", true),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(edge.columns()[index].name(), name);
        assert_eq!(
            edge.columns()[index].column_type(),
            &TableColumnType::Scalar(if index == 1 {
                TableScalarType::Utf8
            } else {
                TableScalarType::U64
            })
        );
        assert_eq!(edge.columns()[index].nullable(), nullable);
    }
    assert_eq!(edge.primary_key(), ["assignment_row", "patch_id"]);
}

fn assert_exact_dependencies(record: &ArtifactRecord, fixture: &support::CellPatchFixture) {
    let mut expected = [
        fixture.link.expected_cells_artifact_id(),
        fixture.link.expected_patches_artifact_id(),
        fixture.link.patch_context_artifact_id(),
        fixture.link.patch_footprints_artifact_id(),
        fixture.link.producer_artifact_id(),
    ];
    expected.sort_unstable();
    assert_eq!(record.dependencies(), expected);
    assert!(record.semantic_metadata().is_empty());
}

#[test]
fn cell_patch_arrow_writers_and_raw_preflights_preserve_both_modes() {
    let mut encoded_by_mode = Vec::new();
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        let mut assignments = Vec::new();
        let assignment_summary =
            write_cell_patch_assignment_table_arrow(&mut assignments, &fixture.link, budgets())
                .expect("write assignment Arrow");
        let mut edges = Vec::new();
        let edge_summary = write_cell_patch_edge_table_arrow(&mut edges, &fixture.link, budgets())
            .expect("write edge Arrow");

        assert_eq!(assignment_summary.row_count(), 3);
        assert_eq!(edge_summary.row_count(), 3);
        assert_eq!(
            assignment_summary.content_digest(),
            ContentDigest::from_bytes(&assignments)
        );
        assert_eq!(
            edge_summary.content_digest(),
            ContentDigest::from_bytes(&edges)
        );
        assert_eq!(
            assignment_summary.encoded_byte_len(),
            assignments.len() as u64
        );
        assert_eq!(edge_summary.encoded_byte_len(), edges.len() as u64);

        let assignment_preflight = preflight_cell_patch_assignment_table_arrow_bytes(
            &assignments,
            &fixture.link,
            budgets(),
        )
        .expect("preflight assignment Arrow");
        let edge_preflight =
            preflight_cell_patch_edge_table_arrow_bytes(&edges, &fixture.link, budgets())
                .expect("preflight edge Arrow");
        assert_eq!(assignment_preflight.row_count(), 3);
        assert_eq!(edge_preflight.row_count(), 3);
        assert_eq!(assignment_preflight.record_batch_count(), 1);
        assert_eq!(edge_preflight.record_batch_count(), 1);
        assert_eq!(
            assignment_preflight.content_digest(),
            assignment_summary.content_digest()
        );
        assert_eq!(
            edge_preflight.content_digest(),
            edge_summary.content_digest()
        );
        encoded_by_mode.push((assignments, edges));
    }

    assert_ne!(encoded_by_mode[0].0, encoded_by_mode[1].0);
    assert_ne!(encoded_by_mode[0].1, encoded_by_mode[1].1);
}

#[test]
fn cell_patch_arrow_publications_have_exact_records_and_full_reader_parity() {
    let fixture = cell_patch_fixture(CellPatchFixtureMode::Interpolation);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("cell-patch-arrow").expect("store ID"),
    )
    .expect("store");
    let assignments = publish_cell_patch_assignment_table_arrow(&store, &fixture.link, budgets())
        .expect("publish assignments")
        .into_record();
    let edges = publish_cell_patch_edge_table_arrow(&store, &fixture.link, budgets())
        .expect("publish edges")
        .into_record();

    assert_eq!(
        assignments.schema().id(),
        "marklab.cell_patch_assignment_table"
    );
    assert_eq!(
        assignments.content().kind(),
        "application/vnd.marklab.cell-patch-assignment-table.v1+arrow"
    );
    assert_eq!(
        assignments.table().expect("assignment manifest").format(),
        TableFormat::ArrowIpcFile
    );
    assert_eq!(assignments.table().expect("manifest").row_count(), 3);
    assert_eq!(edges.schema().id(), "marklab.cell_patch_edge_table");
    assert_eq!(
        edges.content().kind(),
        "application/vnd.marklab.cell-patch-edge-table.v1+arrow"
    );
    assert_eq!(
        edges.table().expect("edge manifest").format(),
        TableFormat::ArrowIpcFile
    );
    assert_eq!(edges.table().expect("manifest").row_count(), 3);
    assert_eq!(assignments.dependencies(), edges.dependencies());
    assert_eq!(assignments.dependencies().len(), 5);
    assert_ne!(assignments.id(), edges.id());
    assert_exact_manifests(
        &assignments,
        &edges,
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.cell-patch-assignment-table.v1",
        "marklab.arrow-ipc.cell-patch-edge-table.v1",
    );
    assert_exact_dependencies(&assignments, &fixture);
    assert_exact_dependencies(&edges, &fixture);
    let repeated = publish_cell_patch_assignment_table_arrow(&store, &fixture.link, budgets())
        .expect("repeat assignment publication");
    assert_eq!(
        repeated.disposition(),
        PublicationDisposition::AlreadyPresent
    );
    assert_eq!(repeated.record().id(), assignments.id());

    let mut assignment_bytes = Vec::new();
    write_cell_patch_assignment_table_arrow(&mut assignment_bytes, &fixture.link, budgets())
        .expect("assignment bytes");
    let mut edge_bytes = Vec::new();
    write_cell_patch_edge_table_arrow(&mut edge_bytes, &fixture.link, budgets())
        .expect("edge bytes");
    let borrowed_assignments = validate_cell_patch_assignment_table_arrow_bytes(
        &assignment_bytes,
        &assignments,
        &fixture.link,
        budgets(),
    )
    .expect("borrowed assignments");
    let managed_assignments = validate_cell_patch_assignment_table_arrow_from_store(
        &store,
        &assignments,
        &fixture.link,
        budgets(),
    )
    .expect("managed assignments");
    assert_eq!(borrowed_assignments, managed_assignments);
    let borrowed_edges =
        validate_cell_patch_edge_table_arrow_bytes(&edge_bytes, &edges, &fixture.link, budgets())
            .expect("borrowed edges");
    let managed_edges =
        validate_cell_patch_edge_table_arrow_from_store(&store, &edges, &fixture.link, budgets())
            .expect("managed edges");
    assert_eq!(borrowed_edges, managed_edges);
}

#[test]
fn cell_patch_parquet_writers_and_raw_preflights_preserve_both_modes() {
    let mut encoded_by_mode = Vec::new();
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        let mut assignments = Vec::new();
        let assignment_summary =
            write_cell_patch_assignment_table_parquet(&mut assignments, &fixture.link, budgets())
                .expect("write assignment Parquet");
        let mut edges = Vec::new();
        let edge_summary =
            write_cell_patch_edge_table_parquet(&mut edges, &fixture.link, budgets())
                .expect("write edge Parquet");
        assert_eq!(assignment_summary.row_count(), 3);
        assert_eq!(edge_summary.row_count(), 3);
        assert_eq!(
            assignment_summary.content_digest(),
            ContentDigest::from_bytes(&assignments)
        );
        assert_eq!(
            edge_summary.content_digest(),
            ContentDigest::from_bytes(&edges)
        );

        let assignment_preflight = preflight_cell_patch_assignment_table_parquet_bytes(
            &assignments,
            &fixture.link,
            budgets(),
        )
        .expect("preflight assignment Parquet");
        let edge_preflight =
            preflight_cell_patch_edge_table_parquet_bytes(&edges, &fixture.link, budgets())
                .expect("preflight edge Parquet");
        assert_eq!(assignment_preflight.row_count(), 3);
        assert_eq!(edge_preflight.row_count(), 3);
        assert_eq!(assignment_preflight.row_group_count(), 1);
        assert_eq!(edge_preflight.row_group_count(), 1);
        assert_eq!(
            assignment_preflight.content_digest(),
            assignment_summary.content_digest()
        );
        assert_eq!(
            edge_preflight.content_digest(),
            edge_summary.content_digest()
        );
        encoded_by_mode.push((assignments, edges));
    }
    assert_ne!(encoded_by_mode[0].0, encoded_by_mode[1].0);
    assert_ne!(encoded_by_mode[0].1, encoded_by_mode[1].1);
}

#[test]
fn cell_patch_parquet_publications_have_exact_records_and_full_reader_parity() {
    let fixture = cell_patch_fixture(CellPatchFixtureMode::Interpolation);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("cell-patch-parquet").expect("store ID"),
    )
    .expect("store");
    let assignments = publish_cell_patch_assignment_table_parquet(&store, &fixture.link, budgets())
        .expect("publish assignments")
        .into_record();
    let edges = publish_cell_patch_edge_table_parquet(&store, &fixture.link, budgets())
        .expect("publish edges")
        .into_record();
    assert_eq!(
        assignments.content().kind(),
        "application/vnd.marklab.cell-patch-assignment-table.v1+parquet"
    );
    assert_eq!(
        assignments.table().expect("manifest").format(),
        TableFormat::ParquetFile
    );
    assert_eq!(
        edges.content().kind(),
        "application/vnd.marklab.cell-patch-edge-table.v1+parquet"
    );
    assert_eq!(
        edges.table().expect("manifest").format(),
        TableFormat::ParquetFile
    );
    assert_eq!(assignments.dependencies(), edges.dependencies());
    assert_exact_manifests(
        &assignments,
        &edges,
        TableFormat::ParquetFile,
        "marklab.parquet.cell-patch-assignment-table.v1",
        "marklab.parquet.cell-patch-edge-table.v1",
    );
    assert_exact_dependencies(&assignments, &fixture);
    assert_exact_dependencies(&edges, &fixture);
    let repeated = publish_cell_patch_edge_table_parquet(&store, &fixture.link, budgets())
        .expect("repeat edge publication");
    assert_eq!(
        repeated.disposition(),
        PublicationDisposition::AlreadyPresent
    );
    assert_eq!(repeated.record().id(), edges.id());

    let mut assignment_bytes = Vec::new();
    write_cell_patch_assignment_table_parquet(&mut assignment_bytes, &fixture.link, budgets())
        .expect("assignment bytes");
    let mut edge_bytes = Vec::new();
    write_cell_patch_edge_table_parquet(&mut edge_bytes, &fixture.link, budgets())
        .expect("edge bytes");
    let borrowed_assignments = validate_cell_patch_assignment_table_parquet_bytes(
        &assignment_bytes,
        &assignments,
        &fixture.link,
        budgets(),
    )
    .expect("borrowed assignments");
    let managed_assignments = validate_cell_patch_assignment_table_parquet_from_store(
        &store,
        &assignments,
        &fixture.link,
        budgets(),
    )
    .expect("managed assignments");
    assert_eq!(borrowed_assignments, managed_assignments);
    let borrowed_edges =
        validate_cell_patch_edge_table_parquet_bytes(&edge_bytes, &edges, &fixture.link, budgets())
            .expect("borrowed edges");
    let managed_edges =
        validate_cell_patch_edge_table_parquet_from_store(&store, &edges, &fixture.link, budgets())
            .expect("managed edges");
    assert_eq!(borrowed_edges, managed_edges);
}

#[test]
fn cell_patch_writers_are_deterministic_and_support_fragmented_sinks() {
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        macro_rules! assert_writer {
            ($writer:path) => {{
                let mut first = Vec::new();
                $writer(&mut first, &fixture.link, budgets()).expect("first canonical write");
                let mut second = Vec::new();
                $writer(&mut second, &fixture.link, budgets()).expect("second canonical write");
                let mut fragmented = FragmentingWriter::default();
                $writer(&mut fragmented, &fixture.link, budgets())
                    .expect("fragmented canonical write");
                assert_eq!(first, second);
                assert_eq!(first, fragmented.bytes);
            }};
        }
        assert_writer!(write_cell_patch_assignment_table_arrow);
        assert_writer!(write_cell_patch_edge_table_arrow);
        assert_writer!(write_cell_patch_assignment_table_parquet);
        assert_writer!(write_cell_patch_edge_table_parquet);
    }
}

#[test]
fn cell_patch_writers_are_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("test executable");
    let first = temporary.path().join("first.bin");
    let second = temporary.path().join("second.bin");
    for path in [&first, &second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("cell_patch_determinism_child")
            .env("MARKLAB_CELL_PATCH_COLUMNAR_CHILD", path)
            .status()
            .expect("fresh child");
        assert!(status.success());
    }
    assert_eq!(
        fs::read(first).expect("first child output"),
        fs::read(second).expect("second child output")
    );
}

#[test]
fn cell_patch_determinism_child() {
    let Some(path) = std::env::var_os("MARKLAB_CELL_PATCH_COLUMNAR_CHILD") else {
        return;
    };
    let fixture = cell_patch_fixture(CellPatchFixtureMode::Interpolation);
    let mut combined = Vec::new();
    macro_rules! append_writer {
        ($writer:path) => {{
            let mut bytes = Vec::new();
            $writer(&mut bytes, &fixture.link, budgets()).expect("child canonical write");
            combined.extend_from_slice(
                &u64::try_from(bytes.len())
                    .expect("encoded length")
                    .to_le_bytes(),
            );
            combined.extend_from_slice(&bytes);
        }};
    }
    append_writer!(write_cell_patch_assignment_table_arrow);
    append_writer!(write_cell_patch_edge_table_arrow);
    append_writer!(write_cell_patch_assignment_table_parquet);
    append_writer!(write_cell_patch_edge_table_parquet);
    fs::write(path, combined).expect("child output");
}
