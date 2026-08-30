/// Add one value with the repository's fixed-order Kahan compensation step.
#[inline]
pub(crate) fn kahan_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_add(sum: &mut f64, correction: &mut f64, value: f64) {
        let corrected = value - *correction;
        let next = *sum + corrected;
        *correction = (next - *sum) - corrected;
        *sum = next;
    }

    #[test]
    fn shared_step_is_bitwise_equal_to_the_previous_local_algorithm() {
        let values = [1.0e16, 1.0, -3.0, 0.25, -1.0e16, -0.0, 7.5];
        let (mut actual_sum, mut actual_correction) = (0.0, 0.0);
        let (mut expected_sum, mut expected_correction) = (0.0, 0.0);

        for value in values {
            kahan_add(&mut actual_sum, &mut actual_correction, value);
            reference_add(&mut expected_sum, &mut expected_correction, value);
        }

        assert_eq!(actual_sum.to_bits(), expected_sum.to_bits());
        assert_eq!(actual_correction.to_bits(), expected_correction.to_bits());
    }
}
