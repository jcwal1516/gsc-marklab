#![cfg(feature = "parquet")]

#[path = "support/patch_region_columnar.rs"]
mod support;

use std::{io::Write, mem::size_of};

use marklab::{
    preflight_patch_region_link_arrow_bytes, preflight_patch_region_link_parquet_bytes,
    publish_patch_region_link_arrow, publish_patch_region_link_parquet,
    validate_patch_region_link_arrow_bytes, validate_patch_region_link_arrow_from_store,
    validate_patch_region_link_parquet_bytes, validate_patch_region_link_parquet_from_store,
    write_patch_region_link_arrow, write_patch_region_link_parquet, EmbeddingColumnarBudgets,
    LocalArtifactStore, MultiscaleColumnarError, PatchRegionDeclaration, PatchRegionLink,
    SpatialColumnarWriteSummary, StoreId,
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

const LARGE: usize = 512 * 1024 * 1024;
const BATCH_ROWS: usize = 8_192;

fn generous() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, LARGE as u64)
}

fn text_bytes(rows: &[PatchRegionDeclaration]) -> usize {
    rows.iter()
        .map(|row| {
            row.patch_id().as_str().len()
                + row.region_id().as_str().len()
                + match row.relation() {
                    marklab::PatchRegionRelation::FullyContained => "fully_contained".len(),
                    marklab::PatchRegionRelation::PartialOverlap => "partial_overlap".len(),
                }
        })
        .sum()
}

fn decoded(rows: &[PatchRegionDeclaration]) -> usize {
    5 * rows.len().div_ceil(8)
        + 3 * (rows.len() + 1) * size_of::<i32>()
        + text_bytes(rows)
        + 2 * rows.len() * size_of::<u64>()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WriterRequirements {
    decoded: u64,
    group: usize,
    retained: usize,
}

fn arrow_writer_requirements(link: &PatchRegionLink) -> WriterRequirements {
    const ALIGNMENT: usize = 64;
    const MESSAGE_WORKSPACE: usize = 64 * 1024;
    let chunks = link
        .nonzero_relations()
        .chunks(BATCH_ROWS)
        .map(decoded)
        .collect::<Vec<_>>();
    let maximum_batch = chunks
        .iter()
        .map(|bytes| bytes + 13 * (ALIGNMENT - 1) + MESSAGE_WORKSPACE)
        .max()
        .unwrap_or(0);
    let record_blocks =
        link.nonzero_relation_count().div_ceil(BATCH_ROWS) * size_of::<arrow::ipc::Block>();
    let block_capacity = record_blocks * 2 + size_of::<Vec<arrow::ipc::Block>>();
    let batch_peak = maximum_batch * 2 + block_capacity;
    let footer_peak = (record_blocks + MESSAGE_WORKSPACE) * 2 + block_capacity + MESSAGE_WORKSPACE;
    WriterRequirements {
        decoded: chunks.iter().map(|bytes| *bytes as u64).sum(),
        group: maximum_batch,
        retained: batch_peak.max(footer_peak),
    }
}

fn parquet_metadata_writer_bytes(row_groups: usize, columns: usize) -> usize {
    const APPLICATION_METADATA: usize = 64 * 1024;
    const FOOTER_BASE: usize = 64 * 1024;
    const FOOTER_PER_GROUP: usize = 2 * 1024;
    let raw_group = size_of::<RowGroup>()
        + columns * size_of::<ColumnChunk>()
        + columns * size_of::<ColumnMetaData>()
        + columns * 2 * size_of::<Encoding>()
        + (columns + 2) * size_of::<String>()
        + columns * size_of::<PageEncodingStats>()
        + 128;
    let high_group =
        size_of::<RowGroupMetaData>() + columns * size_of::<ColumnChunkMetaData>() + 256;
    let per_group = (raw_group + high_group + FOOTER_PER_GROUP) * 2;
    APPLICATION_METADATA
        + FOOTER_BASE
        + size_of::<FileMetaData>()
        + size_of::<ParquetMetaData>()
        + row_groups * per_group
}

fn parquet_writer_requirements(link: &PatchRegionLink) -> WriterRequirements {
    const FIXED_WORKSPACE: usize = 64 * 1024;
    const LOCKED_BYTES_HANDLE: usize = 4 * size_of::<usize>();
    let chunks = link
        .nonzero_relations()
        .chunks(BATCH_ROWS)
        .map(|rows| (decoded(rows), rows.len() * 28 + text_bytes(rows)))
        .collect::<Vec<_>>();
    let decoded_total = chunks.iter().map(|(bytes, _)| *bytes as u64).sum();
    let maximum_input = chunks.iter().map(|(bytes, _)| *bytes).max().unwrap_or(0);
    let maximum_plain = chunks.iter().map(|(_, bytes)| *bytes).max().unwrap_or(0);
    let group_rows = link.nonzero_relation_count().min(BATCH_ROWS);
    let page_overhead = group_rows * 5 * (1024 + 4 * LOCKED_BYTES_HANDLE);
    let encoder_workspace = group_rows * 16;
    let group = maximum_input
        + encoder_workspace
        + maximum_plain * 2
        + page_overhead
        + maximum_plain * 4
        + FIXED_WORKSPACE;
    let metadata =
        parquet_metadata_writer_bytes(link.nonzero_relation_count().div_ceil(BATCH_ROWS), 5);
    WriterRequirements {
        decoded: decoded_total,
        group,
        retained: (group + metadata).max(metadata + FIXED_WORKSPACE),
    }
}

fn arrow_reader_requirements(bytes: &[u8]) -> (usize, usize, u64) {
    const READER_WORKSPACE: usize = 64 * 1024;
    let length_offset = bytes.len() - 10;
    let footer_length = usize::try_from(i32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ))
    .expect("positive footer length");
    let footer_start = length_offset - footer_length;
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..length_offset])
        .expect("trusted Arrow footer");
    let blocks = footer.recordBatches().expect("record batches");
    let mut maximum_group = 0_usize;
    let mut maximum_decoded = 0_usize;
    let mut aggregate_decoded = 0_u64;
    for block in blocks.iter() {
        let offset = usize::try_from(block.offset()).expect("block offset");
        let metadata = usize::try_from(block.metaDataLength()).expect("metadata length");
        let body = usize::try_from(block.bodyLength()).expect("body length");
        maximum_group = maximum_group.max(metadata + body);
        let message = arrow::ipc::root_as_message(&bytes[offset + 8..offset + metadata])
            .expect("record message");
        let decoded = message
            .header_as_record_batch()
            .expect("record batch")
            .buffers()
            .expect("buffers")
            .iter()
            .map(|buffer| usize::try_from(buffer.length()).expect("buffer length"))
            .sum::<usize>();
        maximum_decoded = maximum_decoded.max(decoded);
        aggregate_decoded += decoded as u64;
    }
    let retained_preflight = footer_length * 2 + maximum_group + READER_WORKSPACE;
    (
        maximum_group,
        retained_preflight + maximum_decoded + READER_WORKSPACE,
        aggregate_decoded,
    )
}

fn parquet_reader_requirements(bytes: &[u8], decoded_groups: &[usize]) -> (usize, usize, u64) {
    const MAXIMUM_FOOTER_BYTES: usize = 1024 * 1024;
    const MAXIMUM_PAGE_HEADER_BYTES: usize = 64 * 1024;
    const READER_WORKSPACE: usize = 64 * 1024;
    const COLUMNS: usize = 5;
    const METADATA_ENTRIES: usize = 13;

    let mut file = tempfile::tempfile().expect("temporary Parquet file");
    file.write_all(bytes)
        .expect("write trusted Parquet fixture");
    let metadata = ParquetMetaDataReader::new()
        .parse_and_finish(&file)
        .expect("trusted Parquet metadata");
    assert_eq!(metadata.num_row_groups(), decoded_groups.len());
    let footer_length = u32::from_le_bytes(
        bytes[bytes.len() - 8..bytes.len() - 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
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
    .expect("nonempty size list");
    let total_elements =
        metadata.num_row_groups() * (1 + 5 * COLUMNS) + 2 * COLUMNS + 1 + METADATA_ENTRIES;
    let estimate_raw = |footer: usize| {
        let protocol_state = 16 * 64;
        let metadata_tree =
            total_elements * maximum_inline_element * 2 + footer + size_of::<FileMetaData>();
        let page_workspace =
            MAXIMUM_PAGE_HEADER_BYTES * 2 + 64 * maximum_inline_element + protocol_state;
        metadata_tree + protocol_state + footer + page_workspace
    };
    let raw_retained = estimate_raw(footer_length);
    let stock_retained = estimate_raw(MAXIMUM_FOOTER_BYTES) * 2;
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
        .zip(decoded_groups)
        .map(|(group, decoded)| {
            let encoded = group
                .columns()
                .iter()
                .map(|column| usize::try_from(column.compressed_size()).expect("chunk bytes"))
                .sum::<usize>();
            let rows = usize::try_from(group.num_rows()).expect("group rows");
            encoded * 2 + decoded * 2 + rows * 4
        })
        .max()
        .unwrap_or(0);
    let retained =
        retained_preflight.max(metadata.memory_size() + maximum_group + READER_WORKSPACE);
    (
        maximum_group,
        retained,
        decoded_groups.iter().map(|bytes| *bytes as u64).sum(),
    )
}

fn assert_writer_budget_edges(
    encoded_len: u64,
    requirements: WriterRequirements,
    mut write: impl FnMut(
        EmbeddingColumnarBudgets,
    ) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError>,
) {
    let exact = EmbeddingColumnarBudgets::new(
        encoded_len,
        requirements.retained,
        requirements.group,
        requirements.decoded,
    );
    write(exact).expect("exact writer budgets");
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
        assert!(
            matches!(
                (&error, expected),
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
        assert!(
            matches!(
                (&error, expected),
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
}

#[test]
fn patch_region_writers_enforce_independent_exact_and_one_short_budgets() {
    let link = support::patch_region_scale_link(BATCH_ROWS + 1);
    let mut arrow = Vec::new();
    write_patch_region_link_arrow(&mut arrow, &link, generous()).expect("generous Arrow writer");
    assert_writer_budget_edges(
        arrow.len() as u64,
        arrow_writer_requirements(&link),
        |limits| write_patch_region_link_arrow(&mut Vec::new(), &link, limits),
    );
    let mut parquet = Vec::new();
    write_patch_region_link_parquet(&mut parquet, &link, generous())
        .expect("generous Parquet writer");
    assert_writer_budget_edges(
        parquet.len() as u64,
        parquet_writer_requirements(&link),
        |limits| write_patch_region_link_parquet(&mut Vec::new(), &link, limits),
    );
}

#[test]
fn patch_region_full_readers_enforce_independent_exact_and_one_short_budgets() {
    let link = support::patch_region_scale_link(3);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("patch-region-budget").expect("store ID"),
    )
    .expect("store");
    let arrow_record = publish_patch_region_link_arrow(&store, &link, generous())
        .expect("publish Arrow")
        .into_record();
    let parquet_record = publish_patch_region_link_parquet(&store, &link, generous())
        .expect("publish Parquet")
        .into_record();
    let mut arrow = Vec::new();
    write_patch_region_link_arrow(&mut arrow, &link, generous()).expect("Arrow bytes");
    let mut parquet = Vec::new();
    write_patch_region_link_parquet(&mut parquet, &link, generous()).expect("Parquet bytes");
    let (group, retained, decoded_bytes) = arrow_reader_requirements(&arrow);
    assert_eq!(decoded_bytes, decoded(link.nonzero_relations()) as u64);
    assert_reader_budget_edges(
        arrow.len() as u64,
        group,
        retained,
        decoded_bytes,
        |limits| validate_patch_region_link_arrow_bytes(&arrow, &arrow_record, &link, limits),
    );
    let decoded_group = decoded(link.nonzero_relations());
    let (group, retained, decoded_bytes) = parquet_reader_requirements(&parquet, &[decoded_group]);
    assert_reader_budget_edges(
        parquet.len() as u64,
        group,
        retained,
        decoded_bytes,
        |limits| validate_patch_region_link_parquet_bytes(&parquet, &parquet_record, &link, limits),
    );
}

#[test]
fn patch_region_profiles_cover_batch_boundaries_and_three_group_full_decode() {
    for (rows, expected_groups) in [(BATCH_ROWS, 1), (BATCH_ROWS + 1, 2)] {
        let link = support::patch_region_scale_link(rows);
        let mut arrow = Vec::new();
        write_patch_region_link_arrow(&mut arrow, &link, generous()).expect("boundary Arrow");
        let mut parquet = Vec::new();
        write_patch_region_link_parquet(&mut parquet, &link, generous()).expect("boundary Parquet");
        assert_eq!(
            preflight_patch_region_link_arrow_bytes(&arrow, &link, generous())
                .expect("Arrow preflight")
                .record_batch_count(),
            expected_groups
        );
        assert_eq!(
            preflight_patch_region_link_parquet_bytes(&parquet, &link, generous())
                .expect("Parquet preflight")
                .row_group_count(),
            expected_groups
        );
    }

    let link = support::patch_region_scale_link(BATCH_ROWS * 2 + 2);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("patch-region-three-groups").expect("store ID"),
    )
    .expect("store");
    let arrow = publish_patch_region_link_arrow(&store, &link, generous())
        .expect("publish Arrow")
        .into_record();
    let parquet = publish_patch_region_link_parquet(&store, &link, generous())
        .expect("publish Parquet")
        .into_record();
    assert_eq!(
        validate_patch_region_link_arrow_from_store(&store, &arrow, &link, generous())
            .expect("three-batch Arrow")
            .record_batch_count(),
        3
    );
    assert_eq!(
        validate_patch_region_link_parquet_from_store(&store, &parquet, &link, generous())
            .expect("three-group Parquet")
            .row_group_count(),
        3
    );
}
