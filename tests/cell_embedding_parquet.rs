#![cfg(feature = "parquet")]

use std::{
    fs,
    io::{self, Write},
    process::Command,
    str::FromStr,
};

use marklab::{
    preflight_cell_embedding_table_arrow_bytes, preflight_cell_embedding_table_parquet_bytes,
    publish_cell_embedding_table_parquet, write_cell_embedding_table_arrow,
    write_cell_embedding_table_parquet, ArtifactId, CellEmbeddingRow, CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings, CellId, ContentDigest, EmbeddingColumnarBudgets,
    EmbeddingColumnarError, EmbeddingStatus, ExpectedCellSet, LocalArtifactStore, ParquetFailure,
    PublicationDisposition, StoreId, TableFormat,
};
use proptest::prelude::*;
use tempfile::TempDir;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn fixture() -> (
    ExpectedCellSet,
    CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings,
) {
    let expected = ExpectedCellSet::new("all.v1", vec![cell("cell-a"), cell("cell-b")])
        .expect("expected cells");
    let expected_id = artifact_id(b"parquet-expected");
    let provenance_id = artifact_id(b"parquet-provenance");
    let row_link_id = artifact_id(b"parquet-row-link");
    let row_link_digest = ContentDigest::from_bytes(b"parquet-row-link-logical");
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, 0.0, 3.5]),
            CellEmbeddingRow::non_present(cell("cell-b"), EmbeddingStatus::MissingVector)
                .expect("missing row"),
        ],
        1024 * 1024,
    )
    .expect("table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_id,
        provenance_id,
        row_link_id,
        row_link_digest,
        table.qc_summary().logical_digest(),
    )
    .expect("bindings");
    (expected, table, bindings)
}

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
}

fn canonical_bytes() -> (
    ExpectedCellSet,
    CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings,
    Vec<u8>,
) {
    let (expected, table, bindings) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_table_parquet(&mut bytes, &table, bindings, budgets())
        .expect("write Parquet");
    (expected, table, bindings, bytes)
}

fn fixture_with_rows(
    row_count: usize,
) -> (
    ExpectedCellSet,
    CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings,
) {
    let cells = (0..row_count)
        .map(|index| cell(&format!("cell-{index:05}")))
        .collect::<Vec<_>>();
    let expected = ExpectedCellSet::new("row-groups.v1", cells.clone()).expect("expected cells");
    let expected_id = artifact_id(b"row-groups-expected");
    let provenance_id = artifact_id(b"row-groups-provenance");
    let row_link_id = artifact_id(b"row-groups-row-link");
    let row_link_digest = ContentDigest::from_bytes(b"row-groups-row-link-logical");
    let rows = cells
        .into_iter()
        .enumerate()
        .map(|(index, cell_id)| {
            if index % 5 == 0 {
                CellEmbeddingRow::non_present(cell_id, EmbeddingStatus::MissingVector)
                    .expect("missing row")
            } else {
                CellEmbeddingRow::present(cell_id, vec![index as f32, 0.0, 1.0])
            }
        })
        .collect();
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        rows,
        16 * 1024 * 1024,
    )
    .expect("table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_id,
        provenance_id,
        row_link_id,
        row_link_digest,
        table.qc_summary().logical_digest(),
    )
    .expect("bindings");
    (expected, table, bindings)
}

fn scale_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        256 * 1024 * 1024,
        128 * 1024 * 1024,
        128 * 1024 * 1024,
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

fn footer_start(bytes: &[u8]) -> usize {
    let length_offset = bytes.len() - 8;
    let length = u32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    length_offset - length
}

#[test]
fn canonical_embedding_parquet_writer_is_deterministic_and_preflights() {
    let (expected, table, bindings) = fixture();
    let mut first = Vec::new();
    let first_summary = write_cell_embedding_table_parquet(&mut first, &table, bindings, budgets())
        .expect("write Parquet");
    let mut second = Vec::new();
    let second_summary =
        write_cell_embedding_table_parquet(&mut second, &table, bindings, budgets())
            .expect("repeat Parquet");

    assert_eq!(second, first);
    assert_eq!(second_summary, first_summary);
    assert_eq!(&first[..4], b"PAR1");
    assert_eq!(&first[first.len() - 4..], b"PAR1");
    assert_eq!(first_summary.encoded_byte_len(), first.len() as u64);
    assert_eq!(
        first_summary.content_digest(),
        ContentDigest::from_bytes(&first)
    );
    assert_eq!(
        first_summary.content_digest().to_string(),
        "7a54901c82e241e467c04cc6bd58a4c1ae88f27171656d7db01aeb0a499e9b88"
    );
    assert_eq!(first_summary.row_count(), 2);
    assert_eq!(first_summary.dimension(), 3);

    let preflight = preflight_cell_embedding_table_parquet_bytes(
        &first,
        &expected,
        table.dimension(),
        bindings,
        budgets(),
    )
    .expect("preflight Parquet");
    assert_eq!(preflight.row_count(), 2);
    assert_eq!(preflight.dimension(), 3);
    assert_eq!(preflight.row_group_count(), 1);
    assert_eq!(preflight.encoded_byte_len(), first.len() as u64);
    assert_eq!(preflight.content_digest(), first_summary.content_digest());
}

#[test]
fn canonical_embedding_parquet_writer_is_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("current test executable");
    let first_path = temporary.path().join("first.parquet");
    let second_path = temporary.path().join("second.parquet");
    for path in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("canonical_embedding_parquet_writer_child")
            .env("MARKLAB_EMBEDDING_PARQUET_CHILD_OUTPUT", path)
            .status()
            .expect("run fresh Parquet writer");
        assert!(status.success());
    }
    let first = fs::read(first_path).expect("first child output");
    let second = fs::read(second_path).expect("second child output");
    assert_eq!(first, second);
    assert_eq!(
        ContentDigest::from_bytes(&first).to_string(),
        "7a54901c82e241e467c04cc6bd58a4c1ae88f27171656d7db01aeb0a499e9b88"
    );
}

#[test]
fn canonical_embedding_parquet_writer_child() {
    let Some(path) = std::env::var_os("MARKLAB_EMBEDDING_PARQUET_CHILD_OUTPUT") else {
        return;
    };
    let (_, table, bindings) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_table_parquet(&mut bytes, &table, bindings, budgets())
        .expect("child Parquet write");
    fs::write(path, bytes).expect("write child output");
}

#[test]
fn parquet_preflight_rejects_wrong_compact_wire_types_before_stock_decode() {
    let (expected, table, bindings, canonical) = canonical_bytes();

    let mut footer_wrong_type = canonical.clone();
    let footer = footer_start(&footer_wrong_type);
    assert_eq!(footer_wrong_type[footer] & 0x0f, 5, "version is i32");
    footer_wrong_type[footer] = (footer_wrong_type[footer] & 0xf0) | 8;
    assert!(matches!(
        preflight_cell_embedding_table_parquet_bytes(
            &footer_wrong_type,
            &expected,
            table.dimension(),
            bindings,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidFooter,
        })
    ));

    let mut page_wrong_type = canonical;
    assert_eq!(page_wrong_type[4] & 0x0f, 5, "page type is i32");
    page_wrong_type[4] = (page_wrong_type[4] & 0xf0) | 8;
    assert!(matches!(
        preflight_cell_embedding_table_parquet_bytes(
            &page_wrong_type,
            &expected,
            table.dimension(),
            bindings,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidPageHeader,
        })
    ));
}

#[test]
fn parquet_writer_charges_fixed_list_levels_chunks_and_metadata() {
    let row_count = 128_usize;
    let dimension = 1_280_u32;
    let cells = (0..row_count)
        .map(|index| cell(&format!("cell-{index:04}")))
        .collect::<Vec<_>>();
    let expected = ExpectedCellSet::new("writer-memory.v1", cells.clone()).expect("expected cells");
    let expected_id = artifact_id(b"writer-memory-expected");
    let provenance_id = artifact_id(b"writer-memory-provenance");
    let row_link_id = artifact_id(b"writer-memory-row-link");
    let row_link_digest = ContentDigest::from_bytes(b"writer-memory-row-link-logical");
    let rows = cells
        .into_iter()
        .map(|cell_id| CellEmbeddingRow::present(cell_id, vec![1.0; dimension as usize]))
        .collect();
    let table = CellEmbeddingTable::from_rows(
        dimension,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        rows,
        8 * 1024 * 1024,
    )
    .expect("table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_id,
        provenance_id,
        row_link_id,
        row_link_digest,
        table.qc_summary().logical_digest(),
    )
    .expect("bindings");
    let limits = EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        6 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    );

    assert!(matches!(
        write_cell_embedding_table_parquet(&mut Vec::new(), &table, bindings, limits),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn canonical_embedding_parquet_writer_honors_fragmenting_sinks() {
    for row_count in [0, 1, 8_193] {
        let (expected, table, bindings) = fixture_with_rows(row_count);
        let mut contiguous = Vec::new();
        let contiguous_summary =
            write_cell_embedding_table_parquet(&mut contiguous, &table, bindings, scale_budgets())
                .expect("contiguous Parquet");
        let mut fragmented = FragmentingWriter::default();
        let fragmented_summary =
            write_cell_embedding_table_parquet(&mut fragmented, &table, bindings, scale_budgets())
                .expect("fragmented Parquet");
        assert_eq!(fragmented.bytes, contiguous);
        assert_eq!(fragmented_summary, contiguous_summary);
        preflight_cell_embedding_table_parquet_bytes(
            &fragmented.bytes,
            &expected,
            table.dimension(),
            bindings,
            scale_budgets(),
        )
        .expect("fragmented preflight");
    }
}

#[test]
fn canonical_parquet_publisher_creates_only_the_managed_record_and_is_idempotent() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("embedding-parquet-store").expect("store ID"),
    )
    .expect("store");
    let (_, table, bindings) = fixture();

    let first = publish_cell_embedding_table_parquet(&store, &table, bindings, budgets())
        .expect("publish Parquet");
    assert_eq!(first.disposition(), PublicationDisposition::Created);
    assert_eq!(first.record().locations().len(), 1);
    assert_eq!(first.record().locations()[0].store_id(), store.store_id());
    assert_eq!(first.record().schema().id(), "marklab.cell_embedding_table");
    assert_eq!(
        first.record().content().kind(),
        "application/vnd.marklab.cell-embedding-table.v1+parquet"
    );
    assert_eq!(
        first.record().table().expect("table manifest").format(),
        TableFormat::ParquetFile
    );
    store.verify(first.record()).expect("verify Parquet");

    let second = publish_cell_embedding_table_parquet(&store, &table, bindings, budgets())
        .expect("reuse Parquet");
    assert_eq!(second.disposition(), PublicationDisposition::AlreadyPresent);
    assert_eq!(second.record(), first.record());
}

#[test]
fn canonical_parquet_writer_uses_exact_empty_and_8192_row_group_boundaries() {
    for (row_count, expected_groups) in [(0, 0), (1, 1), (8_191, 1), (8_192, 1), (8_193, 2)] {
        let (expected, table, bindings) = fixture_with_rows(row_count);
        let mut bytes = Vec::new();
        let summary =
            write_cell_embedding_table_parquet(&mut bytes, &table, bindings, scale_budgets())
                .expect("write boundary table");
        let preflight = preflight_cell_embedding_table_parquet_bytes(
            &bytes,
            &expected,
            table.dimension(),
            bindings,
            scale_budgets(),
        )
        .expect("preflight boundary table");
        assert_eq!(summary.row_count(), row_count as u64);
        assert_eq!(preflight.row_group_count(), expected_groups);
        if row_count == 8_192 {
            assert_eq!(
                summary.content_digest().to_string(),
                "1a8c42b5d65d70fdbc40b0d44ca748a982e929b90956347d17fd0c25a5ea6a27"
            );
        }
    }
}

#[test]
fn parquet_preflight_enforces_exact_file_row_group_decoded_and_retained_edges() {
    let (expected, table, bindings, bytes) = canonical_bytes();
    let exact_file = EmbeddingColumnarBudgets::new(
        bytes.len() as u64,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    preflight_cell_embedding_table_parquet_bytes(
        &bytes,
        &expected,
        table.dimension(),
        bindings,
        exact_file,
    )
    .expect("exact file budget");
    assert!(matches!(
        preflight_cell_embedding_table_parquet_bytes(
            &bytes,
            &expected,
            table.dimension(),
            bindings,
            EmbeddingColumnarBudgets::new(
                bytes.len() as u64 - 1,
                8 * 1024 * 1024,
                8 * 1024 * 1024,
                8 * 1024 * 1024,
            ),
        ),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));

    let row_group_required = match preflight_cell_embedding_table_parquet_bytes(
        &bytes,
        &expected,
        table.dimension(),
        bindings,
        EmbeddingColumnarBudgets::new(8 * 1024 * 1024, 8 * 1024 * 1024, 0, 8 * 1024 * 1024),
    ) {
        Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected row-group budget failure, observed {result:?}"),
    };
    let decoded_required = match preflight_cell_embedding_table_parquet_bytes(
        &bytes,
        &expected,
        table.dimension(),
        bindings,
        EmbeddingColumnarBudgets::new(8 * 1024 * 1024, 8 * 1024 * 1024, 8 * 1024 * 1024, 0),
    ) {
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected decoded budget failure, observed {result:?}"),
    };
    let writer_decoded_required = match write_cell_embedding_table_parquet(
        &mut Vec::new(),
        &table,
        bindings,
        EmbeddingColumnarBudgets::new(8 * 1024 * 1024, 8 * 1024 * 1024, 8 * 1024 * 1024, 0),
    ) {
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected writer decoded budget failure, observed {result:?}"),
    };
    assert_eq!(writer_decoded_required, decoded_required);
    let retained_required = match preflight_cell_embedding_table_parquet_bytes(
        &bytes,
        &expected,
        table.dimension(),
        bindings,
        EmbeddingColumnarBudgets::new(8 * 1024 * 1024, 0, 8 * 1024 * 1024, 8 * 1024 * 1024),
    ) {
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected retained budget failure, observed {result:?}"),
    };

    let exact = EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        retained_required,
        row_group_required,
        decoded_required,
    );
    preflight_cell_embedding_table_parquet_bytes(
        &bytes,
        &expected,
        table.dimension(),
        bindings,
        exact,
    )
    .expect("exact structural budgets");
    for one_short in [
        EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            retained_required - 1,
            row_group_required,
            decoded_required,
        ),
        EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            retained_required,
            row_group_required - 1,
            decoded_required,
        ),
        EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            retained_required,
            row_group_required,
            decoded_required - 1,
        ),
    ] {
        assert!(preflight_cell_embedding_table_parquet_bytes(
            &bytes,
            &expected,
            table.dimension(),
            bindings,
            one_short,
        )
        .is_err());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn arrow_and_parquet_writers_preserve_the_same_logical_shape(row_count in 0_usize..256) {
        let (expected, table, bindings) = fixture_with_rows(row_count);
        let mut arrow = Vec::new();
        let arrow_summary = write_cell_embedding_table_arrow(
            &mut arrow,
            &table,
            bindings,
            scale_budgets(),
        )
        .expect("write Arrow");
        let mut parquet = Vec::new();
        let parquet_summary = write_cell_embedding_table_parquet(
            &mut parquet,
            &table,
            bindings,
            scale_budgets(),
        )
        .expect("write Parquet");
        let arrow_preflight = preflight_cell_embedding_table_arrow_bytes(
            &arrow,
            &expected,
            bindings,
            scale_budgets(),
        )
        .expect("preflight Arrow");
        let parquet_preflight = preflight_cell_embedding_table_parquet_bytes(
            &parquet,
            &expected,
            table.dimension(),
            bindings,
            scale_budgets(),
        )
        .expect("preflight Parquet");

        prop_assert_eq!(arrow_summary.row_count(), parquet_summary.row_count());
        prop_assert_eq!(arrow_summary.dimension(), parquet_summary.dimension());
        prop_assert_eq!(arrow_preflight.row_count(), parquet_preflight.row_count());
        prop_assert_eq!(arrow_preflight.dimension(), parquet_preflight.dimension());
        prop_assert_eq!(
            table.qc_summary().logical_digest(),
            bindings.table_logical_digest(),
        );
    }
}
