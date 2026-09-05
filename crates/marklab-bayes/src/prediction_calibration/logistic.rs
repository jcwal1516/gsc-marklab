//! One/two-parameter calibration objectives; the offset model fixes slope to one.
use crate::{probability_transform::sigmoid, BayesError};
use std::time::Instant;

pub(super) fn check_deadline(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(BayesError::InvalidSpec(
            "calibration deadline exceeded".into(),
        ))
    } else {
        Ok(())
    }
}

struct Evaluation {
    value: f64,
    gradient: [f64; 2],
    hessian: [f64; 3],
}

fn evaluate(
    x: &[f64],
    y: &[f64],
    parameters: [f64; 2],
    ridge: f64,
    offset: bool,
    deadline: Instant,
) -> Result<Evaluation, BayesError> {
    check_deadline(deadline)?;
    let [a, b] = parameters;
    let mut e = Evaluation {
        value: ridge * (a * a + if offset { 0. } else { b * b }),
        gradient: [2. * ridge * a, if offset { 0. } else { 2. * ridge * b }],
        hessian: [2. * ridge, 0., if offset { 0. } else { 2. * ridge }],
    };
    for (&x, &y) in x.iter().zip(y) {
        let z = a + b * x;
        let p = sigmoid(z);
        e.value += z.max(0.) + (-z.abs()).exp().ln_1p() - y * z;
        let residual = p - y;
        let weight = p * (1. - p);
        e.gradient[0] += residual;
        e.hessian[0] += weight;
        if !offset {
            e.gradient[1] += residual * x;
            e.hessian[1] += weight * x;
            e.hessian[2] += weight * x * x;
        }
    }
    if !e.value.is_finite()
        || e.gradient
            .iter()
            .chain(e.hessian.iter())
            .any(|x| !x.is_finite())
    {
        return Err(BayesError::InvalidSpec(
            "calibration objective is nonfinite".into(),
        ));
    }
    Ok(e)
}

pub(super) fn fit(
    x: &[f64],
    y: &[f64],
    initial: [f64; 2],
    ridge: f64,
    offset: bool,
    deadline: Instant,
) -> Result<[f64; 2], BayesError> {
    let mut parameters = initial;
    let mut e = evaluate(x, y, parameters, ridge, offset, deadline)?;
    for _ in 0..1000 {
        if e.gradient.iter().all(|g| g.abs() <= 1e-10) {
            return Ok(parameters);
        }
        let [h00, h01, h11] = e.hessian;
        if h00 <= 0. {
            break;
        }
        // Solve the two-dimensional Hessian via its Schur complement. For an exactly
        // constant unpenalized predictor, move along [1,x], preserving the initial nullspace.
        let direction = if offset {
            [e.gradient[0] / h00, 0.]
        } else if ridge == 0. && x.iter().all(|v| *v == x[0]) {
            let scale = 1. + x[0] * x[0];
            let d = e.gradient[0] / h00 / scale;
            [d, d * x[0]]
        } else {
            let schur = h11 - h01 / h00 * h01;
            if schur <= 0. {
                break;
            }
            let d1 = (e.gradient[1] - h01 / h00 * e.gradient[0]) / schur;
            [(e.gradient[0] - h01 * d1) / h00, d1]
        };
        let descent = e.gradient[0] * direction[0] + e.gradient[1] * direction[1];
        if !descent.is_finite() || descent <= 0. {
            break;
        }
        let mut step = 1.;
        let mut accepted = None;
        for _ in 0..64 {
            let next = [
                parameters[0] - step * direction[0],
                parameters[1] - step * direction[1],
            ];
            let candidate = evaluate(x, y, next, ridge, offset, deadline)?;
            if candidate.value <= e.value - 1e-4 * step * descent {
                accepted = Some((next, candidate));
                break;
            }
            step *= 0.5;
        }
        let Some((next, candidate)) = accepted else {
            break;
        };
        let converged =
            (e.value - candidate.value) <= 1e-14 * e.value.abs().max(candidate.value.abs()).max(1.);
        parameters = next;
        e = candidate;
        if converged {
            return Ok(parameters);
        }
    }
    Err(BayesError::InvalidSpec(
        "calibration optimizer did not converge".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn analytic_gradient_and_hessian_match_centered_differences() {
        let x = [-3., -0.2, 0.1, 2.5];
        let y = [0.1, 0.7, 0.3, 0.9];
        let p = [0.3, 0.7];
        let deadline = Instant::now() + Duration::from_secs(10);
        let h = 1e-5;
        for offset in [false, true] {
            let e = evaluate(&x, &y, p, 1e-8, offset, deadline).unwrap();
            for axis in 0..if offset { 1 } else { 2 } {
                let mut plus = p;
                let mut minus = p;
                plus[axis] += h;
                minus[axis] -= h;
                let a = evaluate(&x, &y, plus, 1e-8, offset, deadline).unwrap();
                let b = evaluate(&x, &y, minus, 1e-8, offset, deadline).unwrap();
                assert!(((a.value - b.value) / (2. * h) - e.gradient[axis]).abs() < 1e-9);
                for row in 0..if offset { 1 } else { 2 } {
                    let index = axis + row;
                    assert!(
                        ((a.gradient[row] - b.gradient[row]) / (2. * h) - e.hessian[index]).abs()
                            < 1e-9
                    );
                }
            }
        }
    }
    #[test]
    fn expired_deadline_is_an_error_before_fit() {
        assert!(
            fit(&[0., 1.], &[0., 1.], [0., 0.], 0., false, Instant::now())
                .unwrap_err()
                .to_string()
                .contains("deadline")
        );
    }
}
