//! Bounded dense inverse-BFGS with a strong-Wolfe line search.
use crate::NumericsError;

/// Minimize a smooth objective using an exact caller-provided gradient.
///
/// The current caller is patient-level regularized logistic fitting (at most 129 parameters).
/// Starts with an identity inverse Hessian, uses c1=1e-4/c2=0.9 strong-Wolfe steps, and requires
/// the infinity norm of the gradient to meet `gradient_tolerance`. Uses O(d²) storage. Each
/// iteration permits at most 64 objective evaluations. Caller errors (including deadlines)
/// propagate unchanged; nonfinite evaluations, lost descent and exhausted bounds are errors.
/// The callback must fill every gradient element. There is no convergence-on-small-step fallback.
pub fn minimize_bfgs(
    initial: &[f64],
    maximum_iterations: usize,
    gradient_tolerance: f64,
    mut objective: impl FnMut(&[f64], &mut [f64]) -> Result<f64, NumericsError>,
) -> Result<Vec<f64>, NumericsError> {
    let n = initial.len();
    if n == 0
        || n > 129
        || maximum_iterations == 0
        || maximum_iterations > 1000
        || !gradient_tolerance.is_finite()
        || gradient_tolerance <= 0.0
        || initial.iter().any(|v| !v.is_finite())
    {
        return Err(NumericsError::Invalid(
            "BFGS dimensions or controls are invalid".into(),
        ));
    }
    let mut x = initial.to_vec();
    let mut g = vec![0.0; n];
    let mut f = evaluate(&mut objective, &x, &mut g)?;
    let mut previous_f = f + dot(&g, &g).sqrt() / 2.0;
    let mut h = vec![0.0; n * n];
    for i in 0..n {
        h[i * n + i] = 1.0;
    }
    let mut direction = vec![0.0; n];
    let mut candidate = vec![0.0; n];
    let mut next_g = vec![0.0; n];
    let mut s = vec![0.0; n];
    let mut y = vec![0.0; n];
    let mut hy = vec![0.0; n];
    for _ in 0..maximum_iterations {
        if norm_inf(&g) <= gradient_tolerance {
            return Ok(x);
        }
        for (row, p) in h.chunks_exact(n).zip(&mut direction) {
            *p = -dot(row, &g);
        }
        let slope = dot(&g, &direction);
        if !slope.is_finite() || slope >= 0.0 {
            return Err(failure("lost descent direction"));
        }
        let first_step = (2.02 * (f - previous_f) / slope).min(1.0);
        let mut low = 0.0;
        let mut high = None;
        let mut low_f = f;
        let mut step = if first_step > 0.0 { first_step } else { 1.0 };
        let mut accepted = None;
        for _ in 0..64 {
            for i in 0..n {
                candidate[i] = x[i] + step * direction[i];
            }
            let next_f = evaluate(&mut objective, &candidate, &mut next_g)?;
            let next_slope = dot(&next_g, &direction);
            if next_f > f + 1e-4 * step * slope || (low > 0.0 && next_f >= low_f) {
                high = Some(step);
            } else {
                if next_slope.abs() <= -0.9 * slope {
                    accepted = Some(next_f);
                    break;
                }
                if next_slope >= 0.0 {
                    high = Some(step);
                } else {
                    low = step;
                    low_f = next_f;
                }
            }
            let new_step = high.map_or(step * 2.0, |upper| (low + upper) / 2.0);
            if !new_step.is_finite() || new_step == step {
                break;
            }
            step = new_step;
        }
        let next_f = accepted.ok_or_else(|| failure("strong-Wolfe line search exhausted"))?;
        for i in 0..n {
            s[i] = candidate[i] - x[i];
            y[i] = next_g[i] - g[i];
        }
        let ys = dot(&y, &s);
        if !ys.is_finite() || ys <= 0.0 {
            return Err(failure("nonpositive curvature"));
        }
        for (row, value) in h.chunks_exact(n).zip(&mut hy) {
            *value = dot(row, &y);
        }
        let factor = (ys + dot(&y, &hy)) / (ys * ys);
        for i in 0..n {
            for j in 0..n {
                h[i * n + j] += factor * s[i] * s[j] - (hy[i] * s[j] + s[i] * hy[j]) / ys;
            }
        }
        if h.iter().any(|v| !v.is_finite()) {
            return Err(failure("inverse Hessian is nonfinite"));
        }
        previous_f = f;
        f = next_f;
        std::mem::swap(&mut x, &mut candidate);
        std::mem::swap(&mut g, &mut next_g);
    }
    if norm_inf(&g) <= gradient_tolerance {
        Ok(x)
    } else {
        Err(failure(
            "iteration limit exhausted before gradient convergence",
        ))
    }
}

fn evaluate(
    objective: &mut impl FnMut(&[f64], &mut [f64]) -> Result<f64, NumericsError>,
    x: &[f64],
    g: &mut [f64],
) -> Result<f64, NumericsError> {
    if x.iter().any(|v| !v.is_finite()) {
        return Err(failure("parameters are nonfinite"));
    }
    g.fill(f64::NAN);
    let f = objective(x, g)?;
    if !f.is_finite() || g.iter().any(|v| !v.is_finite()) {
        return Err(failure("objective or gradient is nonfinite"));
    }
    Ok(f)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn norm_inf(g: &[f64]) -> f64 {
    g.iter().map(|v| v.abs()).fold(0.0, f64::max)
}
fn failure(detail: &str) -> NumericsError {
    NumericsError::Numerical(format!("BFGS {detail}"))
}
