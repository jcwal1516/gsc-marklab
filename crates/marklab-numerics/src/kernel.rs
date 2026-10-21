//! Stationary covariance kernels.

/// Matérn covariance with smoothness 3/2 at `distance`:
/// `amplitude² (1 + √3 d/ℓ) exp(−√3 d/ℓ)`.
pub fn matern32(distance: f64, amplitude: f64, length_scale: f64) -> f64 {
    let scaled = 3.0_f64.sqrt() * distance / length_scale;
    amplitude * amplitude * (1.0 + scaled) * (-scaled).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_closed_form_values() {
        let at_one_scaled_length = (1.0 + 3.0_f64.sqrt()) * (-(3.0_f64.sqrt())).exp();

        assert_eq!(matern32(0.0, 2.0, 1.0), 4.0);
        assert!(
            (matern32(1.0 / 3.0_f64.sqrt(), 2.0, 1.0) - 4.0 * 2.0 * (-1.0_f64).exp()).abs() < 1e-15
        );
        assert!((matern32(2.5, 1.0, 2.5) - at_one_scaled_length).abs() < 1e-15);
    }

    #[test]
    fn decreases_with_distance() {
        assert!(matern32(1.0, 1.0, 1.0) > matern32(2.0, 1.0, 1.0));
    }
}
