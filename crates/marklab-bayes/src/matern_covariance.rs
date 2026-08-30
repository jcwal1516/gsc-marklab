pub(crate) fn matern32_1d(left: f64, right: f64, amplitude: f64, length_scale: f64) -> f64 {
    let scaled = 3.0_f64.sqrt() * (left - right).abs() / length_scale;
    amplitude * amplitude * (1.0 + scaled) * (-scaled).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_dimensional_kernel_matches_the_previous_formula_bit_for_bit() {
        let cases: [(f64, f64, f64, f64); 3] = [
            (0.0, 0.0, 1.0, 2.0),
            (-2.5, 7.25, 1.75, 3.5),
            (1.0e6, 1.0e6 + 0.25, 0.5, 0.125),
        ];
        for (left, right, amplitude, length_scale) in cases {
            let scaled = 3.0_f64.sqrt() * (left - right).abs() / length_scale;
            let expected = amplitude * amplitude * (1.0 + scaled) * (-scaled).exp();
            assert_eq!(
                matern32_1d(left, right, amplitude, length_scale).to_bits(),
                expected.to_bits()
            );
        }
    }
}
