use parquet::format::{
    ConvertedType, FieldRepetitionType, FileMetaData, LogicalType, SchemaElement, Type,
};

use crate::columnar::{CellEmbeddingTablePhysicalBindings, EmbeddingColumnarError, ParquetFailure};

use super::super::profile::{
    metadata_values, MAXIMUM_APPLICATION_METADATA_BYTES, METADATA_KEYS, ROOT_NAME,
};
use super::parquet_failure;

pub(super) fn validate_schema(schema: &[SchemaElement]) -> Result<(), EmbeddingColumnarError> {
    if schema.len() != 6
        || !is_group(&schema[0], ROOT_NAME, None, 3, None, None)
        || !is_utf8(&schema[1], "cell_id")
        || !is_group(
            &schema[2],
            "embedding",
            Some(FieldRepetitionType::REQUIRED),
            1,
            Some(ConvertedType::LIST),
            Some("list"),
        )
        || !is_group(
            &schema[3],
            "list",
            Some(FieldRepetitionType::REPEATED),
            1,
            None,
            None,
        )
        || schema[4].type_ != Some(Type::FLOAT)
        || schema[4].type_length.is_some()
        || schema[4].repetition_type != Some(FieldRepetitionType::REQUIRED)
        || schema[4].name != "element"
        || schema[4].num_children.is_some()
        || schema[4].converted_type.is_some()
        || schema[4].scale.is_some()
        || schema[4].precision.is_some()
        || schema[4].field_id.is_some()
        || schema[4].logical_type.is_some()
        || !is_utf8(&schema[5], "embedding_status")
    {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
}

fn is_group(
    element: &SchemaElement,
    name: &str,
    repetition: Option<FieldRepetitionType>,
    children: i32,
    converted: Option<ConvertedType>,
    logical: Option<&str>,
) -> bool {
    element.type_.is_none()
        && element.type_length.is_none()
        && element.repetition_type == repetition
        && element.name == name
        && element.num_children == Some(children)
        && element.converted_type == converted
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && match logical {
            Some("list") => matches!(element.logical_type, Some(LogicalType::LIST(_))),
            None => element.logical_type.is_none(),
            _ => false,
        }
}

fn is_utf8(element: &SchemaElement, name: &str) -> bool {
    element.type_ == Some(Type::BYTE_ARRAY)
        && element.type_length.is_none()
        && element.repetition_type == Some(FieldRepetitionType::REQUIRED)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UTF8)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(element.logical_type, Some(LogicalType::STRING(_)))
}

pub(super) fn validate_metadata(
    metadata: &FileMetaData,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<(), EmbeddingColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    if entries.len() != METADATA_KEYS.len() {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let values = metadata_values(bindings);
    let mut total_bytes = 0_usize;
    for ((entry, key), value) in entries.iter().zip(METADATA_KEYS).zip(values) {
        let observed_value = entry
            .value
            .as_deref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
        total_bytes = total_bytes
            .checked_add(entry.key.len())
            .and_then(|total| total.checked_add(observed_value.len()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if entry.key != key || observed_value != value {
            return Err(parquet_failure(ParquetFailure::InvalidMetadata));
        }
    }
    if total_bytes > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    Ok(())
}
