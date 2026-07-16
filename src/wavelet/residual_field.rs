pub fn standardized_residual(local_p: f64, global_p: f64, n_eff: f64) -> f64 {
    if n_eff <= 0.0 {
        return 0.0;
    }
    let denom = (global_p * (1.0 - global_p) / n_eff + f64::EPSILON).sqrt();
    (local_p - global_p) / denom
}
