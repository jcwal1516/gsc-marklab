use crate::columnar::{
    parquet::{
        profile::{MAXIMUM_FOOTER_BYTES, ROW_GROUP_ROWS},
        writer::{MAXIMUM_ENCODED_FOOTER_BASE_BYTES, MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP},
    },
    ColumnarError,
};

pub(super) fn enforce_footer_bound(row_count: usize) -> Result<(), ColumnarError> {
    let footer_bound = MAXIMUM_ENCODED_FOOTER_BASE_BYTES
        .checked_add(
            row_count
                .div_ceil(ROW_GROUP_ROWS)
                .checked_mul(MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP)
                .ok_or(ColumnarError::SizeOverflow)?,
        )
        .ok_or(ColumnarError::SizeOverflow)?;
    if footer_bound > MAXIMUM_FOOTER_BYTES {
        return Err(ColumnarError::ParquetWriter);
    }
    Ok(())
}
