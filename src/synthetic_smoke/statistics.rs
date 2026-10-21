use super::model::BinomialConfidenceInterval;

pub(super) fn observed_rate(successes: usize, completed: usize) -> Option<f64> {
    (completed > 0).then_some(successes as f64 / completed as f64)
}

pub(super) fn wilson_interval(
    successes: usize,
    completed: usize,
) -> Option<BinomialConfidenceInterval> {
    const Z_95: f64 = 1.959_963_984_540_054;
    wilson_bounds(successes, completed, Z_95).map(|(lower, upper)| BinomialConfidenceInterval {
        confidence_level: 0.95,
        lower,
        upper,
    })
}

/// Wilson score interval bounds at the standard-normal quantile `z`.
pub(super) fn wilson_bounds(successes: usize, completed: usize, z: f64) -> Option<(f64, f64)> {
    if completed == 0 || successes > completed {
        return None;
    }
    let n = completed as f64;
    let p = successes as f64 / n;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let half_width = z * ((p * (1.0 - p) + z2 / (4.0 * n)) / n).sqrt() / denominator;
    let lower = if successes == 0 {
        0.0
    } else {
        (center - half_width).max(0.0)
    };
    let upper = if successes == completed {
        1.0
    } else {
        (center + half_width).min(1.0)
    };
    Some((lower, upper))
}
