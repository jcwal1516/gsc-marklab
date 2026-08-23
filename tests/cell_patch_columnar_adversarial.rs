#![cfg(feature = "parquet")]

#[path = "support/cell_patch_columnar.rs"]
mod support;

use std::{collections::BTreeMap, io::Cursor, io::Write};

use arrow::{
    datatypes::Schema,
    ipc::{
        reader::FileReader,
        writer::{FileWriter, IpcWriteOptions},
        MetadataVersion,
    },
    record_batch::RecordBatch,
};
use marklab::{
    preflight_cell_patch_assignment_table_arrow_bytes,
    preflight_cell_patch_assignment_table_parquet_bytes,
    preflight_cell_patch_edge_table_arrow_bytes, preflight_cell_patch_edge_table_parquet_bytes,
    write_cell_patch_assignment_table_arrow, write_cell_patch_assignment_table_parquet,
    write_cell_patch_edge_table_arrow, write_cell_patch_edge_table_parquet,
    CellPatchAssignmentMode, CellPatchLink, EmbeddingColumnarBudgets, MultiscaleColumnarError,
    SpatialArrowFailure, SpatialParquetFailure,
};
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{ColumnOrder, CompressionCodec, Encoding, FileMetaData, Statistics, TypeDefinedOrder},
    schema::types,
    thrift::{TCompactOutputProtocol, TSerializable},
};
use support::{cell_patch_fixture, CellPatchFixtureMode};

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn mode_wire(mode: CellPatchAssignmentMode) -> &'static str {
    match mode {
        CellPatchAssignmentMode::ContainedShared => "contained_shared",
        CellPatchAssignmentMode::DeclaredWeightedInterpolation => "declared_weighted_interpolation",
    }
}

fn expected_metadata(
    link: &CellPatchLink,
    encoding: &str,
    schema: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "marklab.assignment_mode".to_owned(),
            mode_wire(link.mode()).to_owned(),
        ),
        ("marklab.encoding_version".to_owned(), encoding.to_owned()),
        (
            "marklab.expected_cells_artifact_id".to_owned(),
            link.expected_cells_artifact_id().to_string(),
        ),
        (
            "marklab.expected_patches_artifact_id".to_owned(),
            link.expected_patches_artifact_id().to_string(),
        ),
        (
            "marklab.footprint_artifact_id".to_owned(),
            link.patch_footprints_artifact_id().to_string(),
        ),
        (
            "marklab.link_logical_digest".to_owned(),
            link.logical_digest().to_string(),
        ),
        (
            "marklab.owning_slide_id".to_owned(),
            link.owning_slide_id().as_str().to_owned(),
        ),
        (
            "marklab.patch_context_artifact_id".to_owned(),
            link.patch_context_artifact_id().to_string(),
        ),
        (
            "marklab.producer_artifact_id".to_owned(),
            link.producer_artifact_id().to_string(),
        ),
        ("marklab.schema_id".to_owned(), schema.to_owned()),
        ("marklab.schema_version".to_owned(), "1".to_owned()),
    ])
}

fn arrow_footer_bounds(bytes: &[u8]) -> (usize, usize) {
    let length_offset = bytes.len() - 10;
    let length = usize::try_from(i32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ))
    .expect("positive footer length");
    (length_offset - length, length)
}

fn first_arrow_block(bytes: &[u8]) -> (usize, usize, usize) {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    (
        usize::try_from(block.offset()).expect("block offset"),
        usize::try_from(block.metaDataLength()).expect("metadata length"),
        usize::try_from(block.bodyLength()).expect("body length"),
    )
}

fn arrow_node_location(bytes: &[u8], index: usize) -> usize {
    let (offset, metadata, _) = first_arrow_block(bytes);
    let message_bytes = &bytes[offset + 8..offset + metadata];
    let message = arrow::ipc::root_as_message(message_bytes).expect("record message");
    let node = message
        .header_as_record_batch()
        .expect("record batch")
        .nodes()
        .expect("nodes")
        .get(index);
    offset + 8 + (node as *const arrow::ipc::FieldNode as usize - message_bytes.as_ptr() as usize)
}

fn arrow_buffer_location(bytes: &[u8], index: usize) -> (usize, std::ops::Range<usize>) {
    let (offset, metadata, _) = first_arrow_block(bytes);
    let message_bytes = &bytes[offset + 8..offset + metadata];
    let message = arrow::ipc::root_as_message(message_bytes).expect("record message");
    let buffer = message
        .header_as_record_batch()
        .expect("record batch")
        .buffers()
        .expect("buffers")
        .get(index);
    let descriptor = offset
        + 8
        + (buffer as *const arrow::ipc::Buffer as usize - message_bytes.as_ptr() as usize);
    let start = offset + metadata + usize::try_from(buffer.offset()).expect("buffer offset");
    let end = start + usize::try_from(buffer.length()).expect("buffer length");
    (descriptor, start..end)
}

fn first_arrow_block_descriptor(bytes: &[u8]) -> usize {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    footer_start + (block as *const arrow::ipc::Block as usize - footer_bytes.as_ptr() as usize)
}

fn swapped_arrow_columns(bytes: &[u8], left: usize, right: usize) -> Vec<u8> {
    let mut reader = FileReader::try_new(Cursor::new(bytes), None).expect("trusted Arrow reader");
    let original_schema = reader.schema();
    let original_batch = reader
        .next()
        .expect("record batch")
        .expect("trusted record batch");
    assert!(reader.next().is_none());
    let mut fields = original_schema
        .fields()
        .iter()
        .map(|field| field.as_ref().clone())
        .collect::<Vec<_>>();
    let mut columns = original_batch.columns().to_vec();
    fields.swap(left, right);
    columns.swap(left, right);
    let schema = Schema::new_with_metadata(fields, original_schema.metadata().clone());
    let batch =
        RecordBatch::try_new(std::sync::Arc::new(schema.clone()), columns).expect("swapped batch");
    let options = IpcWriteOptions::try_new(64, false, MetadataVersion::V5).expect("IPC options");
    let mut output = Vec::new();
    let mut writer =
        FileWriter::try_new_with_options(&mut output, &schema, options).expect("Arrow writer");
    writer.write(&batch).expect("swapped batch write");
    writer.finish().expect("finish swapped Arrow");
    output
}

fn trusted_parquet_metadata(bytes: &[u8]) -> ParquetMetaData {
    let mut file = tempfile::tempfile().expect("temporary Parquet file");
    file.write_all(bytes).expect("trusted Parquet bytes");
    ParquetMetaDataReader::new()
        .parse_and_finish(&file)
        .expect("trusted Parquet metadata")
}

fn raw_parquet_metadata(metadata: &ParquetMetaData) -> FileMetaData {
    let file = metadata.file_metadata();
    let columns = file.schema_descr().num_columns();
    FileMetaData {
        version: file.version(),
        schema: types::to_thrift(file.schema()).expect("Thrift schema"),
        num_rows: file.num_rows(),
        row_groups: metadata
            .row_groups()
            .iter()
            .map(|group| group.to_thrift())
            .collect(),
        key_value_metadata: file.key_value_metadata().cloned(),
        created_by: file.created_by().map(str::to_owned),
        column_orders: Some(
            (0..columns)
                .map(|_| ColumnOrder::TYPEORDER(TypeDefinedOrder {}))
                .collect(),
        ),
        encryption_algorithm: None,
        footer_signing_key_metadata: None,
    }
}

fn rewrite_parquet_footer(bytes: &[u8], mutate: impl FnOnce(&mut FileMetaData)) -> Vec<u8> {
    let metadata = trusted_parquet_metadata(bytes);
    let mut raw = raw_parquet_metadata(&metadata);
    mutate(&mut raw);
    let mut footer = Vec::new();
    {
        let mut protocol = TCompactOutputProtocol::new(&mut footer);
        raw.write_to_out_protocol(&mut protocol)
            .expect("rewrite footer");
    }
    let original_footer_start = bytes.len()
        - 8
        - u32::from_le_bytes(
            bytes[bytes.len() - 8..bytes.len() - 4]
                .try_into()
                .expect("footer length"),
        ) as usize;
    let mut output = bytes[..original_footer_start].to_vec();
    output.extend_from_slice(&footer);
    output.extend_from_slice(
        &u32::try_from(footer.len())
            .expect("footer length")
            .to_le_bytes(),
    );
    output.extend_from_slice(b"PAR1");
    output
}

fn read_varint(bytes: &[u8], cursor: &mut usize) -> u64 {
    let mut value = 0_u64;
    for shift in (0..35).step_by(7) {
        let byte = bytes[*cursor];
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
    }
    panic!("oversized fixture varint")
}

fn read_compact_i32(bytes: &[u8], cursor: &mut usize) -> i32 {
    let encoded = read_varint(bytes, cursor);
    ((encoded >> 1) as i64 ^ -((encoded & 1) as i64)) as i32
}

fn expect_field(bytes: &[u8], cursor: &mut usize, delta: u8, compact_type: u8) {
    let header = bytes[*cursor];
    *cursor += 1;
    assert_eq!(header >> 4, delta);
    assert_eq!(header & 0x0f, compact_type);
}

fn page_v2_layout(bytes: &[u8], start: usize) -> (usize, usize, usize) {
    let mut cursor = start;
    for _ in 0..3 {
        expect_field(bytes, &mut cursor, 1, 5);
        read_compact_i32(bytes, &mut cursor);
    }
    expect_field(bytes, &mut cursor, 5, 12);
    expect_field(bytes, &mut cursor, 1, 5);
    read_compact_i32(bytes, &mut cursor);
    expect_field(bytes, &mut cursor, 1, 5);
    let null_count_at = cursor;
    read_compact_i32(bytes, &mut cursor);
    expect_field(bytes, &mut cursor, 1, 5);
    read_compact_i32(bytes, &mut cursor);
    expect_field(bytes, &mut cursor, 1, 5);
    read_compact_i32(bytes, &mut cursor);
    expect_field(bytes, &mut cursor, 1, 5);
    let definition_length =
        usize::try_from(read_compact_i32(bytes, &mut cursor)).expect("definition length");
    expect_field(bytes, &mut cursor, 1, 5);
    assert_eq!(read_compact_i32(bytes, &mut cursor), 0);
    expect_field(bytes, &mut cursor, 1, 2);
    assert_eq!(bytes[cursor], 0, "nested stop");
    cursor += 1;
    assert_eq!(bytes[cursor], 0, "page-header stop");
    cursor += 1;
    (null_count_at, cursor, definition_length)
}

fn parquet_column_start(bytes: &[u8], column: usize) -> usize {
    usize::try_from(
        trusted_parquet_metadata(bytes)
            .row_group(0)
            .column(column)
            .data_page_offset(),
    )
    .expect("column offset")
}

#[test]
fn cell_patch_application_metadata_is_exact_in_both_formats_and_modes() {
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        for (assignment, encoding, schema) in [
            (
                true,
                "marklab.arrow-ipc.cell-patch-assignment-table.v1",
                "marklab.cell_patch_assignment_table",
            ),
            (
                false,
                "marklab.arrow-ipc.cell-patch-edge-table.v1",
                "marklab.cell_patch_edge_table",
            ),
        ] {
            let mut bytes = Vec::new();
            if assignment {
                write_cell_patch_assignment_table_arrow(&mut bytes, &fixture.link, budgets())
                    .expect("assignment Arrow");
            } else {
                write_cell_patch_edge_table_arrow(&mut bytes, &fixture.link, budgets())
                    .expect("edge Arrow");
            }
            let reader = FileReader::try_new(Cursor::new(bytes), None).expect("Arrow reader");
            assert_eq!(
                BTreeMap::from_iter(reader.schema().metadata().clone()),
                expected_metadata(&fixture.link, encoding, schema),
            );
        }
        for (assignment, encoding, schema) in [
            (
                true,
                "marklab.parquet.cell-patch-assignment-table.v1",
                "marklab.cell_patch_assignment_table",
            ),
            (
                false,
                "marklab.parquet.cell-patch-edge-table.v1",
                "marklab.cell_patch_edge_table",
            ),
        ] {
            let mut bytes = Vec::new();
            if assignment {
                write_cell_patch_assignment_table_parquet(&mut bytes, &fixture.link, budgets())
                    .expect("assignment Parquet");
            } else {
                write_cell_patch_edge_table_parquet(&mut bytes, &fixture.link, budgets())
                    .expect("edge Parquet");
            }
            let entries = trusted_parquet_metadata(&bytes)
                .file_metadata()
                .key_value_metadata()
                .expect("application metadata")
                .iter()
                .map(|entry| {
                    (
                        entry.key.clone(),
                        entry.value.clone().expect("metadata value"),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(entries, expected_metadata(&fixture.link, encoding, schema));
        }
    }
}

#[test]
fn cell_patch_arrow_preflight_rejects_nodes_buffers_validity_fillers_and_rows() {
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        let mut assignments = Vec::new();
        write_cell_patch_assignment_table_arrow(&mut assignments, &fixture.link, budgets())
            .expect("assignment Arrow");
        let mut edges = Vec::new();
        write_cell_patch_edge_table_arrow(&mut edges, &fixture.link, budgets())
            .expect("edge Arrow");

        let mut wrong_assignment_nulls = assignments.clone();
        let node = arrow_node_location(&wrong_assignment_nulls, 0);
        wrong_assignment_nulls[node + 8..node + 16].copy_from_slice(&1_i64.to_le_bytes());
        assert!(matches!(
            preflight_cell_patch_assignment_table_arrow_bytes(
                &wrong_assignment_nulls,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidFieldNodes,
            })
        ));

        let mut wrong_assignment_validity = assignments.clone();
        let (_, validity) = arrow_buffer_location(&wrong_assignment_validity, 0);
        wrong_assignment_validity[validity.start] = 0x07;
        assert!(matches!(
            preflight_cell_patch_assignment_table_arrow_bytes(
                &wrong_assignment_validity,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidBuffers,
            })
        ));

        for node_index in [2, 3] {
            let mut wrong_nulls = edges.clone();
            let node = arrow_node_location(&wrong_nulls, node_index);
            let wrong = match mode {
                CellPatchFixtureMode::Contained => 2_i64,
                CellPatchFixtureMode::Interpolation => 1_i64,
            };
            wrong_nulls[node + 8..node + 16].copy_from_slice(&wrong.to_le_bytes());
            assert!(matches!(
                preflight_cell_patch_edge_table_arrow_bytes(&wrong_nulls, &fixture.link, budgets(),),
                Err(MultiscaleColumnarError::Arrow {
                    reason: SpatialArrowFailure::InvalidFieldNodes,
                })
            ));
        }
        for buffer_index in [5, 7] {
            let mut wrong_weight_validity = edges.clone();
            let (_, validity) = arrow_buffer_location(&wrong_weight_validity, buffer_index);
            wrong_weight_validity[validity.start] = match mode {
                CellPatchFixtureMode::Contained => 0xff,
                CellPatchFixtureMode::Interpolation => 0x07,
            };
            assert!(matches!(
                preflight_cell_patch_edge_table_arrow_bytes(
                    &wrong_weight_validity,
                    &fixture.link,
                    budgets(),
                ),
                Err(MultiscaleColumnarError::Arrow {
                    reason: SpatialArrowFailure::InvalidBuffers,
                })
            ));
        }
        if mode == CellPatchFixtureMode::Contained {
            for buffer_index in [6, 8] {
                let mut hidden_weight = edges.clone();
                let (_, values) = arrow_buffer_location(&hidden_weight, buffer_index);
                hidden_weight[values.start] = 1;
                assert!(matches!(
                    preflight_cell_patch_edge_table_arrow_bytes(
                        &hidden_weight,
                        &fixture.link,
                        budgets(),
                    ),
                    Err(MultiscaleColumnarError::Arrow {
                        reason: SpatialArrowFailure::InvalidCanonicalRows,
                    })
                ));
            }
        }

        let mut negative_buffer = assignments.clone();
        let (descriptor, _) = arrow_buffer_location(&negative_buffer, 0);
        negative_buffer[descriptor + 8..descriptor + 16].copy_from_slice(&(-1_i64).to_le_bytes());
        assert!(matches!(
            preflight_cell_patch_assignment_table_arrow_bytes(
                &negative_buffer,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidBuffers,
            })
        ));

        let mut wrong_prefix = assignments.clone();
        let (_, starts) = arrow_buffer_location(&wrong_prefix, 11);
        wrong_prefix[starts.start] = 1;
        assert!(matches!(
            preflight_cell_patch_assignment_table_arrow_bytes(
                &wrong_prefix,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidCanonicalRows,
            })
        ));
        let mut wrong_assignment_row = edges.clone();
        let (_, assignment_rows) = arrow_buffer_location(&wrong_assignment_row, 1);
        wrong_assignment_row[assignment_rows.start] = 1;
        assert!(matches!(
            preflight_cell_patch_edge_table_arrow_bytes(
                &wrong_assignment_row,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidCanonicalRows,
            })
        ));

        let mut wrong_status = assignments.clone();
        let status_at = wrong_status
            .windows(b"assigned".len())
            .position(|window| window == b"assigned")
            .expect("assignment status");
        wrong_status[status_at] ^= 0x20;
        assert!(matches!(
            preflight_cell_patch_assignment_table_arrow_bytes(
                &wrong_status,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidCanonicalRows,
            })
        ));

        for (bytes, needle, assignment) in [
            (&assignments, b"cell-00000000".as_slice(), true),
            (&edges, b"patch-00000000".as_slice(), false),
        ] {
            let mut wrong_row = bytes.clone();
            let at = wrong_row
                .windows(needle.len())
                .position(|window| window == needle)
                .expect("canonical row value");
            wrong_row[at] ^= 0x20;
            let error = if assignment {
                preflight_cell_patch_assignment_table_arrow_bytes(
                    &wrong_row,
                    &fixture.link,
                    budgets(),
                )
                .map(|_| ())
            } else {
                preflight_cell_patch_edge_table_arrow_bytes(&wrong_row, &fixture.link, budgets())
                    .map(|_| ())
            };
            assert!(matches!(
                error,
                Err(MultiscaleColumnarError::Arrow {
                    reason: SpatialArrowFailure::InvalidCanonicalRows,
                })
            ));
        }
    }
}

#[test]
fn cell_patch_arrow_preflight_rejects_metadata_and_message_bounds() {
    let fixture = cell_patch_fixture(CellPatchFixtureMode::Interpolation);
    let mut canonical = Vec::new();
    write_cell_patch_edge_table_arrow(&mut canonical, &fixture.link, budgets())
        .expect("edge Arrow");

    let mut bad_footer_length = canonical.clone();
    let footer_length_offset = bad_footer_length.len() - 10;
    bad_footer_length[footer_length_offset..footer_length_offset + 4]
        .copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(matches!(
        preflight_cell_patch_edge_table_arrow_bytes(&bad_footer_length, &fixture.link, budgets(),),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidFooterLength,
        })
    ));

    assert!(matches!(
        preflight_cell_patch_edge_table_arrow_bytes(
            &swapped_arrow_columns(&canonical, 2, 3),
            &fixture.link,
            budgets(),
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidSchema,
        })
    ));

    let mut oversized_message = canonical.clone();
    let block = first_arrow_block_descriptor(&oversized_message);
    oversized_message[block + 8..block + 12].copy_from_slice(&(128 * 1024_i32).to_le_bytes());
    assert!(matches!(
        preflight_cell_patch_edge_table_arrow_bytes(&oversized_message, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBlock,
        })
    ));

    let from = b"marklab.arrow-ipc.cell-patch-edge-table.v1";
    let to = b"marklab.arrow-ipc.cell-patch-edge-table.x1";
    assert_eq!(from.len(), to.len());
    let mut wrong_metadata = canonical.clone();
    let offsets = wrong_metadata
        .windows(from.len())
        .enumerate()
        .filter_map(|(index, window)| (window == from).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 2);
    for offset in offsets {
        wrong_metadata[offset..offset + to.len()].copy_from_slice(to);
    }
    assert!(matches!(
        preflight_cell_patch_edge_table_arrow_bytes(&wrong_metadata, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidApplicationMetadata,
        })
    ));
}

#[test]
fn cell_patch_parquet_preflight_rejects_levels_null_counts_and_rows() {
    for mode in [
        CellPatchFixtureMode::Contained,
        CellPatchFixtureMode::Interpolation,
    ] {
        let fixture = cell_patch_fixture(mode);
        let mut assignments = Vec::new();
        write_cell_patch_assignment_table_parquet(&mut assignments, &fixture.link, budgets())
            .expect("assignment Parquet");
        let mut edges = Vec::new();
        write_cell_patch_edge_table_parquet(&mut edges, &fixture.link, budgets())
            .expect("edge Parquet");

        for column in [2, 3] {
            let start = parquet_column_start(&edges, column);
            let (null_count, body, definitions) = page_v2_layout(&edges, start);
            assert!(definitions > 0);
            let mut wrong_nulls = edges.clone();
            assert_eq!(wrong_nulls[null_count] & 0x80, 0);
            wrong_nulls[null_count] = match mode {
                CellPatchFixtureMode::Contained => 4,
                CellPatchFixtureMode::Interpolation => 2,
            };
            assert!(matches!(
                preflight_cell_patch_edge_table_parquet_bytes(
                    &wrong_nulls,
                    &fixture.link,
                    budgets(),
                ),
                Err(MultiscaleColumnarError::Parquet {
                    reason: SpatialParquetFailure::InvalidPage,
                })
            ));

            let mut wrong_levels = edges.clone();
            wrong_levels[body + definitions - 1] ^= 1;
            assert!(matches!(
                preflight_cell_patch_edge_table_parquet_bytes(
                    &wrong_levels,
                    &fixture.link,
                    budgets(),
                ),
                Err(MultiscaleColumnarError::Parquet {
                    reason: SpatialParquetFailure::InvalidPage,
                })
            ));
        }

        for (bytes, needle, assignment) in [
            (&assignments, b"cell-00000000".as_slice(), true),
            (&edges, b"patch-00000000".as_slice(), false),
        ] {
            let mut wrong_row = bytes.clone();
            let at = wrong_row
                .windows(needle.len())
                .position(|window| window == needle)
                .expect("canonical row value");
            wrong_row[at] ^= 0x20;
            let error = if assignment {
                preflight_cell_patch_assignment_table_parquet_bytes(
                    &wrong_row,
                    &fixture.link,
                    budgets(),
                )
                .map(|_| ())
            } else {
                preflight_cell_patch_edge_table_parquet_bytes(&wrong_row, &fixture.link, budgets())
                    .map(|_| ())
            };
            assert!(matches!(
                error,
                Err(MultiscaleColumnarError::Parquet {
                    reason: SpatialParquetFailure::InvalidCanonicalRows,
                })
            ));
        }

        let assignment_start = parquet_column_start(&assignments, 4);
        let (_, assignment_body, definition_length) =
            page_v2_layout(&assignments, assignment_start);
        assert_eq!(definition_length, 0);
        let mut wrong_prefix = assignments.clone();
        wrong_prefix[assignment_body] = 1;
        assert!(matches!(
            preflight_cell_patch_assignment_table_parquet_bytes(
                &wrong_prefix,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));

        let edge_start = parquet_column_start(&edges, 0);
        let (_, edge_body, definition_length) = page_v2_layout(&edges, edge_start);
        assert_eq!(definition_length, 0);
        let mut wrong_assignment_row = edges.clone();
        wrong_assignment_row[edge_body] = 1;
        assert!(matches!(
            preflight_cell_patch_edge_table_parquet_bytes(
                &wrong_assignment_row,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));

        let mut wrong_status = assignments.clone();
        let status_at = wrong_status
            .windows(b"assigned".len())
            .position(|window| window == b"assigned")
            .expect("assignment status");
        wrong_status[status_at] ^= 0x20;
        assert!(matches!(
            preflight_cell_patch_assignment_table_parquet_bytes(
                &wrong_status,
                &fixture.link,
                budgets(),
            ),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));
    }
}

#[test]
fn cell_patch_parquet_preflight_rejects_schema_metadata_and_forbidden_features() {
    let fixture = cell_patch_fixture(CellPatchFixtureMode::Interpolation);
    let mut canonical = Vec::new();
    write_cell_patch_edge_table_parquet(&mut canonical, &fixture.link, budgets())
        .expect("edge Parquet");

    let mut bad_magic = canonical.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_cell_patch_edge_table_parquet_bytes(&bad_magic, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMagic,
        })
    ));

    let mut oversized_footer = canonical.clone();
    let length_at = oversized_footer.len() - 8;
    oversized_footer[length_at..length_at + 4]
        .copy_from_slice(&(1024 * 1024_u32 + 1).to_le_bytes());
    assert!(matches!(
        preflight_cell_patch_edge_table_parquet_bytes(&oversized_footer, &fixture.link, budgets(),),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidFooterLength,
        })
    ));

    let wrong_schema = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.schema[0].name = "marklab_cell_patch_edge_tablx".to_owned();
    });
    assert!(matches!(
        preflight_cell_patch_edge_table_parquet_bytes(&wrong_schema, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidSchema,
        })
    ));

    let wrong_metadata = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.key_value_metadata.as_mut().expect("metadata")[0].key =
            "marklab.assignment_modx".to_owned();
    });
    assert!(matches!(
        preflight_cell_patch_edge_table_parquet_bytes(&wrong_metadata, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMetadata,
        })
    ));

    let forbidden = [
        rewrite_parquet_footer(&canonical, |metadata| {
            metadata.row_groups[0].columns[0]
                .meta_data
                .as_mut()
                .expect("column metadata")
                .codec = CompressionCodec::SNAPPY;
        }),
        rewrite_parquet_footer(&canonical, |metadata| {
            metadata.row_groups[0].columns[0]
                .meta_data
                .as_mut()
                .expect("column metadata")
                .encodings
                .push(Encoding::RLE_DICTIONARY);
        }),
        rewrite_parquet_footer(&canonical, |metadata| {
            metadata.row_groups[0].columns[0]
                .meta_data
                .as_mut()
                .expect("column metadata")
                .statistics = Some(Statistics {
                max: None,
                min: None,
                null_count: Some(0),
                distinct_count: None,
                max_value: None,
                min_value: None,
                is_max_value_exact: None,
                is_min_value_exact: None,
            });
        }),
        rewrite_parquet_footer(&canonical, |metadata| {
            metadata.row_groups[0].columns[0].offset_index_offset = Some(1);
            metadata.row_groups[0].columns[0].offset_index_length = Some(1);
        }),
    ];
    for (index, bytes) in forbidden.into_iter().enumerate() {
        let observed =
            preflight_cell_patch_edge_table_parquet_bytes(&bytes, &fixture.link, budgets());
        let rejected = if index == 1 {
            matches!(
                &observed,
                Err(MultiscaleColumnarError::Parquet {
                    reason: SpatialParquetFailure::InvalidFooter
                        | SpatialParquetFailure::ForbiddenFeature,
                })
            )
        } else {
            matches!(
                &observed,
                Err(MultiscaleColumnarError::Parquet {
                    reason: SpatialParquetFailure::ForbiddenFeature,
                })
            )
        };
        assert!(rejected, "forbidden case {index} produced {observed:?}");
    }

    let oversized_range = rewrite_parquet_footer(&canonical, |metadata| {
        let column = metadata.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column metadata");
        column.total_compressed_size = i64::MAX;
        column.total_uncompressed_size = i64::MAX;
    });
    assert!(matches!(
        preflight_cell_patch_edge_table_parquet_bytes(&oversized_range, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Parquet { .. })
    ));

    let truncated = &canonical[..canonical.len() - 1];
    assert!(matches!(
        preflight_cell_patch_edge_table_parquet_bytes(truncated, &fixture.link, budgets()),
        Err(MultiscaleColumnarError::Parquet { .. })
    ));
}
