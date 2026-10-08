use std::mem::size_of;

use marklab_embeddings::CellEmbeddingTable;

use crate::common::finite::canonical_zero;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarEmbeddingCrossCovarianceError {
    RowUnavailable { row: usize },
    SizeOverflow,
    AllocationFailed { requested: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScalarEmbeddingCrossCovarianceRequirements {
    pub(crate) component_operations: u64,
    pub(crate) working_bytes: usize,
    pub(crate) dimension: usize,
}

pub(crate) fn requirements(
    present_rows: u64,
    dimension: u32,
) -> Result<ScalarEmbeddingCrossCovarianceRequirements, ScalarEmbeddingCrossCovarianceError> {
    let dimension_u64 = u64::from(dimension);
    let component_operations = present_rows
        .checked_mul(dimension_u64)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| {
            dimension_u64
                .checked_mul(2)
                .and_then(|tail| value.checked_add(tail))
        })
        .ok_or(ScalarEmbeddingCrossCovarianceError::SizeOverflow)?;
    let dimension = usize::try_from(dimension)
        .map_err(|_| ScalarEmbeddingCrossCovarianceError::SizeOverflow)?;
    let working_bytes = dimension
        .checked_mul(size_of::<f64>())
        .and_then(|value| value.checked_mul(2))
        .ok_or(ScalarEmbeddingCrossCovarianceError::SizeOverflow)?;
    Ok(ScalarEmbeddingCrossCovarianceRequirements {
        component_operations,
        working_bytes,
        dimension,
    })
}

pub(crate) fn energy(
    table: &CellEmbeddingTable,
    scalar_values: &[f32],
    present_rows: u64,
    scalar_mean: f64,
    requirements: ScalarEmbeddingCrossCovarianceRequirements,
) -> Result<f64, ScalarEmbeddingCrossCovarianceError> {
    let denominator = present_rows as f64;
    let mut embedding_means =
        zeroed_accumulator(requirements.dimension, requirements.working_bytes)?;
    let mut covariance_sums =
        zeroed_accumulator(requirements.dimension, requirements.working_bytes)?;
    for row_index in 0..table.row_count() {
        let embedding_row = table
            .row(row_index)
            .map_err(|_| ScalarEmbeddingCrossCovarianceError::RowUnavailable { row: row_index })?;
        let Some(vector) = embedding_row.vector() else {
            continue;
        };
        for (sum, &component) in embedding_means.iter_mut().zip(vector) {
            *sum += f64::from(component);
        }
    }
    for mean in &mut embedding_means {
        *mean /= denominator;
    }
    for row_index in 0..table.row_count() {
        let scalar = *scalar_values
            .get(row_index)
            .ok_or(ScalarEmbeddingCrossCovarianceError::RowUnavailable { row: row_index })?;
        let embedding_row = table
            .row(row_index)
            .map_err(|_| ScalarEmbeddingCrossCovarianceError::RowUnavailable { row: row_index })?;
        let Some(vector) = embedding_row.vector() else {
            continue;
        };
        let centered_scalar = f64::from(scalar) - scalar_mean;
        for ((sum, &component), &embedding_mean) in
            covariance_sums.iter_mut().zip(vector).zip(&embedding_means)
        {
            *sum += centered_scalar * (f64::from(component) - embedding_mean);
        }
    }
    let mut result = 0.0_f64;
    for covariance_sum in covariance_sums {
        let covariance = covariance_sum / denominator;
        result += covariance * covariance;
    }
    Ok(canonical_zero(result / requirements.dimension as f64))
}

fn zeroed_accumulator(
    dimension: usize,
    requested: usize,
) -> Result<Vec<f64>, ScalarEmbeddingCrossCovarianceError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(dimension)
        .map_err(|_| ScalarEmbeddingCrossCovarianceError::AllocationFailed { requested })?;
    values.resize(dimension, 0.0);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_preserve_exact_work_and_storage_formulas() {
        assert_eq!(
            requirements(3, 4),
            Ok(ScalarEmbeddingCrossCovarianceRequirements {
                component_operations: 32,
                working_bytes: 64,
                dimension: 4,
            })
        );
        assert_eq!(
            requirements(u64::MAX, u32::MAX),
            Err(ScalarEmbeddingCrossCovarianceError::SizeOverflow)
        );
        assert_eq!(
            zeroed_accumulator(usize::MAX, usize::MAX),
            Err(ScalarEmbeddingCrossCovarianceError::AllocationFailed {
                requested: usize::MAX
            })
        );
    }
}
