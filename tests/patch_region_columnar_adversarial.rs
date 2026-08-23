#![cfg(feature = "parquet")]

#[allow(dead_code, unused_imports)]
#[path = "patch_region_link/support.rs"]
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
    preflight_patch_region_link_arrow_bytes, preflight_patch_region_link_parquet_bytes,
    write_patch_region_link_arrow, write_patch_region_link_parquet, ContentDigest,
    EmbeddingColumnarBudgets, MultiscaleColumnarError, PatchRegionAssessment, PatchRegionLink,
    SpatialArrowFailure, SpatialParquetFailure,
};
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{ColumnOrder, CompressionCodec, Encoding, FileMetaData, Statistics, TypeDefinedOrder},
    schema::types,
    thrift::{TCompactOutputProtocol, TSerializable},
};

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn link() -> PatchRegionLink {
    let fixture = support::fixture();
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        support::declarations(),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("assessment");
    let assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        support::artifact(b"patch-region-hostile-assessment"),
        ContentDigest::from_bytes(&assessment_bytes),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("patch-region link")
}

fn expected_metadata(link: &PatchRegionLink, encoding: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "marklab.assessed_pair_count".to_owned(),
            link.assessed_pair_count().to_string(),
        ),
        (
            "marklab.assessment_artifact_id".to_owned(),
            link.assessment_artifact_id().to_string(),
        ),
        (
            "marklab.assessment_policy".to_owned(),
            "expected_cartesian_exhaustive".to_owned(),
        ),
        (
            "marklab.converter_artifact_id".to_owned(),
            link.converter_artifact_id().to_string(),
        ),
        ("marklab.encoding_version".to_owned(), encoding.to_owned()),
        (
            "marklab.expected_patches_artifact_id".to_owned(),
            link.expected_patches_artifact_id().to_string(),
        ),
        (
            "marklab.expected_regions_artifact_id".to_owned(),
            link.expected_regions_artifact_id().to_string(),
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
            "marklab.schema_id".to_owned(),
            "marklab.patch_region_link".to_owned(),
        ),
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
fn patch_region_application_metadata_is_exact_in_both_formats() {
    let link = link();
    let mut arrow = Vec::new();
    write_patch_region_link_arrow(&mut arrow, &link, budgets()).expect("Arrow");
    let reader = FileReader::try_new(Cursor::new(&arrow), None).expect("Arrow reader");
    assert_eq!(
        BTreeMap::from_iter(reader.schema().metadata().clone()),
        expected_metadata(&link, "marklab.arrow-ipc.patch-region-link.v1")
    );
    let (footer_start, footer_length) = arrow_footer_bounds(&arrow);
    let footer = arrow::ipc::root_as_footer(&arrow[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let arrow_pairs = footer
        .schema()
        .expect("Arrow schema")
        .custom_metadata()
        .expect("Arrow application metadata")
        .iter()
        .map(|entry| {
            (
                entry.key().expect("metadata key").to_owned(),
                entry.value().expect("metadata value").to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        arrow_pairs,
        expected_metadata(&link, "marklab.arrow-ipc.patch-region-link.v1")
            .into_iter()
            .collect::<Vec<_>>()
    );

    let mut parquet = Vec::new();
    write_patch_region_link_parquet(&mut parquet, &link, budgets()).expect("Parquet");
    let metadata = trusted_parquet_metadata(&parquet);
    let raw_entries = metadata
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
        .collect::<Vec<_>>();
    assert_eq!(
        raw_entries,
        expected_metadata(&link, "marklab.parquet.patch-region-link.v1")
            .into_iter()
            .collect::<Vec<_>>()
    );
}

#[test]
fn patch_region_arrow_preflight_rejects_nodes_buffers_rows_schema_and_bounds() {
    let link = link();
    let mut canonical = Vec::new();
    write_patch_region_link_arrow(&mut canonical, &link, budgets()).expect("Arrow");

    let mut wrong_nulls = canonical.clone();
    let node = arrow_node_location(&wrong_nulls, 0);
    wrong_nulls[node + 8..node + 16].copy_from_slice(&1_i64.to_le_bytes());
    assert!(matches!(
        preflight_patch_region_link_arrow_bytes(&wrong_nulls, &link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidFieldNodes,
        })
    ));

    for buffer_index in [0, 3, 6, 9, 11] {
        let mut wrong_validity = canonical.clone();
        let (_, validity) = arrow_buffer_location(&wrong_validity, buffer_index);
        wrong_validity[validity.start] = 0x07;
        assert!(matches!(
            preflight_patch_region_link_arrow_bytes(&wrong_validity, &link, budgets()),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidBuffers,
            })
        ));
    }

    let mut negative_buffer = canonical.clone();
    let (descriptor, _) = arrow_buffer_location(&negative_buffer, 0);
    negative_buffer[descriptor + 8..descriptor + 16].copy_from_slice(&(-1_i64).to_le_bytes());
    assert!(matches!(
        preflight_patch_region_link_arrow_bytes(&negative_buffer, &link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBuffers,
        })
    ));

    for needle in [
        b"patch-a".as_slice(),
        b"region-a".as_slice(),
        b"fully_contained".as_slice(),
    ] {
        let mut wrong_row = canonical.clone();
        let at = wrong_row
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("canonical row value");
        wrong_row[at] ^= 0x20;
        assert!(matches!(
            preflight_patch_region_link_arrow_bytes(&wrong_row, &link, budgets()),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidCanonicalRows,
            })
        ));
    }

    for buffer_index in [10, 12] {
        let mut wrong_fraction = canonical.clone();
        let (_, values) = arrow_buffer_location(&wrong_fraction, buffer_index);
        wrong_fraction[values.start] ^= 1;
        assert!(matches!(
            preflight_patch_region_link_arrow_bytes(&wrong_fraction, &link, budgets()),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidCanonicalRows,
            })
        ));
    }

    let mut wrong_padding = canonical.clone();
    let (_, validity) = arrow_buffer_location(&wrong_padding, 0);
    let (_, offsets) = arrow_buffer_location(&wrong_padding, 1);
    assert!(validity.end < offsets.start);
    wrong_padding[validity.end] = 1;
    assert!(matches!(
        preflight_patch_region_link_arrow_bytes(&wrong_padding, &link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBuffers,
        })
    ));

    assert!(matches!(
        preflight_patch_region_link_arrow_bytes(
            &swapped_arrow_columns(&canonical, 3, 4),
            &link,
            budgets(),
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidSchema,
        })
    ));

    let from = b"marklab.arrow-ipc.patch-region-link.v1";
    let to = b"marklab.arrow-ipc.patch-region-link.x1";
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
        preflight_patch_region_link_arrow_bytes(&wrong_metadata, &link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidApplicationMetadata,
        })
    ));

    let mut oversized_message = canonical.clone();
    let block = first_arrow_block_descriptor(&oversized_message);
    oversized_message[block + 8..block + 12].copy_from_slice(&(128 * 1024_i32).to_le_bytes());
    assert!(matches!(
        preflight_patch_region_link_arrow_bytes(&oversized_message, &link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBlock,
        })
    ));

    let mut bad_footer_length = canonical.clone();
    let footer_length_offset = bad_footer_length.len() - 10;
    bad_footer_length[footer_length_offset..footer_length_offset + 4]
        .copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(matches!(
        preflight_patch_region_link_arrow_bytes(&bad_footer_length, &link, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidFooterLength,
        })
    ));
}

#[test]
fn patch_region_parquet_preflight_rejects_pages_rows_schema_and_forbidden_features() {
    let link = link();
    let mut canonical = Vec::new();
    write_patch_region_link_parquet(&mut canonical, &link, budgets()).expect("Parquet");

    for column in 0..5 {
        let start = parquet_column_start(&canonical, column);
        let (null_count, _, definitions) = page_v2_layout(&canonical, start);
        assert_eq!(definitions, 0);
        let mut wrong_nulls = canonical.clone();
        wrong_nulls[null_count] = 2;
        assert!(matches!(
            preflight_patch_region_link_parquet_bytes(&wrong_nulls, &link, budgets()),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidPage,
            })
        ));
    }

    for needle in [
        b"patch-a".as_slice(),
        b"region-a".as_slice(),
        b"fully_contained".as_slice(),
    ] {
        let mut wrong_row = canonical.clone();
        let at = wrong_row
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("canonical row value");
        wrong_row[at] ^= 0x20;
        assert!(matches!(
            preflight_patch_region_link_parquet_bytes(&wrong_row, &link, budgets()),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));
    }

    for column in [3, 4] {
        let start = parquet_column_start(&canonical, column);
        let (_, body, definitions) = page_v2_layout(&canonical, start);
        assert_eq!(definitions, 0);
        let mut wrong_fraction = canonical.clone();
        wrong_fraction[body] ^= 1;
        assert!(matches!(
            preflight_patch_region_link_parquet_bytes(&wrong_fraction, &link, budgets()),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));
    }

    let mut bad_magic = canonical.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&bad_magic, &link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMagic,
        })
    ));

    let mut oversized_footer = canonical.clone();
    let length_at = oversized_footer.len() - 8;
    oversized_footer[length_at..length_at + 4]
        .copy_from_slice(&(1024 * 1024_u32 + 1).to_le_bytes());
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&oversized_footer, &link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidFooterLength,
        })
    ));

    let wrong_schema = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.schema[0].name = "marklab_patch_region_linx".to_owned();
    });
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&wrong_schema, &link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidSchema,
        })
    ));

    let wrong_u64_annotation = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.schema[4].converted_type = None;
    });
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&wrong_u64_annotation, &link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidSchema,
        })
    ));

    let wrong_metadata = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.key_value_metadata.as_mut().expect("metadata")[0].key =
            "marklab.assessed_pair_counx".to_owned();
    });
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&wrong_metadata, &link, budgets()),
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
        let observed = preflight_patch_region_link_parquet_bytes(&bytes, &link, budgets());
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

    let missing_encoding_statistics = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column metadata")
            .encoding_stats = None;
    });
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&missing_encoding_statistics, &link, budgets(),),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidRowGroup,
        })
    ));

    let missing_column_order = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.column_orders = None;
    });
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&missing_column_order, &link, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMetadata,
        })
    ));

    let oversized_range = rewrite_parquet_footer(&canonical, |metadata| {
        let column = metadata.row_groups[0].columns[0]
            .meta_data
            .as_mut()
            .expect("column metadata");
        column.total_compressed_size = i64::MAX;
        column.total_uncompressed_size = i64::MAX;
    });
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(&oversized_range, &link, budgets()),
        Err(MultiscaleColumnarError::Parquet { .. })
    ));
    assert!(matches!(
        preflight_patch_region_link_parquet_bytes(
            &canonical[..canonical.len() - 1],
            &link,
            budgets(),
        ),
        Err(MultiscaleColumnarError::Parquet { .. })
    ));
}
