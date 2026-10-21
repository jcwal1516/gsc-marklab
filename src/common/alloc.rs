use std::collections::TryReserveError;

/// Allocate `length` copies of `value`, reporting allocation failure instead of aborting.
pub(crate) fn try_filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, TryReserveError> {
    let mut values = Vec::new();
    values.try_reserve_exact(length)?;
    values.resize(length, value);
    Ok(values)
}
