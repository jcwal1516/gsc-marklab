#![cfg(feature = "parquet")]

#[allow(dead_code)]
#[path = "support/multiscale_matrix_columnar.rs"]
mod support;

use std::{io::Write, mem::size_of};

use marklab::{
    preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    validate_patch_embedding_table_arrow_bytes, validate_patch_embedding_table_arrow_from_store,
    validate_patch_embedding_table_parquet_bytes,
    validate_patch_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
    write_patch_embedding_table_parquet, ColumnarWriteSummary, EmbeddingColumnarBudgets,
    EmbeddingStatus, LocalArtifactStore, MultiscaleColumnarError, PatchEmbeddingTable, StoreId,
};
use parquet::{
    file::metadata::{
        ColumnChunkMetaData, ParquetMetaData, ParquetMetaDataReader, RowGroupMetaData,
    },
    format::{
        ColumnChunk, ColumnMetaData, ColumnOrder, Encoding, FileMetaData, KeyValue,
        PageEncodingStats, RowGroup, SchemaElement,
    },
};
use tempfile::TempDir;

use support::{patch_table_with_rows, LARGE};

const BATCH_ROWS: usize = 8_192;
const FIXED_WORKSPACE: usize = 64 * 1_024;
const MAXIMUM_FOOTER_BYTES: usize = 1_024 * 1_024;
const COLUMN_COUNT: usize = 3;
const METADATA_ENTRIES: usize = 9;

fn generous() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, LARGE as u64)
}

fn status_len(status: EmbeddingStatus) -> usize {
    match status {
        EmbeddingStatus::Present => "present".len(),
        EmbeddingStatus::MissingVector => "missing_vector".len(),
        EmbeddingStatus::ExtractionFailed => "extraction_failed".len(),
        EmbeddingStatus::QcRejected => "qc_rejected".len(),
    }
}

fn text_bytes(table: &PatchEmbeddingTable, start: usize, end: usize) -> (usize, usize) {
    (start..end).fold((0, 0), |(ids, statuses), index| {
        let row = table.row(index).expect("resource row");
        (
            ids + row.patch_id().as_str().len(),
            statuses + status_len(row.status()),
        )
    })
}

fn decoded(table: &PatchEmbeddingTable, start: usize, end: usize) -> usize {
    let rows = end - start;
    if rows == 0 {
        return 0;
    }
    let components = rows * table.dimension() as usize;
    let (ids, statuses) = text_bytes(table, start, end);
    3 * rows.div_ceil(8)
        + components.div_ceil(8)
        + 2 * (rows + 1) * size_of::<i32>()
        + ids
        + statuses
        + components * size_of::<f32>()
}

fn decoded_groups(table: &PatchEmbeddingTable) -> Vec<usize> {
    (0..table.row_count())
        .step_by(BATCH_ROWS)
        .map(|start| decoded(table, start, (start + BATCH_ROWS).min(table.row_count())))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WriterRequirements {
    decoded: u64,
    group: usize,
    retained: usize,
}

fn arrow_writer_requirements(table: &PatchEmbeddingTable) -> WriterRequirements {
    const ALIGNMENT: usize = 64;
    const BUFFER_COUNT: usize = 9;
    let groups = decoded_groups(table);
    let maximum_batch = groups
        .iter()
        .map(|bytes| bytes + BUFFER_COUNT * (ALIGNMENT - 1) + FIXED_WORKSPACE)
        .max()
        .unwrap_or(0);
    let blocks = groups.len() * size_of::<arrow::ipc::Block>();
    let block_capacity = 2 * blocks + size_of::<Vec<arrow::ipc::Block>>();
    let batch_peak = 2 * maximum_batch + block_capacity;
    let footer_peak = 2 * (blocks + FIXED_WORKSPACE) + block_capacity + FIXED_WORKSPACE;
    WriterRequirements {
        decoded: groups.iter().map(|bytes| *bytes as u64).sum(),
        group: maximum_batch,
        retained: batch_peak.max(footer_peak),
    }
}

fn parquet_metadata_writer_bytes(row_groups: usize) -> usize {
    const FOOTER_BASE: usize = 64 * 1_024;
    const FOOTER_PER_GROUP: usize = 2 * 1_024;
    let raw_group = size_of::<RowGroup>()
        + COLUMN_COUNT * size_of::<ColumnChunk>()
        + COLUMN_COUNT * size_of::<ColumnMetaData>()
        + COLUMN_COUNT * 2 * size_of::<Encoding>()
        + (COLUMN_COUNT + 2) * size_of::<String>()
        + COLUMN_COUNT * size_of::<PageEncodingStats>()
        + 128;
    let high_group =
        size_of::<RowGroupMetaData>() + COLUMN_COUNT * size_of::<ColumnChunkMetaData>() + 256;
    FIXED_WORKSPACE
        + FOOTER_BASE
        + size_of::<FileMetaData>()
        + size_of::<ParquetMetaData>()
        + row_groups * 2 * (raw_group + high_group + FOOTER_PER_GROUP)
}

fn parquet_writer_requirements(table: &PatchEmbeddingTable) -> WriterRequirements {
    const LOCKED_BYTES_HANDLE: usize = 4 * size_of::<usize>();
    let groups = (0..table.row_count())
        .step_by(BATCH_ROWS)
        .map(|start| {
            let end = (start + BATCH_ROWS).min(table.row_count());
            let rows = end - start;
            let components = rows * table.dimension() as usize;
            let (ids, statuses) = text_bytes(table, start, end);
            (
                decoded(table, start, end),
                components * size_of::<f32>() + rows * 2 * size_of::<u32>() + ids + statuses,
            )
        })
        .collect::<Vec<_>>();
    let maximum_input = groups.iter().map(|(bytes, _)| *bytes).max().unwrap_or(0);
    let maximum_plain = groups.iter().map(|(_, bytes)| *bytes).max().unwrap_or(0);
    let group_rows = table.row_count().min(BATCH_ROWS);
    let group_components = group_rows * table.dimension() as usize;
    let level_bytes = 4 * group_components.div_ceil(8);
    let page_overhead = group_rows * COLUMN_COUNT * (1_024 + 4 * LOCKED_BYTES_HANDLE);
    let level_workspace = group_components * 16;
    let plain_and_levels = maximum_plain + level_bytes;
    let group = maximum_input
        + level_workspace
        + 2 * plain_and_levels
        + page_overhead
        + 4 * plain_and_levels
        + FIXED_WORKSPACE;
    let metadata = parquet_metadata_writer_bytes(groups.len());
    WriterRequirements {
        decoded: groups.iter().map(|(bytes, _)| *bytes as u64).sum(),
        group,
        retained: (group + metadata).max(metadata + FIXED_WORKSPACE),
    }
}

fn arrow_reader_requirements(bytes: &[u8]) -> (usize, usize, u64) {
    let length_offset = bytes.len() - 10;
    let footer_len = usize::try_from(i32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ))
    .expect("positive footer length");
    let footer_start = length_offset - footer_len;
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..length_offset])
        .expect("trusted Arrow footer");
    let blocks = footer.recordBatches().expect("record batches");
    let mut maximum_group = 0;
    let mut maximum_decoded = 0;
    let mut aggregate_decoded = 0_u64;
    for block in blocks.iter() {
        let offset = usize::try_from(block.offset()).expect("block offset");
        let metadata = usize::try_from(block.metaDataLength()).expect("metadata length");
        let body = usize::try_from(block.bodyLength()).expect("body length");
        maximum_group = maximum_group.max(metadata + body);
        let message = arrow::ipc::root_as_message(&bytes[offset + 8..offset + metadata])
            .expect("record message");
        let batch_decoded = message
            .header_as_record_batch()
            .expect("record batch")
            .buffers()
            .expect("buffers")
            .iter()
            .map(|buffer| usize::try_from(buffer.length()).expect("buffer length"))
            .sum::<usize>();
        maximum_decoded = maximum_decoded.max(batch_decoded);
        aggregate_decoded += batch_decoded as u64;
    }
    let retained_preflight = 2 * footer_len + maximum_group + FIXED_WORKSPACE;
    (
        maximum_group,
        retained_preflight + maximum_decoded + FIXED_WORKSPACE,
        aggregate_decoded,
    )
}

fn raw_parquet_preflight_bytes(footer_len: usize, row_groups: usize) -> usize {
    const MAXIMUM_PAGE_HEADER_BYTES: usize = 64 * 1_024;
    let maximum_inline_element = [
        size_of::<SchemaElement>(),
        size_of::<RowGroup>(),
        size_of::<ColumnChunk>(),
        size_of::<ColumnMetaData>(),
        size_of::<KeyValue>(),
        size_of::<PageEncodingStats>(),
        size_of::<ColumnOrder>(),
        size_of::<Encoding>(),
        size_of::<String>(),
    ]
    .into_iter()
    .max()
    .expect("nonempty sizes");
    let total_elements =
        row_groups * (COLUMN_COUNT * 5 + 3) + (COLUMN_COUNT + 3) + COLUMN_COUNT + METADATA_ENTRIES;
    let protocol_state = 16 * 64;
    let metadata_tree =
        2 * total_elements * maximum_inline_element + footer_len + size_of::<FileMetaData>();
    let page_workspace =
        2 * MAXIMUM_PAGE_HEADER_BYTES + 64 * maximum_inline_element + protocol_state;
    metadata_tree + protocol_state + footer_len + page_workspace
}

fn parquet_reader_requirements(
    bytes: &[u8],
    expected_decoded_groups: &[usize],
) -> (usize, usize, u64) {
    let mut file = tempfile::tempfile().expect("temporary Parquet file");
    file.write_all(bytes).expect("trusted Parquet fixture");
    let metadata = ParquetMetaDataReader::new()
        .parse_and_finish(&file)
        .expect("trusted Parquet metadata");
    assert_eq!(metadata.num_row_groups(), expected_decoded_groups.len());
    let footer_len = u32::from_le_bytes(
        bytes[bytes.len() - 8..bytes.len() - 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let raw_retained = raw_parquet_preflight_bytes(footer_len, metadata.num_row_groups());
    let stock_retained =
        2 * raw_parquet_preflight_bytes(MAXIMUM_FOOTER_BYTES, metadata.num_row_groups());
    let maximum_chunk = metadata
        .row_groups()
        .iter()
        .flat_map(|group| group.columns())
        .map(|column| usize::try_from(column.compressed_size()).expect("chunk bytes"))
        .max()
        .unwrap_or(0);
    let retained_preflight = raw_retained
        .max(stock_retained)
        .max(raw_retained + maximum_chunk);
    let maximum_group = metadata
        .row_groups()
        .iter()
        .zip(expected_decoded_groups)
        .map(|(group, decoded)| {
            let encoded = group
                .columns()
                .iter()
                .map(|column| usize::try_from(column.compressed_size()).expect("chunk bytes"))
                .sum::<usize>();
            let rows = usize::try_from(group.num_rows()).expect("group rows");
            2 * encoded + 2 * decoded + 4 * rows
        })
        .max()
        .unwrap_or(0);
    let retained =
        retained_preflight.max(2 * metadata.memory_size() + maximum_group + FIXED_WORKSPACE);
    (
        maximum_group,
        retained,
        expected_decoded_groups
            .iter()
            .map(|bytes| *bytes as u64)
            .sum(),
    )
}

fn assert_writer_budget_edges(
    encoded_len: u64,
    requirements: WriterRequirements,
    mut write: impl FnMut(
        EmbeddingColumnarBudgets,
    ) -> Result<ColumnarWriteSummary, MultiscaleColumnarError>,
) {
    write(EmbeddingColumnarBudgets::new(
        encoded_len,
        requirements.retained,
        requirements.group,
        requirements.decoded,
    ))
    .expect("exact writer budgets");
    for (limits, expected) in [
        (
            EmbeddingColumnarBudgets::new(
                encoded_len - 1,
                requirements.retained,
                requirements.group,
                requirements.decoded,
            ),
            "file",
        ),
        (
            EmbeddingColumnarBudgets::new(
                encoded_len,
                requirements.retained,
                requirements.group,
                requirements.decoded - 1,
            ),
            "decoded",
        ),
        (
            EmbeddingColumnarBudgets::new(
                encoded_len,
                requirements.retained,
                requirements.group - 1,
                requirements.decoded,
            ),
            "group",
        ),
        (
            EmbeddingColumnarBudgets::new(
                encoded_len,
                requirements.retained - 1,
                requirements.group,
                requirements.decoded,
            ),
            "retained",
        ),
    ] {
        let error = write(limits).expect_err("one-short writer budget");
        assert_budget_error(&error, expected);
    }
}

fn assert_reader_budget_edges<T>(
    encoded_len: u64,
    group: usize,
    retained: usize,
    decoded: u64,
    mut validate: impl FnMut(EmbeddingColumnarBudgets) -> Result<T, MultiscaleColumnarError>,
) {
    validate(EmbeddingColumnarBudgets::new(
        encoded_len,
        retained,
        group,
        decoded,
    ))
    .unwrap_or_else(|error| panic!("exact reader budgets failed: {error:?}"));
    for (limits, expected) in [
        (
            EmbeddingColumnarBudgets::new(encoded_len - 1, retained, group, decoded),
            "file",
        ),
        (
            EmbeddingColumnarBudgets::new(encoded_len, retained, group, decoded - 1),
            "decoded",
        ),
        (
            EmbeddingColumnarBudgets::new(encoded_len, retained, group - 1, decoded),
            "group",
        ),
        (
            EmbeddingColumnarBudgets::new(encoded_len, retained - 1, group, decoded),
            "retained",
        ),
    ] {
        let error = match validate(limits) {
            Err(error) => error,
            Ok(_) => panic!("one-short {expected} budget unexpectedly passed"),
        };
        assert_budget_error(&error, expected);
    }
}

fn assert_budget_error(error: &MultiscaleColumnarError, expected: &str) {
    assert!(
        matches!(
            (error, expected),
            (
                MultiscaleColumnarError::FileByteBudgetExceeded { .. },
                "file"
            ) | (
                MultiscaleColumnarError::DecodedByteBudgetExceeded { .. },
                "decoded"
            ) | (
                MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. },
                "group"
            ) | (
                MultiscaleColumnarError::RetainedByteBudgetExceeded { .. },
                "retained"
            )
        ),
        "unexpected {expected} error: {error:?}"
    );
}

#[test]
fn matrix_writers_enforce_independent_exact_and_one_short_budgets() {
    let table = patch_table_with_rows(BATCH_ROWS + 1, 3);
    let mut arrow = Vec::new();
    write_patch_embedding_table_arrow(&mut arrow, &table, generous())
        .expect("generous Arrow writer");
    assert_writer_budget_edges(
        arrow.len() as u64,
        arrow_writer_requirements(&table),
        |limits| write_patch_embedding_table_arrow(&mut Vec::new(), &table, limits),
    );

    let mut parquet = Vec::new();
    write_patch_embedding_table_parquet(&mut parquet, &table, generous())
        .expect("generous Parquet writer");
    assert_writer_budget_edges(
        parquet.len() as u64,
        parquet_writer_requirements(&table),
        |limits| write_patch_embedding_table_parquet(&mut Vec::new(), &table, limits),
    );
}

#[test]
fn matrix_full_readers_enforce_independent_exact_and_one_short_budgets() {
    let table = patch_table_with_rows(3, 3);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("matrix-budget").expect("store ID"),
    )
    .expect("store");
    let arrow_record = publish_patch_embedding_table_arrow(&store, &table, generous())
        .expect("publish Arrow")
        .into_record();
    let parquet_record = publish_patch_embedding_table_parquet(&store, &table, generous())
        .expect("publish Parquet")
        .into_record();
    let mut arrow = Vec::new();
    write_patch_embedding_table_arrow(&mut arrow, &table, generous()).expect("Arrow bytes");
    let mut parquet = Vec::new();
    write_patch_embedding_table_parquet(&mut parquet, &table, generous()).expect("Parquet bytes");

    let expected_groups = decoded_groups(&table);
    let (group, retained, decoded_bytes) = arrow_reader_requirements(&arrow);
    assert_eq!(
        decoded_bytes,
        expected_groups.iter().map(|v| *v as u64).sum::<u64>()
    );
    assert_reader_budget_edges(
        arrow.len() as u64,
        group,
        retained,
        decoded_bytes,
        |limits| validate_patch_embedding_table_arrow_bytes(&arrow, &arrow_record, &table, limits),
    );

    let (group, retained, decoded_bytes) = parquet_reader_requirements(&parquet, &expected_groups);
    assert_reader_budget_edges(
        parquet.len() as u64,
        group,
        retained,
        decoded_bytes,
        |limits| {
            validate_patch_embedding_table_parquet_bytes(&parquet, &parquet_record, &table, limits)
        },
    );
}

#[test]
fn matrix_profiles_full_decode_batch_boundaries_and_three_groups() {
    for (rows, expected_groups) in [
        (BATCH_ROWS, 1),
        (BATCH_ROWS + 1, 2),
        (BATCH_ROWS * 2 + 2, 3),
    ] {
        let table = patch_table_with_rows(rows, 3);
        let root = TempDir::new().expect("store root");
        let store = LocalArtifactStore::open(
            root.path(),
            StoreId::new(format!("matrix-groups-{expected_groups}")).expect("store ID"),
        )
        .expect("store");
        let arrow = publish_patch_embedding_table_arrow(&store, &table, generous())
            .expect("publish Arrow")
            .into_record();
        let parquet = publish_patch_embedding_table_parquet(&store, &table, generous())
            .expect("publish Parquet")
            .into_record();
        assert_eq!(
            validate_patch_embedding_table_arrow_from_store(&store, &arrow, &table, generous())
                .expect("full Arrow decode")
                .record_batch_count(),
            expected_groups
        );
        assert_eq!(
            validate_patch_embedding_table_parquet_from_store(
                &store,
                &parquet,
                &table,
                generous(),
            )
            .expect("full Parquet decode")
            .row_group_count(),
            expected_groups
        );

        let mut arrow_bytes = Vec::new();
        write_patch_embedding_table_arrow(&mut arrow_bytes, &table, generous())
            .expect("boundary Arrow");
        let mut parquet_bytes = Vec::new();
        write_patch_embedding_table_parquet(&mut parquet_bytes, &table, generous())
            .expect("boundary Parquet");
        assert_eq!(
            preflight_patch_embedding_table_arrow_bytes(&arrow_bytes, &table, generous())
                .expect("Arrow preflight")
                .record_batch_count(),
            expected_groups
        );
        assert_eq!(
            preflight_patch_embedding_table_parquet_bytes(&parquet_bytes, &table, generous())
                .expect("Parquet preflight")
                .row_group_count(),
            expected_groups
        );
    }
}
