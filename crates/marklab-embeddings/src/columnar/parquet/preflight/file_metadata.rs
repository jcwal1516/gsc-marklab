use parquet::format::{ColumnOrder, CompressionCodec, Encoding, FileMetaData, PageType, Type};

use crate::{
    columnar::{
        enforce_decoded_budget, enforce_row_group_budget, CellEmbeddingTablePhysicalBindings,
        EmbeddingColumnarBudgets, EmbeddingColumnarError, ParquetFailure,
    },
    ExpectedCellSet,
};

use super::super::profile::{embedding_decoded_bytes, CREATED_BY, ROW_GROUP_ROWS};
use super::{
    pages::ValidatedPages,
    parquet_failure,
    schema::{validate_metadata, validate_schema},
};

pub(super) fn validate_file_metadata<F>(
    metadata: &FileMetaData,
    footer_start: usize,
    expected: &ExpectedCellSet,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
    mut page_validator: F,
) -> Result<(), EmbeddingColumnarError>
where
    F: FnMut(usize, usize, usize, usize, u32) -> Result<ValidatedPages, EmbeddingColumnarError>,
{
    let expected_rows = expected.cells().len();
    if metadata.version != 2
        || usize::try_from(metadata.num_rows).ok() != Some(expected_rows)
        || metadata.row_groups.len() != expected_rows.div_ceil(ROW_GROUP_ROWS)
    {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    validate_schema(&metadata.schema)?;
    validate_metadata(metadata, bindings)?;
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
    let mut identifier_bytes = 0_usize;
    let mut status_bytes = 0_usize;
    let mut next_chunk_offset = super::super::profile::PARQUET_MAGIC.len();
    for (group_index, row_group) in metadata.row_groups.iter().enumerate() {
        let remaining = expected_rows
            .checked_sub(aggregate_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let rows = remaining.min(ROW_GROUP_ROWS);
        if usize::try_from(row_group.num_rows).ok() != Some(rows)
            || row_group.columns.len() != 3
            || row_group.sorting_columns.is_some()
            || row_group.ordinal != i16::try_from(group_index).ok()
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let group_start = row_group
            .file_offset
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowGroup))?;
        if group_start != next_chunk_offset {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        let expected_paths: [&[&str]; 3] = [
            &["cell_id"],
            &["embedding", "list", "element"],
            &["embedding_status"],
        ];
        let expected_types = [Type::BYTE_ARRAY, Type::FLOAT, Type::BYTE_ARRAY];
        let expected_values = [
            rows,
            rows.checked_mul(
                usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
            rows,
        ];
        let mut group_compressed_bytes = 0_usize;
        let mut group_uncompressed_bytes = 0_usize;
        for (column_index, column) in row_group.columns.iter().enumerate() {
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
            let expected_path = expected_paths[column_index];
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
            if column_metadata.type_ != expected_types[column_index]
                || column_metadata.path_in_schema.len() != expected_path.len()
                || column_metadata
                    .path_in_schema
                    .iter()
                    .map(String::as_str)
                    .ne(expected_path.iter().copied())
                || usize::try_from(column_metadata.num_values).ok()
                    != Some(expected_values[column_index])
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
            let data_offset = usize::try_from(column_metadata.data_page_offset)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let compressed_bytes = usize::try_from(column_metadata.total_compressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            let uncompressed_bytes = usize::try_from(column_metadata.total_uncompressed_size)
                .map_err(|_| parquet_failure(ParquetFailure::InvalidColumnChunk))?;
            if data_offset != next_chunk_offset {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            next_chunk_offset = data_offset
                .checked_add(compressed_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            if next_chunk_offset > footer_start {
                return Err(parquet_failure(ParquetFailure::InvalidColumnChunk));
            }
            let pages = page_validator(
                data_offset,
                next_chunk_offset,
                column_index,
                rows,
                dimension,
            )?;
            if column_metadata
                .encoding_stats
                .as_ref()
                .is_none_or(|statistics| {
                    i32::try_from(pages.page_count).ok() != Some(statistics[0].count)
                })
            {
                return Err(parquet_failure(ParquetFailure::InvalidPage));
            }
            match column_index {
                0 => {
                    identifier_bytes = identifier_bytes
                        .checked_add(pages.string_payload_bytes)
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                }
                2 => {
                    status_bytes = status_bytes
                        .checked_add(pages.string_payload_bytes)
                        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
                }
                _ => {}
            }
            group_compressed_bytes = group_compressed_bytes
                .checked_add(compressed_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            group_uncompressed_bytes = group_uncompressed_bytes
                .checked_add(uncompressed_bytes)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
        if usize::try_from(row_group.total_byte_size).ok() != Some(group_uncompressed_bytes)
            || row_group
                .total_compressed_size
                .and_then(|value| usize::try_from(value).ok())
                != Some(group_compressed_bytes)
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowGroup));
        }
        enforce_row_group_budget(group_compressed_bytes, budgets)?;
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    if aggregate_rows != expected_rows || next_chunk_offset != footer_start {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    enforce_decoded_budget(
        embedding_decoded_bytes(expected_rows, dimension, identifier_bytes, status_bytes)?,
        budgets,
    )
}
