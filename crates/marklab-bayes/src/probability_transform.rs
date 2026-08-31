pub(crate) fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exponential = value.exp();
        exponential / (1.0 + exponential)
    }
}

pub(crate) fn clipped_logit(probability: f64) -> f64 {
    let clipped = probability.clamp(1e-12, 1.0 - 1e-12);
    (clipped / (1.0 - clipped)).ln()
}
