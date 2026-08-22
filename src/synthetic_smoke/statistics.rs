use super::model::BinomialConfidenceInterval;

pub(super) fn observed_rate(successes: usize, completed: usize) -> Option<f64> {
    (completed > 0).then_some(successes as f64 / completed as f64)
}

pub(super) fn wilson_interval(
    successes: usize,
    completed: usize,
) -> Option<BinomialConfidenceInterval> {
    if completed == 0 || successes > completed {
        return None;
    }
    const Z_95: f64 = 1.959_963_984_540_054;
    let n = completed as f64;
    let p = successes as f64 / n;
    let z2 = Z_95 * Z_95;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let half_width = Z_95 * ((p * (1.0 - p) + z2 / (4.0 * n)) / n).sqrt() / denominator;
    Some(BinomialConfidenceInterval {
        confidence_level: 0.95,
        lower: if successes == 0 {
            0.0
        } else {
            (center - half_width).max(0.0)
        },
        upper: if successes == completed {
            1.0
        } else {
            (center + half_width).min(1.0)
        },
    })
}
