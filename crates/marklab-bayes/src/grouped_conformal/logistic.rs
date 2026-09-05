//! Training-only regularized logistic likelihood on a contiguous standardized matrix.
use super::super::{GroupedConformalModel, GroupedConformalSpec};
use marklab_numerics::{minimize_bfgs, NumericsError};
use std::time::Instant;

pub(super) fn fit(
    spec: &GroupedConformalSpec,
    deadline: Instant,
) -> Result<GroupedConformalModel, NumericsError> {
    let d = spec.feature_names.len();
    let training = spec
        .patients
        .iter()
        .filter(|p| p.split == "train")
        .collect::<Vec<_>>();
    let mut mean = vec![0.0; d];
    for p in &training {
        for (m, x) in mean.iter_mut().zip(&p.features) {
            *m += x;
        }
    }
    for m in &mut mean {
        *m /= training.len() as f64;
    }
    let mut sd = vec![0.0; d];
    for p in &training {
        for ((sum, x), m) in sd.iter_mut().zip(&p.features).zip(&mean) {
            *sum += (x - m) * (x - m);
        }
    }
    for s in &mut sd {
        *s = (*s / training.len() as f64).sqrt();
    }
    if mean.iter().any(|x| !x.is_finite()) || sd.iter().any(|x| !x.is_finite() || *x <= 1e-14) {
        return Err(NumericsError::Invalid(
            "every training feature must have finite population SD above 1e-14".into(),
        ));
    }
    let mut matrix = Vec::with_capacity(training.len() * d);
    let mut labels = Vec::with_capacity(training.len());
    for p in training {
        matrix.extend(
            p.features
                .iter()
                .zip(&mean)
                .zip(&sd)
                .map(|((x, m), s)| (x - m) / s),
        );
        labels.push(f64::from(p.label));
    }
    let parameters = match minimize_bfgs(&vec![0.0; d + 1], 500, 1e-8, |parameters, gradient| {
        objective(
            &matrix,
            &labels,
            parameters,
            gradient,
            spec.l2_penalty,
            deadline,
        )
    }) {
        Ok(parameters) => parameters,
        Err(first @ NumericsError::Numerical(_)) => {
            newton(&matrix, &labels, d, spec.l2_penalty, deadline).map_err(|second| {
                NumericsError::Numerical(format!(
                    "logistic BFGS failed ({first}); Newton recovery failed ({second})"
                ))
            })?
        }
        Err(error) => return Err(error),
    };
    Ok(GroupedConformalModel {
        training_mean: mean,
        training_population_sd: sd,
        intercept: parameters[0],
        coefficients: parameters[1..].to_vec(),
        l2_penalty: spec.l2_penalty,
    })
}

fn objective(
    matrix: &[f64],
    labels: &[f64],
    parameters: &[f64],
    gradient: &mut [f64],
    penalty: f64,
    deadline: Instant,
) -> Result<f64, NumericsError> {
    let d = parameters.len() - 1;
    gradient.fill(0.0);
    let mut value = 0.0;
    for (index, (row, label)) in matrix.chunks_exact(d).zip(labels).enumerate() {
        if index % 256 == 0 && Instant::now() >= deadline {
            return Err(NumericsError::Resource(
                "grouped conformal deadline exceeded".into(),
            ));
        }
        let z = parameters[0]
            + row
                .iter()
                .zip(&parameters[1..])
                .map(|(x, b)| x * b)
                .sum::<f64>();
        // Softplus in its stable branch, preserving the unnormalized reference likelihood.
        value += z.max(0.0) + (-z.abs()).exp().ln_1p() - label * z;
        let probability = if z >= 0.0 {
            1.0 / (1.0 + (-z).exp())
        } else {
            let e = z.exp();
            e / (1.0 + e)
        };
        let residual = probability - label;
        gradient[0] += residual;
        for (g, x) in gradient[1..].iter_mut().zip(row) {
            *g += residual * x;
        }
    }
    for (g, b) in gradient[1..].iter_mut().zip(&parameters[1..]) {
        *g += penalty * b;
        value += 0.5 * penalty * b * b;
    }
    Ok(value)
}

// The likelihood is convex and positive L2 identifies all slopes. Newton recovery uses the exact
// positive Hessian and the SAME gradient criterion when finite-precision Wolfe search stalls.
// The two strategies get 500 iterations each; the total never exceeds the reference's 1000.
fn newton(
    matrix: &[f64],
    labels: &[f64],
    d: usize,
    penalty: f64,
    deadline: Instant,
) -> Result<Vec<f64>, NumericsError> {
    let n = d + 1;
    let mut x = vec![0.0; n];
    let mut gradient = vec![0.0; n];
    let mut next_gradient = vec![0.0; n];
    let mut hessian = vec![0.0; n * n];
    let mut row_with_intercept = vec![1.0; n];
    let mut candidate = vec![0.0; n];
    let mut value = objective(matrix, labels, &x, &mut gradient, penalty, deadline)?;
    for _ in 0..500 {
        if gradient.iter().all(|v| v.abs() <= 1e-8) {
            return Ok(x);
        }
        hessian.fill(0.0);
        for (index, row) in matrix.chunks_exact(d).enumerate() {
            if index % 256 == 0 && Instant::now() >= deadline {
                return Err(NumericsError::Resource(
                    "grouped conformal deadline exceeded".into(),
                ));
            }
            let z = x[0] + row.iter().zip(&x[1..]).map(|(a, b)| a * b).sum::<f64>();
            let e = (-z.abs()).exp();
            let weight = e / ((1.0 + e) * (1.0 + e));
            row_with_intercept[1..].copy_from_slice(row);
            for i in 0..n {
                for j in 0..=i {
                    hessian[i * n + j] += weight * row_with_intercept[i] * row_with_intercept[j];
                }
            }
        }
        for i in 0..n {
            if i > 0 {
                hessian[i * n + i] += penalty;
            }
            for j in 0..i {
                hessian[j * n + i] = hessian[i * n + j];
            }
        }
        let lower = crate::linalg::cholesky(&hessian, n).ok_or_else(|| {
            NumericsError::Numerical("logistic Hessian is not finite positive definite".into())
        })?;
        let mut step = crate::linalg::solve_lower(&lower, n, &gradient);
        crate::linalg::solve_upper_from_lower_transpose_in_place(&lower, n, &mut step);
        let slope = -gradient.iter().zip(&step).map(|(a, b)| a * b).sum::<f64>();
        if !slope.is_finite() || slope >= 0.0 {
            return Err(NumericsError::Numerical(
                "logistic Newton direction is not descending".into(),
            ));
        }
        let mut scale = 1.0;
        let mut accepted = None;
        for _ in 0..64 {
            for i in 0..n {
                candidate[i] = x[i] - scale * step[i];
            }
            let next = objective(
                matrix,
                labels,
                &candidate,
                &mut next_gradient,
                penalty,
                deadline,
            )?;
            if !next.is_finite() || next_gradient.iter().any(|g| !g.is_finite()) {
                return Err(NumericsError::Numerical(
                    "logistic Newton objective or gradient is nonfinite".into(),
                ));
            }
            // At machine-resolution objective differences, stationarity is the stopping proof.
            if next_gradient.iter().all(|v| v.abs() <= 1e-8) {
                return Ok(candidate);
            }
            if next <= value + 1e-4 * scale * slope {
                accepted = Some(next);
                break;
            }
            scale *= 0.5;
        }
        value = accepted.ok_or_else(|| {
            NumericsError::Numerical("logistic Newton line search exhausted".into())
        })?;
        std::mem::swap(&mut x, &mut candidate);
        std::mem::swap(&mut gradient, &mut next_gradient);
    }
    Err(NumericsError::Numerical(
        "logistic Newton iteration limit exhausted".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logistic_gradient_matches_central_differences() {
        let matrix = [1.0, 2.0, -1.0, 0.2, 0.3, -0.5];
        let labels = [1.0, 0.0, 1.0];
        let p = [0.1, -0.4, 0.7];
        let mut g = [0.0; 3];
        let deadline = Instant::now() + std::time::Duration::from_secs(1);
        objective(&matrix, &labels, &p, &mut g, 0.1, deadline).unwrap();
        for i in 0..3 {
            let mut a = p;
            let mut b = p;
            a[i] += 1e-5;
            b[i] -= 1e-5;
            let finite = (objective(&matrix, &labels, &a, &mut [0.0; 3], 0.1, deadline).unwrap()
                - objective(&matrix, &labels, &b, &mut [0.0; 3], 0.1, deadline).unwrap())
                / 2e-5;
            assert!((finite - g[i]).abs() < 1e-9);
        }
    }
}
