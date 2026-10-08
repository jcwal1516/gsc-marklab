#![cfg(feature = "parquet")]

#[path = "support/cell_patch_columnar.rs"]
mod support;

use std::{io::Write, mem::size_of};

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
    write_cell_patch_edge_table_parquet, CellPatchAssignment, CellPatchAssignmentStatus,
    CellPatchEdge, CellPatchLink, EmbeddingColumnarBudgets, LocalArtifactStore,
    MultiscaleColumnarError, SpatialColumnarWriteSummary, StoreId,
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
use support::{cell_patch_fixture, interpolation_scale_fixture, CellPatchFixtureMode};
use tempfile::TempDir;

const LARGE: usize = 512 * 1024 * 1024;
const BATCH_ROWS: usize = 8_192;

fn generous() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, LARGE as u64)
}

fn assignment_decoded(rows: &[CellPatchAssignment]) -> usize {
    let validity = rows.len().div_ceil(8) * 6;
    let offsets = (rows.len() + 1) * size_of::<i32>() * 2;
    let text = rows
        .iter()
        .map(|row| {
            row.cell_id().as_str().len()
                + match row.status() {
                    CellPatchAssignmentStatus::Assigned => "assigned".len(),
                    CellPatchAssignmentStatus::OutsideSampledSupport => {
                        "outside_sampled_support".len()
                    }
                    CellPatchAssignmentStatus::InterpolationUnavailable => {
                        "interpolation_unavailable".len()
                    }
                }
        })
        .sum::<usize>();
    validity + offsets + text + rows.len() * size_of::<u64>() * 4
}

fn assignment_text_bytes(rows: &[CellPatchAssignment]) -> usize {
    rows.iter()
        .map(|row| {
            row.cell_id().as_str().len()
                + match row.status() {
                    CellPatchAssignmentStatus::Assigned => "assigned".len(),
                    CellPatchAssignmentStatus::OutsideSampledSupport => {
                        "outside_sampled_support".len()
                    }
                    CellPatchAssignmentStatus::InterpolationUnavailable => {
                        "interpolation_unavailable".len()
                    }
                }
        })
        .sum()
}

fn edge_decoded(rows: &[CellPatchEdge]) -> usize {
    let validity = rows.len().div_ceil(8) * 4;
    let offsets = (rows.len() + 1) * size_of::<i32>();
    let text = rows
        .iter()
        .map(|row| row.patch_id().as_str().len())
        .sum::<usize>();
    validity + offsets + text + rows.len() * size_of::<u64>() * 3
}

fn edge_text_bytes(rows: &[CellPatchEdge]) -> usize {
    rows.iter().map(|row| row.patch_id().as_str().len()).sum()
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
        aggregate_decoded += u64::try_from(decoded).expect("decoded length");
    }
    let retained_preflight = footer_length * 2 + maximum_group + READER_WORKSPACE;
    (
        maximum_group,
        retained_preflight + maximum_decoded + READER_WORKSPACE,
        aggregate_decoded,
    )
}

fn parquet_reader_requirements(
    bytes: &[u8],
    columns: usize,
    decoded_groups: &[usize],
) -> (usize, usize, u64) {
    const MAXIMUM_FOOTER_BYTES: usize = 1024 * 1024;
    const MAXIMUM_PAGE_HEADER_BYTES: usize = 64 * 1024;
    const READER_WORKSPACE: usize = 64 * 1024;
    const METADATA_ENTRIES: usize = 11;

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
        metadata.num_row_groups() * (1 + 5 * columns) + 2 * columns + 1 + METADATA_ENTRIES;
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
        decoded_groups
            .iter()
            .map(|bytes| u64::try_from(*bytes).expect("decoded bytes"))
            .sum(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WriterRequirements {
    decoded: u64,
    group: usize,
    retained: usize,
}

fn arrow_writer_requirements(
    row_count: usize,
    decoded_groups: &[usize],
    buffer_count: usize,
) -> WriterRequirements {
    const ALIGNMENT: usize = 64;
    const MESSAGE_WORKSPACE: usize = 64 * 1024;
    let maximum_batch = decoded_groups
        .iter()
        .map(|decoded| decoded + buffer_count * (ALIGNMENT - 1) + MESSAGE_WORKSPACE)
        .max()
        .unwrap_or(0);
    let record_blocks = row_count.div_ceil(BATCH_ROWS) * size_of::<arrow::ipc::Block>();
    let block_capacity = record_blocks * 2 + size_of::<Vec<arrow::ipc::Block>>();
    let batch_peak = maximum_batch * 2 + block_capacity;
    let footer_peak = (record_blocks + MESSAGE_WORKSPACE) * 2 + block_capacity + MESSAGE_WORKSPACE;
    WriterRequirements {
        decoded: decoded_groups
            .iter()
            .map(|bytes| u64::try_from(*bytes).expect("decoded bytes"))
            .sum(),
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
    let base = APPLICATION_METADATA
        + FOOTER_BASE
        + size_of::<FileMetaData>()
        + size_of::<ParquetMetaData>();
    base + row_groups * per_group
}

fn parquet_writer_requirements(
    row_count: usize,
    columns: usize,
    chunks: &[(usize, usize, usize)],
) -> WriterRequirements {
    const FIXED_WORKSPACE: usize = 64 * 1024;
    const LOCKED_BYTES_HANDLE: usize = 4 * size_of::<usize>();
    let decoded = chunks
        .iter()
        .map(|(decoded, _, _)| u64::try_from(*decoded).expect("decoded bytes"))
        .sum();
    let maximum_input = chunks
        .iter()
        .map(|(decoded, _, _)| *decoded)
        .max()
        .unwrap_or(0);
    let maximum_values = chunks
        .iter()
        .map(|(_, plain, levels)| plain + levels)
        .max()
        .unwrap_or(0);
    let group_rows = row_count.min(BATCH_ROWS);
    let page_overhead = group_rows * columns * (1024 + 4 * LOCKED_BYTES_HANDLE);
    let encoder_workspace = group_rows * 16;
    let retained_chunks = maximum_values * 2 + page_overhead;
    let page_transient = maximum_values * 4;
    let group =
        maximum_input + encoder_workspace + retained_chunks + page_transient + FIXED_WORKSPACE;
    let metadata = parquet_metadata_writer_bytes(row_count.div_ceil(BATCH_ROWS), columns);
    WriterRequirements {
        decoded,
        group,
        retained: (group + metadata).max(metadata + FIXED_WORKSPACE),
    }
}

fn assignment_arrow_writer_requirements(link: &CellPatchLink) -> WriterRequirements {
    let chunks = link
        .assignments()
        .chunks(BATCH_ROWS)
        .map(assignment_decoded)
        .collect::<Vec<_>>();
    arrow_writer_requirements(link.assignment_count(), &chunks, 14)
}

fn edge_arrow_writer_requirements(link: &CellPatchLink) -> WriterRequirements {
    let chunks = link
        .edges()
        .chunks(BATCH_ROWS)
        .map(edge_decoded)
        .collect::<Vec<_>>();
    arrow_writer_requirements(link.edge_count(), &chunks, 9)
}

fn assignment_parquet_writer_requirements(link: &CellPatchLink) -> WriterRequirements {
    let chunks = link
        .assignments()
        .chunks(BATCH_ROWS)
        .map(|rows| {
            (
                assignment_decoded(rows),
                rows.len() * 40 + assignment_text_bytes(rows),
                0,
            )
        })
        .collect::<Vec<_>>();
    parquet_writer_requirements(link.assignment_count(), 6, &chunks)
}

fn edge_parquet_writer_requirements(link: &CellPatchLink) -> WriterRequirements {
    let fixed_values = match link.mode() {
        marklab::CellPatchAssignmentMode::ContainedShared => 12,
        marklab::CellPatchAssignmentMode::DeclaredWeightedInterpolation => 28,
    };
    let chunks = link
        .edges()
        .chunks(BATCH_ROWS)
        .map(|rows| {
            (
                edge_decoded(rows),
                rows.len() * fixed_values + edge_text_bytes(rows),
                rows.len().div_ceil(8) * 4,
            )
        })
        .collect::<Vec<_>>();
    parquet_writer_requirements(link.edge_count(), 4, &chunks)
}

fn assert_writer_budget_edges(
    encoded_len: u64,
    requirements: WriterRequirements,
    mut write: impl FnMut(
        EmbeddingColumnarBudgets,
    ) -> Result<SpatialColumnarWriteSummary, MultiscaleColumnarError>,
) {
    let decoded_error = write(EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, 0))
        .expect_err("zero decoded budget");
    assert_eq!(
        decoded_error,
        MultiscaleColumnarError::DecodedByteBudgetExceeded {
            required: requirements.decoded,
            maximum: 0,
        }
    );
    assert_eq!(
        write(EmbeddingColumnarBudgets::new(
            LARGE as u64,
            LARGE,
            0,
            requirements.decoded,
        )),
        Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded {
            required: requirements.group,
            maximum: 0,
        })
    );
    assert_eq!(
        write(EmbeddingColumnarBudgets::new(
            LARGE as u64,
            0,
            requirements.group,
            requirements.decoded,
        )),
        Err(MultiscaleColumnarError::RetainedByteBudgetExceeded {
            required: requirements.retained,
            maximum: 0,
        })
    );
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
fn cell_patch_writers_enforce_exact_and_one_short_resource_budgets() {
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        let assignment_arrow = assignment_arrow_writer_requirements(&fixture.link);
        let edge_arrow = edge_arrow_writer_requirements(&fixture.link);
        let assignment_parquet = assignment_parquet_writer_requirements(&fixture.link);
        let edge_parquet = edge_parquet_writer_requirements(&fixture.link);
        macro_rules! assert_writer {
            ($writer:path, $requirements:expr) => {{
                let mut bytes = Vec::new();
                $writer(&mut bytes, &fixture.link, generous()).expect("generous writer");
                assert_writer_budget_edges(bytes.len() as u64, $requirements, |limits| {
                    $writer(&mut Vec::new(), &fixture.link, limits)
                });
            }};
        }
        assert_writer!(write_cell_patch_assignment_table_arrow, assignment_arrow);
        assert_writer!(write_cell_patch_edge_table_arrow, edge_arrow);
        assert_writer!(
            write_cell_patch_assignment_table_parquet,
            assignment_parquet
        );
        assert_writer!(write_cell_patch_edge_table_parquet, edge_parquet);
    }
}

#[test]
fn cell_patch_full_readers_enforce_independent_exact_and_one_short_budgets() {
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        let root = TempDir::new().expect("store root");
        let store_name = match mode {
            CellPatchFixtureMode::Contained => "cell-patch-budget-contained",
            CellPatchFixtureMode::Interpolation => "cell-patch-budget-interpolation",
        };
        let store =
            LocalArtifactStore::open(root.path(), StoreId::new(store_name).expect("store ID"))
                .expect("store");
        let assignment_arrow =
            publish_cell_patch_assignment_table_arrow(&store, &fixture.link, generous())
                .expect("publish assignment Arrow")
                .into_record();
        let edge_arrow = publish_cell_patch_edge_table_arrow(&store, &fixture.link, generous())
            .expect("publish edge Arrow")
            .into_record();
        let assignment_parquet =
            publish_cell_patch_assignment_table_parquet(&store, &fixture.link, generous())
                .expect("publish assignment Parquet")
                .into_record();
        let edge_parquet = publish_cell_patch_edge_table_parquet(&store, &fixture.link, generous())
            .expect("publish edge Parquet")
            .into_record();

        let mut assignment_arrow_bytes = Vec::new();
        write_cell_patch_assignment_table_arrow(
            &mut assignment_arrow_bytes,
            &fixture.link,
            generous(),
        )
        .expect("assignment Arrow bytes");
        let mut edge_arrow_bytes = Vec::new();
        write_cell_patch_edge_table_arrow(&mut edge_arrow_bytes, &fixture.link, generous())
            .expect("edge Arrow bytes");
        let mut assignment_parquet_bytes = Vec::new();
        write_cell_patch_assignment_table_parquet(
            &mut assignment_parquet_bytes,
            &fixture.link,
            generous(),
        )
        .expect("assignment Parquet bytes");
        let mut edge_parquet_bytes = Vec::new();
        write_cell_patch_edge_table_parquet(&mut edge_parquet_bytes, &fixture.link, generous())
            .expect("edge Parquet bytes");

        let assignment_decoded = assignment_decoded(fixture.link.assignments());
        let edge_decoded = edge_decoded(fixture.link.edges());
        let (group, retained, decoded) = arrow_reader_requirements(&assignment_arrow_bytes);
        assert_eq!(decoded, assignment_decoded as u64);
        assert_reader_budget_edges(
            assignment_arrow_bytes.len() as u64,
            group,
            retained,
            decoded,
            |limits| {
                validate_cell_patch_assignment_table_arrow_bytes(
                    &assignment_arrow_bytes,
                    &assignment_arrow,
                    &fixture.link,
                    limits,
                )
            },
        );
        let (group, retained, decoded) = arrow_reader_requirements(&edge_arrow_bytes);
        assert_eq!(decoded, edge_decoded as u64);
        assert_reader_budget_edges(
            edge_arrow_bytes.len() as u64,
            group,
            retained,
            decoded,
            |limits| {
                validate_cell_patch_edge_table_arrow_bytes(
                    &edge_arrow_bytes,
                    &edge_arrow,
                    &fixture.link,
                    limits,
                )
            },
        );
        let (group, retained, decoded) =
            parquet_reader_requirements(&assignment_parquet_bytes, 6, &[assignment_decoded]);
        assert_reader_budget_edges(
            assignment_parquet_bytes.len() as u64,
            group,
            retained,
            decoded,
            |limits| {
                validate_cell_patch_assignment_table_parquet_bytes(
                    &assignment_parquet_bytes,
                    &assignment_parquet,
                    &fixture.link,
                    limits,
                )
            },
        );
        let (group, retained, decoded) =
            parquet_reader_requirements(&edge_parquet_bytes, 4, &[edge_decoded]);
        assert_reader_budget_edges(
            edge_parquet_bytes.len() as u64,
            group,
            retained,
            decoded,
            |limits| {
                validate_cell_patch_edge_table_parquet_bytes(
                    &edge_parquet_bytes,
                    &edge_parquet,
                    &fixture.link,
                    limits,
                )
            },
        );
    }
}

#[test]
fn cell_patch_profiles_cover_batch_boundaries_and_three_group_full_decode() {
    for (rows, expected_groups) in [(BATCH_ROWS, 1), (BATCH_ROWS + 1, 2)] {
        let fixture = interpolation_scale_fixture(rows);
        let mut assignment_arrow = Vec::new();
        write_cell_patch_assignment_table_arrow(&mut assignment_arrow, &fixture.link, generous())
            .expect("boundary assignment Arrow");
        let mut edge_arrow = Vec::new();
        write_cell_patch_edge_table_arrow(&mut edge_arrow, &fixture.link, generous())
            .expect("boundary edge Arrow");
        let mut assignment_parquet = Vec::new();
        write_cell_patch_assignment_table_parquet(
            &mut assignment_parquet,
            &fixture.link,
            generous(),
        )
        .expect("boundary assignment Parquet");
        let mut edge_parquet = Vec::new();
        write_cell_patch_edge_table_parquet(&mut edge_parquet, &fixture.link, generous())
            .expect("boundary edge Parquet");
        assert_eq!(
            preflight_cell_patch_assignment_table_arrow_bytes(
                &assignment_arrow,
                &fixture.link,
                generous(),
            )
            .expect("assignment Arrow preflight")
            .record_batch_count(),
            expected_groups,
        );
        assert_eq!(
            preflight_cell_patch_edge_table_arrow_bytes(&edge_arrow, &fixture.link, generous())
                .expect("edge Arrow preflight")
                .record_batch_count(),
            expected_groups,
        );
        assert_eq!(
            preflight_cell_patch_assignment_table_parquet_bytes(
                &assignment_parquet,
                &fixture.link,
                generous(),
            )
            .expect("assignment Parquet preflight")
            .row_group_count(),
            expected_groups,
        );
        assert_eq!(
            preflight_cell_patch_edge_table_parquet_bytes(
                &edge_parquet,
                &fixture.link,
                generous(),
            )
            .expect("edge Parquet preflight")
            .row_group_count(),
            expected_groups,
        );
        if rows == BATCH_ROWS + 1 {
            assert_writer_budget_edges(
                assignment_arrow.len() as u64,
                assignment_arrow_writer_requirements(&fixture.link),
                |limits| {
                    write_cell_patch_assignment_table_arrow(&mut Vec::new(), &fixture.link, limits)
                },
            );
            assert_writer_budget_edges(
                edge_arrow.len() as u64,
                edge_arrow_writer_requirements(&fixture.link),
                |limits| write_cell_patch_edge_table_arrow(&mut Vec::new(), &fixture.link, limits),
            );
            assert_writer_budget_edges(
                assignment_parquet.len() as u64,
                assignment_parquet_writer_requirements(&fixture.link),
                |limits| {
                    write_cell_patch_assignment_table_parquet(
                        &mut Vec::new(),
                        &fixture.link,
                        limits,
                    )
                },
            );
            assert_writer_budget_edges(
                edge_parquet.len() as u64,
                edge_parquet_writer_requirements(&fixture.link),
                |limits| {
                    write_cell_patch_edge_table_parquet(&mut Vec::new(), &fixture.link, limits)
                },
            );
        }
    }

    let fixture = interpolation_scale_fixture(BATCH_ROWS * 2 + 2);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("cell-patch-three-groups").expect("store ID"),
    )
    .expect("store");
    let assignment_arrow =
        publish_cell_patch_assignment_table_arrow(&store, &fixture.link, generous())
            .expect("publish assignment Arrow")
            .into_record();
    let edge_arrow = publish_cell_patch_edge_table_arrow(&store, &fixture.link, generous())
        .expect("publish edge Arrow")
        .into_record();
    let assignment_parquet =
        publish_cell_patch_assignment_table_parquet(&store, &fixture.link, generous())
            .expect("publish assignment Parquet")
            .into_record();
    let edge_parquet = publish_cell_patch_edge_table_parquet(&store, &fixture.link, generous())
        .expect("publish edge Parquet")
        .into_record();
    assert_eq!(
        validate_cell_patch_assignment_table_arrow_from_store(
            &store,
            &assignment_arrow,
            &fixture.link,
            generous(),
        )
        .expect("three-batch assignment Arrow")
        .record_batch_count(),
        3,
    );
    assert_eq!(
        validate_cell_patch_edge_table_arrow_from_store(
            &store,
            &edge_arrow,
            &fixture.link,
            generous(),
        )
        .expect("three-batch edge Arrow")
        .record_batch_count(),
        3,
    );
    assert_eq!(
        validate_cell_patch_assignment_table_parquet_from_store(
            &store,
            &assignment_parquet,
            &fixture.link,
            generous(),
        )
        .expect("three-group assignment Parquet")
        .row_group_count(),
        3,
    );
    assert_eq!(
        validate_cell_patch_edge_table_parquet_from_store(
            &store,
            &edge_parquet,
            &fixture.link,
            generous(),
        )
        .expect("three-group edge Parquet")
        .row_group_count(),
        3,
    );
}
