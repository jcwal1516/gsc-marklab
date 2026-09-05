//! Coordinate maximization of the entropic partial-transport dual.
use crate::{transport::log_sum_exp, BayesError, PartialTransportSpec};
use std::time::Instant;

pub(super) struct SolvedPlan {
    pub masses: Vec<f64>,
    pub iterations: u32,
}

pub(super) fn check_deadline(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(BayesError::InvalidSpec(
            "partial transport deadline exceeded".into(),
        ))
    } else {
        Ok(())
    }
}
fn finite(value: f64) -> Result<f64, BayesError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BayesError::InvalidSpec(
            "partial transport scaling is nonfinite".into(),
        ))
    }
}

pub(super) fn solve(
    spec: &PartialTransportSpec,
    deadline: Instant,
) -> Result<SolvedPlan, BayesError> {
    let rows = spec.source.len();
    let columns = spec.target.len();
    let active_rows = spec
        .source
        .iter()
        .enumerate()
        .filter_map(|(i, r)| (r.mass > 0.).then_some(i))
        .collect::<Vec<_>>();
    let active_columns = spec
        .target
        .iter()
        .enumerate()
        .filter_map(|(i, r)| (r.mass > 0.).then_some(i))
        .collect::<Vec<_>>();
    let cells = active_rows
        .iter()
        .flat_map(|i| active_columns.iter().map(move |j| i * columns + j))
        .collect::<Vec<_>>();
    let minimum = cells
        .iter()
        .map(|i| spec.costs_row_major[*i])
        .fold(f64::INFINITY, f64::min);
    let mut kernel = vec![0.; rows * columns];
    for &i in &cells {
        kernel[i] = finite(-(spec.costs_row_major[i] - minimum) / spec.epsilon)?;
    }
    let log_source = spec
        .source
        .iter()
        .map(|r| if r.mass > 0. { r.mass.ln() } else { 0. })
        .collect::<Vec<_>>();
    let log_target = spec
        .target
        .iter()
        .map(|r| if r.mass > 0. { r.mass.ln() } else { 0. })
        .collect::<Vec<_>>();
    let mut r = vec![0.; rows];
    let mut c = vec![0.; columns];
    let mut z = finite(spec.transported_mass.ln() - log_sum_exp(cells.iter().map(|i| kernel[*i])))?;
    let mut masses = vec![0.; rows * columns];
    let mut row_mass = vec![0.; rows];
    let mut column_mass = vec![0.; columns];
    for iterations in 1..=2000 {
        check_deadline(deadline)?;
        // r/c are nonpositive dual multipliers. Cancel the old coordinate before its exact
        // update (equivalently, retain the KL/Dykstra correction for each inequality set).
        for &i in &active_rows {
            let product = finite(log_sum_exp(
                active_columns
                    .iter()
                    .map(|j| kernel[i * columns + j] + c[*j] + z),
            ))?;
            r[i] = finite(log_source[i] - product)?.min(0.);
        }
        for &j in &active_columns {
            let product = finite(log_sum_exp(
                active_rows
                    .iter()
                    .map(|i| kernel[i * columns + j] + r[*i] + z),
            ))?;
            c[j] = finite(log_target[j] - product)?.min(0.);
        }
        let maximum = finite(
            cells
                .iter()
                .map(|index| kernel[*index] + r[*index / columns] + c[*index % columns])
                .fold(f64::NEG_INFINITY, f64::max),
        )?;
        let mut weights = 0.;
        for &index in &cells {
            masses[index] =
                (kernel[index] + r[index / columns] + c[index % columns] - maximum).exp();
            weights += masses[index];
        }
        let scale = finite(spec.transported_mass / weights)?;
        z = finite(spec.transported_mass.ln() - maximum - weights.ln())?;
        row_mass.fill(0.);
        column_mass.fill(0.);
        let mut cost = 0.;
        let mut regularization = 0.;
        // Scale shifted weights directly, avoiding exp(log(requested_mass)) round-trip error.
        for &index in &cells {
            masses[index] *= scale;
            let mass = masses[index];
            row_mass[index / columns] += mass;
            column_mass[index % columns] += mass;
            cost += mass * spec.costs_row_major[index];
            if mass > 0. {
                regularization += mass * (mass.ln() - 1.);
            }
        }
        let transported = finite(row_mass.iter().sum())?;
        let objective = finite(cost + spec.epsilon * regularization)?;
        let mass_error = (transported - spec.transported_mass).abs();
        let mut violation = mass_error;
        // This L1 complementarity bound dominates the absolute primal-dual gap, including
        // the equality residual and the constant cost shift; cancellations cannot hide error.
        let mut residual = finite((minimum + spec.epsilon * z).abs() * mass_error)?;
        for &i in &active_rows {
            violation = violation.max((row_mass[i] - spec.source[i].mass).max(0.));
            residual += spec.epsilon * r[i].abs() * (row_mass[i] - spec.source[i].mass).abs();
        }
        for &j in &active_columns {
            violation = violation.max((column_mass[j] - spec.target[j].mass).max(0.));
            residual += spec.epsilon * c[j].abs() * (column_mass[j] - spec.target[j].mass).abs();
        }
        finite(residual)?;
        if violation <= super::super::FEASIBILITY_TOLERANCE
            && residual <= 1e-12 * (1. + objective.abs())
        {
            return Ok(SolvedPlan { masses, iterations });
        }
    }
    Err(BayesError::InvalidSpec(
        "partial transport did not meet feasibility and dual convergence within 2000 sweeps".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_partial_transport_deadline_stops_before_iteration() {
        let spec = PartialTransportSpec {
            source: vec![crate::TransportMass {
                id: "s".into(),
                mass: 1.,
            }],
            target: vec![crate::TransportMass {
                id: "t".into(),
                mass: 1.,
            }],
            costs_row_major: vec![0.],
            transported_mass: 1.,
            epsilon: 1.,
            timeout_seconds: 1,
        };
        let result = solve(&spec, Instant::now());
        assert!(result.err().unwrap().to_string().contains("deadline"));
    }
}
