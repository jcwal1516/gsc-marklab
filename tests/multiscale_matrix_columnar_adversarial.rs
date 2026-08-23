#![cfg(feature = "parquet")]

#[allow(dead_code)]
#[path = "support/multiscale_matrix_columnar.rs"]
mod support;

use std::{collections::BTreeMap, io::Cursor, io::Write, mem::size_of};

use arrow::ipc::reader::FileReader;
use marklab::{
    preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
    preflight_region_embedding_table_arrow_bytes, preflight_slide_embedding_table_parquet_bytes,
    write_patch_embedding_table_arrow, write_patch_embedding_table_parquet,
    write_region_embedding_table_arrow, write_region_embedding_table_parquet,
    write_slide_embedding_table_arrow, write_slide_embedding_table_parquet, ArtifactId,
    ContentDigest, EmbeddingColumnarBudgets, MultiscaleColumnarError, SpatialArrowFailure,
    SpatialParquetFailure,
};
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{ColumnOrder, CompressionCodec, Encoding, FileMetaData, Statistics, TypeDefinedOrder},
    schema::types,
    thrift::{TCompactOutputProtocol, TSerializable},
};

use support::{matrix_fixture, LARGE};

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, LARGE as u64)
}

struct MetadataValues<'a> {
    entity_kind: &'a str,
    expected_entities_artifact_id: ArtifactId,
    logical_digest: ContentDigest,
    owning_slide_id: &'a str,
    provenance_artifact_id: ArtifactId,
    schema_id: &'a str,
    support_artifact_id: ArtifactId,
}

fn expected_metadata(encoding: &str, values: MetadataValues<'_>) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("marklab.encoding_version".to_owned(), encoding.to_owned()),
        (
            "marklab.entity_kind".to_owned(),
            values.entity_kind.to_owned(),
        ),
        (
            "marklab.expected_entities_artifact_id".to_owned(),
            values.expected_entities_artifact_id.to_string(),
        ),
        (
            "marklab.logical_digest".to_owned(),
            values.logical_digest.to_string(),
        ),
        (
            "marklab.owning_slide_id".to_owned(),
            values.owning_slide_id.to_owned(),
        ),
        (
            "marklab.provenance_artifact_id".to_owned(),
            values.provenance_artifact_id.to_string(),
        ),
        ("marklab.schema_id".to_owned(), values.schema_id.to_owned()),
        ("marklab.schema_version".to_owned(), "1".to_owned()),
        (
            "marklab.support_artifact_id".to_owned(),
            values.support_artifact_id.to_string(),
        ),
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

fn first_arrow_block(bytes: &[u8]) -> (usize, usize) {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    (
        usize::try_from(block.offset()).expect("block offset"),
        usize::try_from(block.metaDataLength()).expect("metadata length"),
    )
}

fn arrow_buffer_range(bytes: &[u8], index: usize) -> std::ops::Range<usize> {
    let (offset, metadata) = first_arrow_block(bytes);
    let message =
        arrow::ipc::root_as_message(&bytes[offset + 8..offset + metadata]).expect("record message");
    let buffer = message
        .header_as_record_batch()
        .expect("record batch")
        .buffers()
        .expect("buffers")
        .get(index);
    let start = offset + metadata + usize::try_from(buffer.offset()).expect("buffer offset");
    let end = start + usize::try_from(buffer.length()).expect("buffer length");
    start..end
}

fn replace_all_same_length(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    assert_eq!(from.len(), to.len());
    let offsets = bytes
        .windows(from.len())
        .enumerate()
        .filter_map(|(index, window)| (window == from).then_some(index))
        .collect::<Vec<_>>();
    for offset in &offsets {
        bytes[*offset..*offset + to.len()].copy_from_slice(to);
    }
    offsets.len()
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
    let mut raw = raw_parquet_metadata(&trusted_parquet_metadata(bytes));
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

fn page_v2_layout(bytes: &[u8], start: usize) -> (usize, usize, usize, usize) {
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
    let repetition_length =
        usize::try_from(read_compact_i32(bytes, &mut cursor)).expect("repetition length");
    expect_field(bytes, &mut cursor, 1, 2);
    assert_eq!(bytes[cursor], 0, "nested stop");
    cursor += 1;
    assert_eq!(bytes[cursor], 0, "page-header stop");
    cursor += 1;
    (null_count_at, cursor, definition_length, repetition_length)
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

macro_rules! assert_metadata {
    ($table:expr, $entity:expr, $schema:expr, $arrow_writer:path, $parquet_writer:path,
        $arrow_encoding:expr, $parquet_encoding:expr) => {{
        let table = $table;
        let expected = |encoding| {
            expected_metadata(
                encoding,
                MetadataValues {
                    entity_kind: $entity,
                    expected_entities_artifact_id: table.expected_entities_artifact_id(),
                    logical_digest: table.logical_digest(),
                    owning_slide_id: table.owning_slide_id().as_str(),
                    provenance_artifact_id: table.provenance_artifact_id(),
                    schema_id: $schema,
                    support_artifact_id: table.support_artifact_id(),
                },
            )
        };
        let mut arrow = Vec::new();
        $arrow_writer(&mut arrow, table, budgets()).expect("Arrow matrix");
        let reader = FileReader::try_new(Cursor::new(&arrow), None).expect("Arrow reader");
        assert_eq!(
            BTreeMap::from_iter(reader.schema().metadata().clone()),
            expected($arrow_encoding)
        );
        let (footer_start, footer_length) = arrow_footer_bounds(&arrow);
        let footer = arrow::ipc::root_as_footer(&arrow[footer_start..footer_start + footer_length])
            .expect("Arrow footer");
        let pairs = footer
            .schema()
            .expect("Arrow schema")
            .custom_metadata()
            .expect("Arrow metadata")
            .iter()
            .map(|entry| {
                (
                    entry.key().expect("key").to_owned(),
                    entry.value().expect("value").to_owned(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pairs,
            expected($arrow_encoding).into_iter().collect::<Vec<_>>()
        );

        let mut parquet = Vec::new();
        $parquet_writer(&mut parquet, table, budgets()).expect("Parquet matrix");
        let pairs = trusted_parquet_metadata(&parquet)
            .file_metadata()
            .key_value_metadata()
            .expect("Parquet metadata")
            .iter()
            .map(|entry| {
                (
                    entry.key.clone(),
                    entry.value.clone().expect("metadata value"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pairs,
            expected($parquet_encoding).into_iter().collect::<Vec<_>>()
        );
    }};
}

#[test]
fn all_matrix_application_metadata_is_exact_and_ordered_in_both_formats() {
    let fixture = matrix_fixture();
    assert_metadata!(
        &fixture.patch_table,
        "patch",
        "marklab.patch_embedding_table",
        write_patch_embedding_table_arrow,
        write_patch_embedding_table_parquet,
        "marklab.arrow-ipc.patch-embedding-table.v1",
        "marklab.parquet.patch-embedding-table.v1"
    );
    assert_metadata!(
        &fixture.region_table,
        "region",
        "marklab.region_embedding_table",
        write_region_embedding_table_arrow,
        write_region_embedding_table_parquet,
        "marklab.arrow-ipc.region-embedding-table.v1",
        "marklab.parquet.region-embedding-table.v1"
    );
    assert_metadata!(
        &fixture.slide_table,
        "slide",
        "marklab.slide_embedding_table",
        write_slide_embedding_table_arrow,
        write_slide_embedding_table_parquet,
        "marklab.arrow-ipc.slide-embedding-table.v1",
        "marklab.parquet.slide-embedding-table.v1"
    );
}

#[test]
fn matrix_arrow_preflight_rejects_profile_rows_buffers_fillers_and_padding() {
    let fixture = matrix_fixture();
    let table = &fixture.patch_table;
    let mut canonical = Vec::new();
    write_patch_embedding_table_arrow(&mut canonical, table, budgets()).expect("Arrow matrix");

    let mut bad_magic = canonical.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&bad_magic, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidMagic,
        })
    ));

    let mut wrong_id = canonical.clone();
    let ids = arrow_buffer_range(&wrong_id, 2);
    wrong_id[ids.start + "patch-".len()] = b'z';
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&wrong_id, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidCanonicalRows,
        })
    ));

    let mut wrong_status = canonical.clone();
    let statuses = arrow_buffer_range(&wrong_status, 8);
    wrong_status[statuses.start] = b'x';
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&wrong_status, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidCanonicalRows,
        })
    ));

    for (component, bits) in [
        (0_usize, f32::NAN.to_bits()),
        (1, (-0.0_f32).to_bits()),
        (3, 1.0_f32.to_bits()),
    ] {
        let mut wrong_component = canonical.clone();
        let values = arrow_buffer_range(&wrong_component, 5);
        let at = values.start + component * size_of::<f32>();
        wrong_component[at..at + 4].copy_from_slice(&bits.to_le_bytes());
        assert!(matches!(
            preflight_patch_embedding_table_arrow_bytes(&wrong_component, table, budgets()),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidCanonicalRows,
            })
        ));
    }

    for validity_index in [0, 3, 4, 6] {
        let mut wrong_validity = canonical.clone();
        let validity = arrow_buffer_range(&wrong_validity, validity_index);
        wrong_validity[validity.start] = 0xfe;
        assert!(matches!(
            preflight_patch_embedding_table_arrow_bytes(&wrong_validity, table, budgets()),
            Err(MultiscaleColumnarError::Arrow {
                reason: SpatialArrowFailure::InvalidBuffers,
            })
        ));
    }

    let mut wrong_padding = canonical.clone();
    let validity = arrow_buffer_range(&wrong_padding, 0);
    let offsets = arrow_buffer_range(&wrong_padding, 1);
    assert!(validity.end < offsets.start);
    wrong_padding[validity.end] = 1;
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&wrong_padding, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBuffers,
        })
    ));

    let mut wrong_message_padding = canonical.clone();
    let (message_start, message_length) = first_arrow_block(&wrong_message_padding);
    let padding_at = message_start + message_length - 1;
    assert_eq!(wrong_message_padding[padding_at], 0);
    wrong_message_padding[padding_at] = 1;
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&wrong_message_padding, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidMessage,
        })
    ));

    let mut wrong_schema_padding = canonical.clone();
    let schema_start = 64_usize;
    let schema_declared = usize::try_from(i32::from_le_bytes(
        wrong_schema_padding[schema_start + 4..schema_start + 8]
            .try_into()
            .expect("schema message length"),
    ))
    .expect("positive schema message length");
    let schema_padding_at = schema_start + 8 + schema_declared - 1;
    assert_eq!(wrong_schema_padding[schema_padding_at], 0);
    wrong_schema_padding[schema_padding_at] = 1;
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&wrong_schema_padding, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidMessage,
        })
    ));

    let mut wrong_schema = canonical.clone();
    assert!(replace_all_same_length(&mut wrong_schema, b"item", b"xxxx") >= 2);
    assert!(matches!(
        preflight_patch_embedding_table_arrow_bytes(&wrong_schema, table, budgets()),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidSchema,
        })
    ));

    assert!(preflight_region_embedding_table_arrow_bytes(
        &canonical,
        &fixture.region_table,
        budgets()
    )
    .is_err());
}

#[test]
fn matrix_parquet_preflight_rejects_profile_rows_lists_and_forbidden_features() {
    let fixture = matrix_fixture();
    let table = &fixture.patch_table;
    let mut canonical = Vec::new();
    write_patch_embedding_table_parquet(&mut canonical, table, budgets()).expect("Parquet matrix");

    let mut bad_magic = canonical.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_patch_embedding_table_parquet_bytes(&bad_magic, table, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMagic,
        })
    ));

    for (from, to) in [
        (b"patch-a".as_slice(), b"patch-z".as_slice()),
        (b"present".as_slice(), b"xresent".as_slice()),
    ] {
        let mut wrong_row = canonical.clone();
        assert_eq!(replace_all_same_length(&mut wrong_row, from, to), 1);
        assert!(matches!(
            preflight_patch_embedding_table_parquet_bytes(&wrong_row, table, budgets()),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));
    }

    let component_start = parquet_column_start(&canonical, 1);
    let (_, body, definitions, repetitions) = page_v2_layout(&canonical, component_start);
    let values_start = body + definitions + repetitions;
    for (component, bits) in [
        (0_usize, f32::NAN.to_bits()),
        (1, (-0.0_f32).to_bits()),
        (3, 1.0_f32.to_bits()),
    ] {
        let mut wrong_component = canonical.clone();
        let at = values_start + component * size_of::<f32>();
        wrong_component[at..at + 4].copy_from_slice(&bits.to_le_bytes());
        assert!(matches!(
            preflight_patch_embedding_table_parquet_bytes(&wrong_component, table, budgets()),
            Err(MultiscaleColumnarError::Parquet {
                reason: SpatialParquetFailure::InvalidCanonicalRows,
            })
        ));
    }

    assert!(repetitions > 0);
    let mut wrong_levels = canonical.clone();
    wrong_levels[body] ^= 1;
    assert!(matches!(
        preflight_patch_embedding_table_parquet_bytes(&wrong_levels, table, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidPage,
        })
    ));

    let wrong_schema = rewrite_parquet_footer(&canonical, |metadata| {
        metadata.schema[0].name = "marklab_patch_embedding_tablx".to_owned();
    });
    assert!(matches!(
        preflight_patch_embedding_table_parquet_bytes(&wrong_schema, table, budgets()),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidSchema,
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
            metadata.column_orders = None;
        }),
        rewrite_parquet_footer(&canonical, |metadata| {
            metadata.row_groups[0].columns[0]
                .meta_data
                .as_mut()
                .expect("column metadata")
                .encoding_stats = None;
        }),
    ];
    for (index, bytes) in forbidden.into_iter().enumerate() {
        let observed = preflight_patch_embedding_table_parquet_bytes(&bytes, table, budgets());
        assert!(observed.is_err(), "forbidden case {index} passed");
    }

    assert!(preflight_slide_embedding_table_parquet_bytes(
        &canonical,
        &fixture.slide_table,
        budgets()
    )
    .is_err());
    assert!(preflight_patch_embedding_table_parquet_bytes(
        &canonical[..canonical.len() - 1],
        table,
        budgets()
    )
    .is_err());
}
