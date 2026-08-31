use parquet::file::metadata::RowGroupMetaData;

use crate::{EmbeddingStatus, ExpectedCellSet};

use super::{super::preflight::PreparedCellEmbeddingParquet, record::parquet_failure};
use crate::columnar::{EmbeddingColumnarError, ParquetFailure};

pub(super) fn validated_group_range(
    row_group: &RowGroupMetaData,
) -> Result<(u64, usize), EmbeddingColumnarError> {
    if row_group.num_columns() != 3 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    let start = u64::try_from(row_group.column(0).data_page_offset())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut next = start;
    for column in row_group.columns() {
        let offset = u64::try_from(column.data_page_offset())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let length = u64::try_from(column.compressed_size())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if offset != next || length == 0 {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        next = next
            .checked_add(length)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok((
        start,
        usize::try_from(
            next.checked_sub(start)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
    ))
}

pub(super) fn estimate_maximum_group_peak(
    prepared: &PreparedCellEmbeddingParquet,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    prepared
        .metadata
        .row_groups()
        .iter()
        .try_fold(0_usize, |maximum, group| {
            estimate_group_peak(group, dimension).map(|required| maximum.max(required))
        })
}

pub(super) fn estimate_group_peak(
    row_group: &RowGroupMetaData,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    let (_, encoded_bytes) = validated_group_range(row_group)?;
    let rows = usize::try_from(row_group.num_rows())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let components = rows
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let arrow_output = components
        .checked_mul(size_of::<f32>())
        .and_then(|value| value.checked_add(encoded_bytes))
        .and_then(|value| {
            value.checked_add(
                rows.checked_add(1)?
                    .checked_mul(size_of::<i32>())?
                    .checked_mul(2)?,
            )
        })
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    encoded_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(arrow_output.checked_mul(2)?))
        .and_then(|value| value.checked_add(components.checked_mul(4)?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(super) fn estimate_materialized_table_bytes(
    expected: &ExpectedCellSet,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    let component_bytes = expected
        .cells()
        .len()
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .and_then(|value| value.checked_mul(size_of::<f32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let row_bytes = expected
        .cells()
        .len()
        .checked_mul(size_of::<marklab_data::CellId>() + size_of::<EmbeddingStatus>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    component_bytes
        .checked_add(row_bytes)
        .and_then(|value| value.checked_add(identifier_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}
