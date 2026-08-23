use std::collections::HashMap;

use arrow::datatypes::{DataType, Field, Schema};
use arrow_ipc::Type;

use crate::CellEmbeddingRowLink;

use super::super::{
    super::super::{ArrowIpcFailure, EmbeddingColumnarError},
    profile::{arrow_failure, validate_utf8_field},
};

pub(super) const SCHEMA_ID: &str = "marklab.cell_embedding_row_link";
const SCHEMA_VERSION: &str = "1";
pub(super) const ENCODING_VERSION: &str = "marklab.arrow-ipc.embedding-row-link.v1";
pub(super) const CONTENT_KIND: &str = "application/vnd.marklab.embedding-row-link.v1+arrow";

pub(super) const METADATA_KEYS: [&str; 9] = [
    "marklab.converter_artifact_id",
    "marklab.encoding_version",
    "marklab.expected_cells_artifact_id",
    "marklab.identity_map_artifact_id",
    "marklab.row_link_digest",
    "marklab.schema_id",
    "marklab.schema_version",
    "marklab.source_cells_artifact_id",
    "marklab.source_vectors_artifact_id",
];

pub(super) fn row_link_schema(
    row_link: &CellEmbeddingRowLink,
) -> Result<Schema, EmbeddingColumnarError> {
    let values = metadata_values(row_link);
    let metadata = METADATA_KEYS
        .iter()
        .zip(values)
        .map(|(key, value)| ((*key).to_owned(), value))
        .collect::<HashMap<_, _>>();
    Ok(Schema::new_with_metadata(
        vec![
            Field::new("cell_id", DataType::Utf8, false),
            Field::new("source_cell_row", DataType::UInt64, false),
            Field::new("source_embedding_row", DataType::UInt64, true),
        ],
        metadata,
    ))
}

pub(super) fn validate_flatbuffer_schema(
    schema: arrow_ipc::Schema<'_>,
    row_link: &CellEmbeddingRowLink,
) -> Result<(), EmbeddingColumnarError> {
    if schema.endianness() != arrow_ipc::Endianness::Little
        || schema
            .features()
            .is_some_and(|features| !features.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_metadata(schema.custom_metadata(), row_link)?;
    let fields = schema
        .fields()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    if fields.len() != 3 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_utf8_field(fields.get(0), "cell_id")?;
    validate_u64_field(fields.get(1), "source_cell_row", false)?;
    validate_u64_field(fields.get(2), "source_embedding_row", true)?;
    Ok(())
}

fn validate_u64_field(
    field: arrow_ipc::Field<'_>,
    name: &str,
    nullable: bool,
) -> Result<(), EmbeddingColumnarError> {
    let integer = field.type_as_int();
    if field.name() != Some(name)
        || field.nullable() != nullable
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Int
        || integer.is_none_or(|integer| integer.bitWidth() != 64 || integer.is_signed())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(())
}

fn validate_metadata(
    metadata: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<arrow_ipc::KeyValue<'_>>>,
    >,
    row_link: &CellEmbeddingRowLink,
) -> Result<(), EmbeddingColumnarError> {
    let metadata =
        metadata.ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
    if metadata.len() != METADATA_KEYS.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    let values = metadata_values(row_link);
    let mut total_bytes = 0_usize;
    for ((entry, key), value) in metadata.iter().zip(METADATA_KEYS).zip(values.iter()) {
        let observed_key = entry
            .key()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
        let observed_value = entry
            .value()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
        total_bytes = total_bytes
            .checked_add(observed_key.len())
            .and_then(|total| total.checked_add(observed_value.len()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if observed_key != key || observed_value != value {
            return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
        }
    }
    if total_bytes > 64 * 1024 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    Ok(())
}

fn metadata_values(row_link: &CellEmbeddingRowLink) -> [String; 9] {
    [
        row_link.converter_artifact_id().to_string(),
        ENCODING_VERSION.to_owned(),
        row_link.expected_cells_artifact_id().to_string(),
        row_link.identity_map_artifact_id().to_string(),
        row_link.logical_digest().to_string(),
        SCHEMA_ID.to_owned(),
        SCHEMA_VERSION.to_owned(),
        row_link.source_cells_artifact_id().to_string(),
        row_link.source_vectors_artifact_id().to_string(),
    ]
}

pub(super) fn validate_nullable_bitmap(
    bytes: &[u8],
    row_link: &CellEmbeddingRowLink,
    start: usize,
    rows: usize,
) -> Result<(), EmbeddingColumnarError> {
    let end = start
        .checked_add(rows)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let entries = row_link
        .entries()
        .get(start..end)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
    for (byte_index, observed) in bytes.iter().enumerate() {
        let mut expected = 0_u8;
        for bit in 0..8 {
            let local_row = byte_index
                .checked_mul(8)
                .and_then(|value| value.checked_add(bit))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if local_row < rows
                && entries
                    .get(local_row)
                    .is_some_and(|entry| entry.source_embedding_row().is_some())
            {
                expected |= 1 << bit;
            }
        }
        if *observed != expected {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
    }
    Ok(())
}
