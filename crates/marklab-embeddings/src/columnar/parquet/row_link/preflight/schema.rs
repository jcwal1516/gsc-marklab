use super::*;

pub(super) fn validate_schema(schema: &[SchemaElement]) -> Result<(), EmbeddingColumnarError> {
    if schema.len() != 4
        || !is_group(&schema[0], ROOT_NAME, 3)
        || !is_utf8(&schema[1], "cell_id")
        || !is_u64(&schema[2], "source_cell_row", FieldRepetitionType::REQUIRED)
        || !is_u64(
            &schema[3],
            "source_embedding_row",
            FieldRepetitionType::OPTIONAL,
        )
    {
        return Err(parquet_failure(ParquetFailure::InvalidSchema));
    }
    Ok(())
}

pub(super) fn validate_metadata(
    metadata: &FileMetaData,
    row_link: &CellEmbeddingRowLink,
) -> Result<(), EmbeddingColumnarError> {
    let entries = metadata
        .key_value_metadata
        .as_ref()
        .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
    if entries.len() != METADATA_KEYS.len() {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let values = metadata_values(row_link);
    let mut total = 0_usize;
    for ((entry, key), value) in entries.iter().zip(METADATA_KEYS).zip(values) {
        let observed = entry
            .value
            .as_deref()
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidMetadata))?;
        total = total
            .checked_add(entry.key.len())
            .and_then(|value| value.checked_add(observed.len()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if entry.key != key || observed != value {
            return Err(parquet_failure(ParquetFailure::InvalidMetadata));
        }
    }
    if total > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    Ok(())
}
