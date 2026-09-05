//! Bounded limited-memory BFGS for smooth objectives with exact gradients.
use crate::NumericsError;
use std::collections::VecDeque;

/// Owned implementation bytes for callers whose result contract requires native source identity.
#[doc(hidden)]
pub const LBFGS_IMPLEMENTATION_SOURCE: &str = include_str!("lbfgs.rs");

struct Correction {
    s: Vec<f64>,
    y: Vec<f64>,
    rho: f64,
}

/// Minimize a smooth objective with limited-memory BFGS and bounded strong-Wolfe search.
///
/// Admits 1..=200 parameters, 1..=2,000 iterations, history 1..=10, and 1..=50 line-search
/// evaluations. Stops when gradient infinity norm reaches `gradient_tolerance` or adjacent
/// objective reduction is at most `relative_objective_tolerance * max(1, |old|, |new|)`.
/// The first search starts at `min(1, 1/||g||₂)` and later searches at one. Curvature history is
/// skipped when finite precision loses positive curvature. Uses O(parameters*history) storage.
/// Caller errors propagate unchanged; nonfinite values, lost descent, exhausted line search, and
/// exhausted iteration bounds are numerical errors. The callback must fill every gradient entry.
#[allow(clippy::too_many_arguments)]
pub fn minimize_lbfgs(
    initial: &[f64],
    maximum_iterations: usize,
    gradient_tolerance: f64,
    relative_objective_tolerance: f64,
    history_size: usize,
    maximum_line_search_evaluations: usize,
    mut objective: impl FnMut(&[f64], &mut [f64]) -> Result<f64, NumericsError>,
) -> Result<Vec<f64>, NumericsError> {
    validate_controls(
        initial,
        maximum_iterations,
        gradient_tolerance,
        relative_objective_tolerance,
        history_size,
        maximum_line_search_evaluations,
    )?;
    let n = initial.len();
    let mut x = initial.to_vec();
    let mut gradient = vec![0.0; n];
    let mut value = evaluate(&mut objective, &x, &mut gradient)?;
    let mut corrections = VecDeque::<Correction>::with_capacity(history_size);
    let mut direction = vec![0.0; n];
    let mut candidate = vec![0.0; n];
    let mut next_gradient = vec![0.0; n];
    let mut alphas = vec![0.0; history_size];
    for iteration in 0..maximum_iterations {
        if norm_inf(&gradient) <= gradient_tolerance {
            return Ok(x);
        }
        two_loop(&gradient, &corrections, &mut alphas, &mut direction);
        let slope = dot(&gradient, &direction);
        if !slope.is_finite() || slope >= 0.0 {
            return Err(failure("lost descent direction"));
        }
        let initial_step = if iteration == 0 {
            (1.0 / norm_l2(&direction)).min(1.0)
        } else {
            1.0
        };
        let next_value = line_search(
            &mut objective,
            &x,
            value,
            &direction,
            slope,
            initial_step,
            maximum_line_search_evaluations,
            &mut candidate,
            &mut next_gradient,
        )?;
        let reduction = value - next_value;
        let relative_stop = reduction >= 0.0
            && reduction
                <= relative_objective_tolerance * value.abs().max(next_value.abs()).max(1.0);
        let mut s = vec![0.0; n];
        let mut y = vec![0.0; n];
        for index in 0..n {
            s[index] = candidate[index] - x[index];
            y[index] = next_gradient[index] - gradient[index];
        }
        let ys = dot(&y, &s);
        if ys.is_finite() && ys > 0.0 {
            if corrections.len() == history_size {
                corrections.pop_front();
            }
            corrections.push_back(Correction {
                s,
                y,
                rho: 1.0 / ys,
            });
        }
        value = next_value;
        std::mem::swap(&mut x, &mut candidate);
        std::mem::swap(&mut gradient, &mut next_gradient);
        if relative_stop || norm_inf(&gradient) <= gradient_tolerance {
            return Ok(x);
        }
    }
    Err(failure("iteration limit exhausted before convergence"))
}

fn validate_controls(
    initial: &[f64],
    maximum_iterations: usize,
    gradient_tolerance: f64,
    relative_objective_tolerance: f64,
    history_size: usize,
    maximum_line_search_evaluations: usize,
) -> Result<(), NumericsError> {
    if initial.is_empty()
        || initial.len() > 200
        || maximum_iterations == 0
        || maximum_iterations > 2_000
        || !gradient_tolerance.is_finite()
        || gradient_tolerance <= 0.0
        || !relative_objective_tolerance.is_finite()
        || relative_objective_tolerance <= 0.0
        || !(1..=10).contains(&history_size)
        || !(1..=50).contains(&maximum_line_search_evaluations)
        || initial.iter().any(|value| !value.is_finite())
    {
        Err(NumericsError::Invalid(
            "L-BFGS dimensions or controls are invalid".into(),
        ))
    } else {
        Ok(())
    }
}

fn two_loop(
    gradient: &[f64],
    corrections: &VecDeque<Correction>,
    alphas: &mut [f64],
    direction: &mut [f64],
) {
    direction.copy_from_slice(gradient);
    for (index, correction) in corrections.iter().enumerate().rev() {
        let alpha = correction.rho * dot(&correction.s, direction);
        alphas[index] = alpha;
        for (value, y) in direction.iter_mut().zip(&correction.y) {
            *value -= alpha * y;
        }
    }
    let scale = corrections
        .back()
        .map(|correction| dot(&correction.s, &correction.y) / dot(&correction.y, &correction.y))
        .unwrap_or(1.0);
    for value in direction.iter_mut() {
        *value *= scale;
    }
    for (index, correction) in corrections.iter().enumerate() {
        let beta = correction.rho * dot(&correction.y, direction);
        for (value, s) in direction.iter_mut().zip(&correction.s) {
            *value += (alphas[index] - beta) * s;
        }
    }
    for value in direction {
        *value = -*value;
    }
}

#[allow(clippy::too_many_arguments)]
fn line_search(
    objective: &mut impl FnMut(&[f64], &mut [f64]) -> Result<f64, NumericsError>,
    x: &[f64],
    value: f64,
    direction: &[f64],
    slope: f64,
    initial_step: f64,
    maximum_evaluations: usize,
    candidate: &mut [f64],
    next_gradient: &mut [f64],
) -> Result<f64, NumericsError> {
    if !initial_step.is_finite() || initial_step <= 0.0 {
        return Err(failure("initial line-search step is invalid"));
    }
    let mut low = 0.0;
    let mut high = None;
    let mut low_value = value;
    let mut step = initial_step;
    for _ in 0..maximum_evaluations {
        for index in 0..x.len() {
            candidate[index] = x[index] + step * direction[index];
        }
        let next_value = evaluate(objective, candidate, next_gradient)?;
        let next_slope = dot(next_gradient, direction);
        if next_value > value + 1e-4 * step * slope || (low > 0.0 && next_value >= low_value) {
            high = Some(step);
        } else {
            if next_slope.abs() <= -0.9 * slope {
                return Ok(next_value);
            }
            if next_slope >= 0.0 {
                high = Some(step);
            } else {
                low = step;
                low_value = next_value;
            }
        }
        let next_step = high.map_or(step * 2.0, |upper| (low + upper) / 2.0);
        if !next_step.is_finite() || next_step == step {
            break;
        }
        step = next_step;
    }
    Err(failure("strong-Wolfe line search exhausted"))
}

fn evaluate(
    objective: &mut impl FnMut(&[f64], &mut [f64]) -> Result<f64, NumericsError>,
    parameters: &[f64],
    gradient: &mut [f64],
) -> Result<f64, NumericsError> {
    if parameters.iter().any(|value| !value.is_finite()) {
        return Err(failure("parameters are nonfinite"));
    }
    gradient.fill(f64::NAN);
    let value = objective(parameters, gradient)?;
    if !value.is_finite() || gradient.iter().any(|value| !value.is_finite()) {
        return Err(failure("objective or gradient is nonfinite"));
    }
    Ok(value)
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn norm_inf(values: &[f64]) -> f64 {
    values.iter().map(|value| value.abs()).fold(0.0, f64::max)
}

fn norm_l2(values: &[f64]) -> f64 {
    dot(values, values).sqrt()
}

fn failure(detail: &str) -> NumericsError {
    NumericsError::Numerical(format!("L-BFGS {detail}"))
}
