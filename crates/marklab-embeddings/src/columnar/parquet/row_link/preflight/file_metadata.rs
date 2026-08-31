use super::{
    schema::{validate_metadata, validate_schema},
    *,
};

pub(super) fn validate_metadata_tree<F>(
    footer_start: usize,
    metadata: &FileMetaData,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
    mut page_validator: F,
) -> Result<(), EmbeddingColumnarError>
where
    F: FnMut(
        usize,
        usize,
        usize,
        usize,
        usize,
        &CellEmbeddingRowLink,
    ) -> Result<usize, EmbeddingColumnarError>,
{
    let expected_rows = row_link.entries().len();
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema)?;
    validate_metadata(metadata, row_link)?;
    if metadata.created_by.as_deref() != Some(CREATED_BY)
        || metadata.encryption_algorithm.is_some()
        || metadata.footer_signing_key_metadata.is_some()
        || metadata.column_orders.as_ref().is_none_or(|orders| {
            orders.len() != 3
                || orders
                    .iter()
                    .any(|order| !matches!(order, ColumnOrder::TYPEORDER(_)))
        })
    {
        return Err(parquet_failure(ParquetFailure::InvalidMetadata));
    }
    let mut aggregate_rows = 0_usize;
    let mut next_offset = PARQUET_MAGIC.len();
    for (group_index, group) in metadata.row_groups.iter().enumerate() {
        let rows = expected_rows
            .checked_sub(aggregate_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?
            .min(ROW_GROUP_ROWS);
        if usize::try_from(group.num_rows).ok() != Some(rows)
            || group.columns.len() != 3
            || group.sorting_columns.is_some()
            || group.ordinal != i16::try_from(group_index).ok()
            || group
                .file_offset
                .and_then(|value| usize::try_from(value).ok())
                != Some(next_offset)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let paths: [&[&str]; 3] = [
            &["cell_id"],
            &["source_cell_row"],
            &["source_embedding_row"],
        ];
        let types = [Type::BYTE_ARRAY, Type::INT64, Type::INT64];
        let mut group_bytes = 0_usize;
        for (column_index, column) in group.columns.iter().enumerate() {
            if column.file_path.is_some()
                || column.file_offset != 0
                || column.offset_index_offset.is_some()
                || column.offset_index_length.is_some()
                || column.column_index_offset.is_some()
                || column.column_index_length.is_some()
                || column.crypto_metadata.is_some()
                || column.encrypted_column_metadata.is_some()
            {
                return Err(parquet_failure(ParquetFailure::ForbiddenAuxiliaryData));
            }
            let column_metadata = column
                .meta_data
                .as_ref()
                .ok_or_else(|| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            if column_metadata.codec != CompressionCodec::UNCOMPRESSED {
                return Err(parquet_failure(ParquetFailure::UnsupportedCompression));
            }
            if column_metadata.encodings != [Encoding::PLAIN, Encoding::RLE] {
                return Err(parquet_failure(ParquetFailure::UnsupportedEncoding));
            }
            if column_metadata.key_value_metadata.is_some()
                || column_metadata.index_page_offset.is_some()
                || column_metadata.dictionary_page_offset.is_some()
                || column_metadata.statistics.is_some()
                || column_metadata.bloom_filter_offset.is_some()
                || column_metadata.bloom_filter_length.is_some()
                || column_metadata.size_statistics.is_some()
                || column_metadata.geospatial_statistics.is_some()
            {
                return Err(parquet_failure(ParquetFailure::ForbiddenAuxiliaryData));
            }
            if column_metadata.type_ != types[column_index]
                || column_metadata
                    .path_in_schema
                    .iter()
                    .map(String::as_str)
                    .ne(paths[column_index].iter().copied())
                || usize::try_from(column_metadata.num_values).ok() != Some(rows)
                || column_metadata.total_compressed_size <= 0
                || column_metadata.total_compressed_size != column_metadata.total_uncompressed_size
                || column_metadata
                    .encoding_stats
                    .as_ref()
                    .is_none_or(|statistics| {
                        statistics.len() != 1
                            || statistics[0].page_type != PageType::DATA_PAGE_V2
                            || statistics[0].encoding != Encoding::PLAIN
                            || statistics[0].count <= 0
                    })
            {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            let start = usize::try_from(column_metadata.data_page_offset)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let length = usize::try_from(column_metadata.total_compressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            if start != next_offset {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            next_offset = start
                .checked_add(length)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if next_offset > footer_start {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            let pages = page_validator(
                start,
                next_offset,
                column_index,
                aggregate_rows,
                rows,
                row_link,
            )?;
            if column_metadata
                .encoding_stats
                .as_ref()
                .is_none_or(|statistics| i32::try_from(pages).ok() != Some(statistics[0].count))
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            group_bytes = group_bytes
                .checked_add(length)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
        if usize::try_from(group.total_byte_size).ok() != Some(group_bytes)
            || group
                .total_compressed_size
                .and_then(|value| usize::try_from(value).ok())
                != Some(group_bytes)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        enforce_row_group_budget(group_bytes, budgets)?;
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    if aggregate_rows != expected.cells().len() || next_offset != footer_start {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(())
}
