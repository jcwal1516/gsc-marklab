use std::mem::size_of;

use crate::{EmbeddingStatus, ExpectedCellSet};

use super::ColumnarError;

pub(crate) fn estimate_materialized_table_bytes(
    expected: &ExpectedCellSet,
    dimension: u32,
) -> Result<usize, ColumnarError> {
    let component_bytes = expected
        .cells()
        .len()
        .checked_mul(usize::try_from(dimension).map_err(|_| ColumnarError::SizeOverflow)?)
        .and_then(|value| value.checked_mul(size_of::<f32>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let row_bytes = expected
        .cells()
        .len()
        .checked_mul(size_of::<marklab_data::CellId>() + size_of::<EmbeddingStatus>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.as_str().len())
            .ok_or(ColumnarError::SizeOverflow)
    })?;
    component_bytes
        .checked_add(row_bytes)
        .and_then(|value| value.checked_add(identifier_bytes))
        .ok_or(ColumnarError::SizeOverflow)
}
