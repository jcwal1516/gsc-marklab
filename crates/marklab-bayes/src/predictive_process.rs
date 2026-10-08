use std::collections::{BTreeSet, HashSet};

use serde::Serialize;
use thiserror::Error;

use crate::{linalg, matern_covariance::matern32_1d};

#[derive(Clone, Debug, Serialize)]
pub struct FieldCoordinate1D {
    pub id: String,
    pub x_um: f64,
}

#[derive(Clone, Debug)]
pub struct PredictiveProcessSpec {
    pub amplitude: f64,
    pub length_scale_um: f64,
    pub jitter: f64,
    pub diagonal_correction: bool,
    pub coordinates: Vec<FieldCoordinate1D>,
    pub knots: Vec<FieldCoordinate1D>,
}

#[derive(Clone, Debug)]
pub struct PredictiveProcessPlan {
    pub coordinates: Vec<FieldCoordinate1D>,
    pub knots: Vec<FieldCoordinate1D>,
    pub amplitude: f64,
    pub length_scale_um: f64,
    pub jitter: f64,
    pub diagonal_correction: bool,
    pub kmm_cholesky: Vec<f64>,
    pub knm: Vec<f64>,
    pub low_rank_diagonal: Vec<f64>,
    pub residual_variances: Vec<f64>,
}

#[derive(Debug, Error)]
pub enum PredictiveProcessError {
    #[error("invalid predictive-process input: {0}")]
    InvalidInput(String),
    #[error("predictive-process numerical failure: {0}")]
    Numerical(String),
}

pub fn low_rank_predictive_process(
    mut spec: PredictiveProcessSpec,
) -> Result<PredictiveProcessPlan, PredictiveProcessError> {
    for (value, name) in [
        (spec.amplitude, "amplitude"),
        (spec.length_scale_um, "length scale"),
        (spec.jitter, "jitter"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(PredictiveProcessError::InvalidInput(format!(
                "{name} must be finite and positive"
            )));
        }
    }
    if !(2..=2_000).contains(&spec.coordinates.len()) || !(2..=128).contains(&spec.knots.len()) {
        return Err(PredictiveProcessError::InvalidInput(
            "requires 2-2000 coordinates and 2-128 knots".into(),
        ));
    }
    validate_and_sort(&mut spec.coordinates, "coordinate")?;
    validate_and_sort(&mut spec.knots, "knot")?;
    let n = spec.coordinates.len() as u128;
    let m = spec.knots.len() as u128;
    let elements = n * m + m * m;
    let work = n * m * m + m * m * m;
    if elements > 10_000_000 || work > 500_000_000 {
        return Err(PredictiveProcessError::InvalidInput(
            "predictive-process memory or work limit exceeded".into(),
        ));
    }

    let dimension = spec.knots.len();
    let mut kmm = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            kmm[row * dimension + column] = matern32_1d(
                spec.knots[row].x_um,
                spec.knots[column].x_um,
                spec.amplitude,
                spec.length_scale_um,
            );
        }
        kmm[row * dimension + row] += spec.jitter;
    }
    let kmm_cholesky = linalg::cholesky(&kmm, dimension)
        .ok_or_else(|| PredictiveProcessError::Numerical("Kmm is not positive definite".into()))?;
    let mut knm = Vec::with_capacity(spec.coordinates.len() * dimension);
    let mut low_rank_diagonal = Vec::with_capacity(spec.coordinates.len());
    let mut residual_variances = Vec::with_capacity(spec.coordinates.len());
    let exact_variance = spec.amplitude * spec.amplitude;
    for coordinate in &spec.coordinates {
        let row = spec
            .knots
            .iter()
            .map(|knot| {
                matern32_1d(
                    coordinate.x_um,
                    knot.x_um,
                    spec.amplitude,
                    spec.length_scale_um,
                )
            })
            .collect::<Vec<_>>();
        let solved = linalg::solve_lower(&kmm_cholesky, dimension, &row);
        let low_rank = solved.iter().map(|value| value * value).sum::<f64>();
        let residual = exact_variance - low_rank;
        let tolerance = 1e-10 * exact_variance.max(1.0);
        if residual < -tolerance || !residual.is_finite() {
            return Err(PredictiveProcessError::Numerical(format!(
                "negative residual variance {residual} at {}",
                coordinate.id
            )));
        }
        knm.extend_from_slice(&row);
        low_rank_diagonal.push(low_rank);
        residual_variances.push(residual.max(0.0));
    }
    Ok(PredictiveProcessPlan {
        coordinates: spec.coordinates,
        knots: spec.knots,
        amplitude: spec.amplitude,
        length_scale_um: spec.length_scale_um,
        jitter: spec.jitter,
        diagonal_correction: spec.diagonal_correction,
        kmm_cholesky,
        knm,
        low_rank_diagonal,
        residual_variances,
    })
}

fn validate_and_sort(
    coordinates: &mut [FieldCoordinate1D],
    kind: &str,
) -> Result<(), PredictiveProcessError> {
    coordinates.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ids = HashSet::new();
    let mut values = BTreeSet::new();
    for coordinate in coordinates {
        if coordinate.id.is_empty()
            || coordinate.id.trim() != coordinate.id
            || !coordinate.x_um.is_finite()
            || !ids.insert(coordinate.id.as_str())
            || !values.insert(coordinate.x_um.to_bits())
        {
            return Err(PredictiveProcessError::InvalidInput(format!(
                "{kind} IDs and positions must be exact, finite, and unique"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knot_rows_have_only_jitter_scale_residual() {
        let plan = low_rank_predictive_process(PredictiveProcessSpec {
            amplitude: 1.0,
            length_scale_um: 1.0,
            jitter: 1e-12,
            diagonal_correction: true,
            coordinates: vec![point("c-0", 0.0), point("c-1", 1.0), point("c-2", 2.0)],
            knots: vec![point("k-0", 0.0), point("k-1", 2.0)],
        })
        .expect("plan");
        assert!(plan.residual_variances[0] <= 1e-9);
        assert!(plan.residual_variances[1] > 0.0);
        assert!(plan.residual_variances[2] <= 1e-9);
    }

    fn point(id: &str, x_um: f64) -> FieldCoordinate1D {
        FieldCoordinate1D {
            id: id.into(),
            x_um,
        }
    }
}
