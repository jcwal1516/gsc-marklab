use std::sync::Arc;

use ::arrow::datatypes::{DataType, Field, Schema};
use parquet::{
    basic::{Compression, Encoding},
    file::properties::{EnabledStatistics, WriterProperties, WriterVersion},
    format::KeyValue,
};

use super::super::{CellEmbeddingTablePhysicalBindings, EmbeddingColumnarError};

pub(super) const PARQUET_MAGIC: &[u8; 4] = b"PAR1";
pub(super) const TRAILER_BYTES: usize = 8;
pub(super) const MAXIMUM_FOOTER_BYTES: usize = 1024 * 1024;
pub(super) const MAXIMUM_PAGE_HEADER_BYTES: usize = 64 * 1024;
pub(super) const MAXIMUM_PAGE_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAXIMUM_APPLICATION_METADATA_BYTES: usize = 64 * 1024;
pub(super) const MAXIMUM_ROWS: usize = 100_000_000;
pub(super) const MAXIMUM_DIMENSION: u32 = 65_536;
pub(super) const ROW_GROUP_ROWS: usize = 8_192;
pub(super) const PUBLIC_BATCH_ROWS: usize = 8_192;
const PARQUET_WRITE_BATCH_SIZE: usize = 1_024;
const PAGE_ROWS: usize = 1_024;
const PAGE_BYTES: usize = 1024 * 1024;

pub(super) const SCHEMA_ID: &str = "marklab.cell_embedding_table";
const SCHEMA_VERSION: &str = "1";
pub(super) const ENCODING_VERSION: &str = "marklab.parquet.embedding-table.v1";
pub(super) const CONTENT_KIND: &str = "application/vnd.marklab.cell-embedding-table.v1+parquet";
pub(super) const ROOT_NAME: &str = "marklab_cell_embedding_table";
pub(super) const CREATED_BY: &str = "marklab-embeddings/0.1.0 parquet-56.2.1 profile-v1";

pub(super) const METADATA_KEYS: [&str; 7] = [
    "marklab.encoding_version",
    "marklab.expected_cells_artifact_id",
    "marklab.logical_digest",
    "marklab.provenance_artifact_id",
    "marklab.row_link_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

pub(super) fn embedding_schema(dimension: u32) -> Result<Schema, EmbeddingColumnarError> {
    let length = i32::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    Ok(Schema::new(vec![
        Field::new("cell_id", DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("element", DataType::Float32, false)),
                length,
            ),
            false,
        ),
        Field::new("embedding_status", DataType::Utf8, false),
    ]))
}

pub(super) fn metadata_values(bindings: CellEmbeddingTablePhysicalBindings) -> [String; 7] {
    [
        ENCODING_VERSION.to_owned(),
        bindings.expected_cells_artifact_id().to_string(),
        bindings.table_logical_digest().to_string(),
        bindings.provenance_artifact_id().to_string(),
        bindings.row_link_artifact_id().to_string(),
        SCHEMA_ID.to_owned(),
        SCHEMA_VERSION.to_owned(),
    ]
}

pub(super) fn writer_properties(bindings: CellEmbeddingTablePhysicalBindings) -> WriterProperties {
    let metadata = METADATA_KEYS
        .iter()
        .zip(metadata_values(bindings))
        .map(|(key, value)| KeyValue::new((*key).to_owned(), Some(value)))
        .collect();
    writer_properties_with_metadata(metadata)
}

pub(super) fn writer_properties_with_metadata(metadata: Vec<KeyValue>) -> WriterProperties {
    WriterProperties::builder()
        .set_writer_version(WriterVersion::PARQUET_2_0)
        .set_data_page_size_limit(PAGE_BYTES)
        .set_data_page_row_count_limit(PAGE_ROWS)
        .set_write_batch_size(PARQUET_WRITE_BATCH_SIZE)
        .set_max_row_group_size(ROW_GROUP_ROWS)
        .set_created_by(CREATED_BY.to_owned())
        .set_offset_index_disabled(true)
        .set_key_value_metadata(Some(metadata))
        .set_column_index_truncate_length(None)
        .set_statistics_truncate_length(None)
        .set_coerce_types(false)
        .set_encoding(Encoding::PLAIN)
        .set_compression(Compression::UNCOMPRESSED)
        .set_dictionary_enabled(false)
        .set_statistics_enabled(EnabledStatistics::None)
        .set_write_page_header_statistics(false)
        .set_bloom_filter_enabled(false)
        .build()
}

pub(super) fn embedding_decoded_bytes(
    row_count: usize,
    dimension: u32,
    identifier_bytes: usize,
    status_bytes: usize,
) -> Result<u64, EmbeddingColumnarError> {
    let dimension = usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    let mut required = 0_usize;
    let mut start = 0_usize;
    while start < row_count {
        let rows = row_count
            .checked_sub(start)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?
            .min(PUBLIC_BATCH_ROWS);
        let components = rows
            .checked_mul(dimension)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let offsets = rows
            .checked_add(1)
            .and_then(|value| value.checked_mul(size_of::<i32>()))
            .and_then(|value| value.checked_mul(2))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let validity = bitmap_bytes(rows)?
            .checked_mul(3)
            .and_then(|value| value.checked_add(bitmap_bytes(components).ok()?))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        required = required
            .checked_add(
                components
                    .checked_mul(size_of::<f32>())
                    .and_then(|value| value.checked_add(offsets))
                    .and_then(|value| value.checked_add(validity))
                    .ok_or(EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        start = start
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    required = required
        .checked_add(identifier_bytes)
        .and_then(|value| value.checked_add(status_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    u64::try_from(required).map_err(|_| EmbeddingColumnarError::SizeOverflow)
}

fn bitmap_bytes(values: usize) -> Result<usize, EmbeddingColumnarError> {
    values
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}
