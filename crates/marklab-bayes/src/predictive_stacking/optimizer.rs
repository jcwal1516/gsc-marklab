//! Density-specific BFGS fitting with an active-set simplex quadratic subproblem.
use crate::{linalg, BayesError};
use std::time::Instant;

pub(super) struct Prepared {
    pub matrix: Vec<f64>,
    pub shifts: Vec<f64>,
    pub columns: usize,
}
fn error(message: &str) -> BayesError {
    BayesError::InvalidSpec(format!("predictive stacking {message}"))
}
pub(super) fn deadline_check(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(error("deadline exceeded"))
    } else {
        Ok(())
    }
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

impl Prepared {
    fn objective(
        &self,
        weights: &[f64],
        skip: Option<usize>,
        gradient: &mut [f64],
        deadline: Instant,
    ) -> Result<f64, BayesError> {
        deadline_check(deadline)?;
        gradient.fill(0.);
        let mut value = 0.;
        for (i, row) in self.matrix.chunks_exact(self.columns).enumerate() {
            if Some(i) == skip {
                continue;
            }
            let mixture = row
                .iter()
                .zip(weights)
                .map(|(q, w)| q * w.max(1e-300))
                .sum::<f64>();
            value -= self.shifts[i] + mixture.ln();
            for (g, q) in gradient.iter_mut().zip(row) {
                *g -= q / mixture;
            }
        }
        if !value.is_finite() || gradient.iter().any(|g| !g.is_finite()) {
            return Err(error("objective or gradient is nonfinite"));
        }
        Ok(value)
    }
}

pub(super) fn fit(
    prepared: &Prepared,
    skip: Option<usize>,
    deadline: Instant,
) -> Result<Vec<f64>, BayesError> {
    let n = prepared.columns;
    let mut weights = vec![1. / n as f64; n];
    let mut gradient = vec![0.; n];
    let mut value = prepared.objective(&weights, skip, &mut gradient, deadline)?;
    let mut hessian = vec![0.; n * n];
    for i in 0..n {
        hessian[i * n + i] = 1.;
    }
    for _ in 0..2000 {
        deadline_check(deadline)?;
        // A common gradient component is orthogonal to every feasible simplex step.
        // Remove it before the QP and directional dot product to avoid cancellation near a fit.
        let anchor = (0..n)
            .max_by(|a, b| weights[*a].total_cmp(&weights[*b]))
            .expect("nonempty admitted models");
        let centered = gradient
            .iter()
            .map(|g| g - gradient[anchor])
            .collect::<Vec<_>>();
        let direction = simplex_direction(&hessian, &centered, &weights)?;
        let slope = dot(&centered, &direction);
        // Retain the reference's absolute directional/objective stopping target, including
        // stationary simplex boundaries and weakly identified model weights.
        if slope.abs() < 1e-12 {
            return Ok(weights);
        }
        if !slope.is_finite() || slope >= 0. {
            return Err(error("quadratic model is not a descent direction"));
        }
        let mut step = 1.;
        let mut candidate = vec![0.; n];
        let mut next_gradient = vec![0.; n];
        let mut next_value = 0.;
        let mut accepted = false;
        for _ in 0..64 {
            for i in 0..n {
                candidate[i] = (weights[i] + step * direction[i]).max(0.);
            }
            let total = candidate.iter().sum::<f64>();
            for w in &mut candidate {
                *w /= total;
            }
            next_value = prepared.objective(&candidate, skip, &mut next_gradient, deadline)?;
            if next_value <= value + 0.1 * step * slope
                || (next_value <= value && -step * slope < 1e-12)
            {
                accepted = true;
                break;
            }
            step *= 0.5;
        }
        if !accepted {
            return Err(error("line search did not converge"));
        }
        let displacement = candidate
            .iter()
            .zip(&weights)
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>();
        if (next_value - value).abs() < 1e-12 || dot(&displacement, &displacement).sqrt() < 1e-12 {
            return Ok(candidate);
        }
        let bs = hessian
            .chunks_exact(n)
            .map(|row| dot(row, &displacement))
            .collect::<Vec<_>>();
        let sbs = dot(&displacement, &bs);
        let mut y = next_gradient
            .iter()
            .zip(&gradient)
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>();
        let sy = dot(&displacement, &y);
        if !sbs.is_finite() || sbs <= 0. {
            return Err(error("quadratic curvature is not positive"));
        }
        // Powell damping changes only the local quadratic model, never the likelihood.
        if sy < 0.2 * sbs {
            let theta = 0.8 * sbs / (sbs - sy);
            for i in 0..n {
                y[i] = theta * y[i] + (1. - theta) * bs[i];
            }
        }
        let sy = dot(&displacement, &y);
        for i in 0..n {
            for j in 0..n {
                hessian[i * n + j] += y[i] * y[j] / sy - bs[i] * bs[j] / sbs;
            }
        }
        weights = candidate;
        gradient = next_gradient;
        value = next_value;
    }
    Err(error("optimizer exceeded 2000 iterations"))
}

fn simplex_direction(
    hessian: &[f64],
    gradient: &[f64],
    weights: &[f64],
) -> Result<Vec<f64>, BayesError> {
    let n = weights.len();
    let mut candidate = weights.to_vec();
    let mut free = weights.iter().map(|w| *w > 0.).collect::<Vec<_>>();
    for _ in 0..256 {
        let indices = (0..n).filter(|i| free[*i]).collect::<Vec<_>>();
        let d = indices.len();
        let displacement = candidate
            .iter()
            .zip(weights)
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>();
        let current = hessian
            .chunks_exact(n)
            .zip(gradient)
            .map(|(row, g)| g + dot(row, &displacement))
            .collect::<Vec<_>>();
        let matrix = indices
            .iter()
            .flat_map(|i| indices.iter().map(move |j| hessian[i * n + j]))
            .collect::<Vec<_>>();
        let lower = linalg::cholesky(&matrix, d)
            .ok_or_else(|| error("simplex quadratic factorization failed"))?;
        let solve = |right: &[f64]| {
            let mut x = linalg::solve_lower(&lower, d, right);
            linalg::solve_upper_from_lower_transpose_in_place(&lower, d, &mut x);
            x
        };
        let hg = solve(&indices.iter().map(|i| current[*i]).collect::<Vec<_>>());
        let ones = solve(&vec![1.; d]);
        let lambda = hg.iter().sum::<f64>() / ones.iter().sum::<f64>();
        let mut direction = vec![0.; n];
        for (k, i) in indices.iter().enumerate() {
            direction[*i] = -hg[k] + lambda * ones[k];
        }
        if direction.iter().any(|v| !v.is_finite()) {
            return Err(error("simplex direction is nonfinite"));
        }
        if dot(&direction, &direction).sqrt() < 1e-10 {
            let entering = (0..n)
                .filter(|i| !free[*i] && current[*i] < lambda - 1e-10)
                .min_by(|a, b| current[*a].total_cmp(&current[*b]));
            if let Some(i) = entering {
                free[i] = true;
                continue;
            }
            return Ok(displacement);
        }
        let mut alpha = 1.;
        let mut leaving = None;
        for &i in &indices {
            if direction[i] < 0. {
                let bound = -candidate[i] / direction[i];
                if bound <= alpha {
                    alpha = bound;
                    leaving = Some(i);
                }
            }
        }
        for i in 0..n {
            candidate[i] = (candidate[i] + alpha * direction[i]).max(0.);
        }
        if let Some(i) = leaving {
            candidate[i] = 0.;
            free[i] = false;
        }
    }
    Err(error("simplex subproblem exceeded its iteration bound"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simplex_quadratic_step_preserves_symmetry_and_reaches_the_boundary() {
        let direction = simplex_direction(
            &[1., 0., 0., 0., 1., 0., 0., 0., 1.],
            &[2., 2., 0.],
            &[1. / 3.; 3],
        )
        .unwrap();
        for (actual, expected) in direction.iter().zip([-1. / 3., -1. / 3., 2. / 3.]) {
            assert!((actual - expected).abs() < 1e-12, "{direction:?}");
        }
    }
}

#[cfg(test)]
mod objective_tests {
    use super::*;
    #[test]
    fn prepared_gradient_matches_direct_density_ratios_and_finite_differences() {
        let prepared = Prepared {
            matrix: vec![1., 0.25, 0.25, 1.],
            shifts: vec![-0.2, -0.7],
            columns: 2,
        };
        let weights = [0.3, 0.7];
        let deadline = Instant::now() + std::time::Duration::from_secs(1);
        let mut gradient = [0.; 2];
        prepared
            .objective(&weights, None, &mut gradient, deadline)
            .unwrap();
        for j in 0..2 {
            let expected = -prepared.matrix[j] / (0.3 + 0.25 * 0.7)
                - prepared.matrix[2 + j] / (0.25 * 0.3 + 0.7);
            assert!((gradient[j] - expected).abs() < 1e-12);
            let mut plus = weights;
            let mut minus = weights;
            plus[j] += 1e-6;
            minus[j] -= 1e-6;
            let a = prepared
                .objective(&plus, None, &mut [0.; 2], deadline)
                .unwrap();
            let b = prepared
                .objective(&minus, None, &mut [0.; 2], deadline)
                .unwrap();
            assert!((gradient[j] - (a - b) / 2e-6).abs() < 1e-8);
        }
        assert!(fit(&prepared, None, Instant::now())
            .unwrap_err()
            .to_string()
            .contains("deadline"));
    }
}
