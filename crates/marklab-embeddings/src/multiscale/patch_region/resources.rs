use std::mem::size_of;

use super::{types::PatchRegionDeclaration, PatchRegionAssessment, PatchRegionLink};
use crate::multiscale::error::MultiscaleEmbeddingError;

pub(super) use crate::multiscale::allocation::try_vec_capacity;

pub(super) const MAX_PATCH_REGION_ROWS: usize = 400_000_000;

pub(super) fn input_bytes(
    rows: &Vec<PatchRegionDeclaration>,
) -> Result<usize, MultiscaleEmbeddingError> {
    rows.iter().try_fold(
        rows.capacity()
            .checked_mul(size_of::<PatchRegionDeclaration>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, row| {
            total
                .checked_add(row.patch_id.as_str().len())
                .and_then(|value| value.checked_add(row.region_id.as_str().len()))
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )
}

pub(super) fn assessment_retained_bytes(
    owning_slide_text_bytes: usize,
    rows: &[PatchRegionDeclaration],
) -> Result<usize, MultiscaleEmbeddingError> {
    retained_bytes::<PatchRegionAssessment>(owning_slide_text_bytes, rows)
}

pub(super) fn link_retained_bytes(
    owning_slide_text_bytes: usize,
    rows: &[PatchRegionDeclaration],
) -> Result<usize, MultiscaleEmbeddingError> {
    retained_bytes::<PatchRegionLink>(owning_slide_text_bytes, rows)
}

fn retained_bytes<T>(
    owning_slide_text_bytes: usize,
    rows: &[PatchRegionDeclaration],
) -> Result<usize, MultiscaleEmbeddingError> {
    let row_storage = rows
        .len()
        .checked_mul(size_of::<PatchRegionDeclaration>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    rows.iter().try_fold(
        size_of::<T>()
            .checked_add(owning_slide_text_bytes)
            .and_then(|value| value.checked_add(row_storage))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, row| {
            total
                .checked_add(row.patch_id.as_str().len())
                .and_then(|value| value.checked_add(row.region_id.as_str().len()))
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )
}

pub(super) fn enforce_retained(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

pub(super) fn enforce_working(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum });
    }
    Ok(())
}
