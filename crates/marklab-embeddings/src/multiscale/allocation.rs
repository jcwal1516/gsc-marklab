use std::mem::size_of;

use super::MultiscaleEmbeddingError;

pub(super) fn try_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, MultiscaleEmbeddingError> {
    let requested = capacity
        .checked_mul(size_of::<T>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| MultiscaleEmbeddingError::AllocationFailed { requested })?;
    Ok(values)
}
