pub fn hann_weight(index: usize, len: usize) -> f64 {
    if len <= 1 {
        return 1.0;
    }
    0.5 * (1.0 - (2.0 * std::f64::consts::PI * index as f64 / (len - 1) as f64).cos())
}
