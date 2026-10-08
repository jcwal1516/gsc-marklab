use std::collections::{BTreeSet, HashSet};

use serde::Serialize;
use thiserror::Error;

use crate::{linalg, matern_covariance::matern32_1d};

#[derive(Clone, Debug, Serialize)]
pub struct NngpObservation {
    pub coordinate_id: String,
    pub x_um: f64,
    pub field_value: f64,
}

#[derive(Clone, Debug)]
pub struct NngpSpec {
    pub mean: f64,
    pub amplitude: f64,
    pub length_scale_um: f64,
    pub neighbors: usize,
    pub jitter: f64,
    pub variance_tolerance: f64,
    pub observations: Vec<NngpObservation>,
}

#[derive(Clone, Debug)]
pub struct NngpPlan {
    pub mean: f64,
    pub amplitude: f64,
    pub length_scale_um: f64,
    pub jitter: f64,
    pub variance_tolerance: f64,
    pub observations: Vec<NngpObservation>,
    pub neighbor_indices: Vec<Vec<usize>>,
    pub coefficients: Vec<Vec<f64>>,
    pub conditional_variances: Vec<f64>,
}

#[derive(Debug, Error)]
pub enum NngpError {
    #[error("invalid NNGP input: {0}")]
    InvalidInput(String),
    #[error("NNGP numerical failure: {0}")]
    Numerical(String),
}

pub fn build_nngp(mut spec: NngpSpec) -> Result<NngpPlan, NngpError> {
    if !spec.mean.is_finite() {
        return Err(NngpError::InvalidInput("mean must be finite".into()));
    }
    for (value, name) in [
        (spec.amplitude, "amplitude"),
        (spec.length_scale_um, "length scale"),
        (spec.jitter, "jitter"),
        (spec.variance_tolerance, "variance tolerance"),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(NngpError::InvalidInput(format!(
                "{name} must be finite and positive"
            )));
        }
    }
    if !(2..=10_000).contains(&spec.observations.len()) || !(1..=64).contains(&spec.neighbors) {
        return Err(NngpError::InvalidInput(
            "requires 2-10000 observations and 1-64 neighbors".into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut coordinates = BTreeSet::new();
    for row in &spec.observations {
        if row.coordinate_id.is_empty()
            || row.coordinate_id.trim() != row.coordinate_id
            || !row.x_um.is_finite()
            || !row.field_value.is_finite()
            || !ids.insert(row.coordinate_id.as_str())
            || !coordinates.insert(row.x_um.to_bits())
        {
            return Err(NngpError::InvalidInput(
                "coordinate IDs/positions must be exact, finite, and unique with finite field values"
                    .into(),
            ));
        }
    }
    spec.observations
        .sort_by(|left, right| left.x_um.total_cmp(&right.x_um));
    let n = spec.observations.len() as u128;
    let m = spec.neighbors.min(spec.observations.len() - 1) as u128;
    let work = n * m.pow(3);
    if work > 500_000_000 {
        return Err(NngpError::InvalidInput(format!(
            "requested NNGP work {work} exceeds 500000000"
        )));
    }
    let mut neighbor_indices = Vec::with_capacity(spec.observations.len());
    let mut coefficients = Vec::with_capacity(spec.observations.len());
    let mut conditional_variances = Vec::with_capacity(spec.observations.len());
    let marginal_variance = spec.amplitude * spec.amplitude + spec.jitter;
    for index in 0..spec.observations.len() {
        let count = spec.neighbors.min(index);
        let neighbors = (index - count..index).collect::<Vec<_>>();
        if neighbors.is_empty() {
            neighbor_indices.push(neighbors);
            coefficients.push(Vec::new());
            conditional_variances.push(marginal_variance);
            continue;
        }
        let mut covariance = vec![0.0; count * count];
        let mut cross = vec![0.0; count];
        for row in 0..count {
            cross[row] = matern32_1d(
                spec.observations[index].x_um,
                spec.observations[neighbors[row]].x_um,
                spec.amplitude,
                spec.length_scale_um,
            );
            for column in 0..count {
                covariance[row * count + column] = matern32_1d(
                    spec.observations[neighbors[row]].x_um,
                    spec.observations[neighbors[column]].x_um,
                    spec.amplitude,
                    spec.length_scale_um,
                );
            }
            covariance[row * count + row] += spec.jitter;
        }
        let lower = linalg::cholesky(&covariance, count)
            .ok_or_else(|| NngpError::Numerical("covariance is not positive definite".into()))?;
        let mut solved = linalg::solve_lower(&lower, count, &cross);
        linalg::solve_upper_from_lower_transpose_in_place(&lower, count, &mut solved);
        let conditional = marginal_variance
            - cross
                .iter()
                .zip(&solved)
                .map(|(left, right)| left * right)
                .sum::<f64>();
        if !conditional.is_finite() || conditional <= spec.variance_tolerance {
            return Err(NngpError::Numerical(format!(
                "conditional variance {conditional} at {} is not above tolerance {}",
                spec.observations[index].coordinate_id, spec.variance_tolerance
            )));
        }
        neighbor_indices.push(neighbors);
        coefficients.push(solved);
        conditional_variances.push(conditional);
    }
    Ok(NngpPlan {
        mean: spec.mean,
        amplitude: spec.amplitude,
        length_scale_um: spec.length_scale_um,
        jitter: spec.jitter,
        variance_tolerance: spec.variance_tolerance,
        observations: spec.observations,
        neighbor_indices,
        coefficients,
        conditional_variances,
    })
}

pub fn nngp_log_density(plan: &NngpPlan) -> Result<f64, NngpError> {
    let mut total = 0.0;
    for index in 0..plan.observations.len() {
        let centered = plan.observations[index].field_value - plan.mean;
        let conditional_mean = plan.neighbor_indices[index]
            .iter()
            .zip(&plan.coefficients[index])
            .map(|(neighbor, coefficient)| {
                coefficient * (plan.observations[*neighbor].field_value - plan.mean)
            })
            .sum::<f64>();
        let residual = centered - conditional_mean;
        let variance = plan.conditional_variances[index];
        total += -0.5
            * ((2.0 * std::f64::consts::PI).ln() + variance.ln() + residual * residual / variance);
    }
    if !total.is_finite() {
        return Err(NngpError::Numerical(
            "NNGP log density is non-finite".into(),
        ));
    }
    Ok(total)
}

pub fn full_gp_log_density(plan: &NngpPlan) -> Result<f64, NngpError> {
    if plan.observations.len() > 128 {
        return Err(NngpError::InvalidInput(
            "full GP reference is limited to 128 observations".into(),
        ));
    }
    let dimension = plan.observations.len();
    let mut covariance = vec![0.0; dimension * dimension];
    let centered = plan
        .observations
        .iter()
        .map(|row| row.field_value - plan.mean)
        .collect::<Vec<_>>();
    for row in 0..dimension {
        for column in 0..dimension {
            covariance[row * dimension + column] = matern32_1d(
                plan.observations[row].x_um,
                plan.observations[column].x_um,
                plan.amplitude,
                plan.length_scale_um,
            );
        }
        covariance[row * dimension + row] += plan.jitter;
    }
    let lower = linalg::cholesky(&covariance, dimension)
        .ok_or_else(|| NngpError::Numerical("covariance is not positive definite".into()))?;
    let whitened = linalg::solve_lower(&lower, dimension, &centered);
    let quadratic = whitened.iter().map(|value| value * value).sum::<f64>();
    let log_determinant = 2.0
        * (0..dimension)
            .map(|index| lower[index * dimension + index].ln())
            .sum::<f64>();
    let result =
        -0.5 * (dimension as f64 * (2.0 * std::f64::consts::PI).ln() + log_determinant + quadratic);
    if !result.is_finite() {
        return Err(NngpError::Numerical(
            "full GP log density is non-finite".into(),
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_predecessors_equal_full_density() {
        let plan = build_nngp(NngpSpec {
            mean: 0.0,
            amplitude: 1.0,
            length_scale_um: 1.5,
            neighbors: 3,
            jitter: 1e-12,
            variance_tolerance: 1e-12,
            observations: vec![
                point("c-0", 0.0, 0.2),
                point("c-1", 1.0, -0.1),
                point("c-2", 2.0, 0.4),
                point("c-3", 3.0, 0.0),
            ],
        })
        .expect("plan");
        let difference =
            (nngp_log_density(&plan).unwrap() - full_gp_log_density(&plan).unwrap()).abs();
        assert!(difference <= 1e-9, "{difference}");
    }

    fn point(id: &str, x_um: f64, field_value: f64) -> NngpObservation {
        NngpObservation {
            coordinate_id: id.into(),
            x_um,
            field_value,
        }
    }
}
