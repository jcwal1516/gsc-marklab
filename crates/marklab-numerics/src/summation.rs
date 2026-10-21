//! Neumaier compensated summation in fixed input order.

mod finite;
pub use finite::{FiniteNeumaierError, FiniteNeumaierSum};

/// Running Neumaier sum.
///
/// Unlike Kahan summation it stays exact when an addend is larger than the running total,
/// so `[1e16, 1, -1e16]` sums to `1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NeumaierSum {
    sum: f64,
    correction: f64,
}

impl NeumaierSum {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
    }

    /// Merge another running sum, keeping both parts of its residual.
    pub fn add_sum(&mut self, other: &NeumaierSum) {
        self.add(other.sum);
        self.add(other.correction);
    }

    /// Remove another running sum, e.g. when a sliding window drops a bin.
    pub fn subtract_sum(&mut self, other: &NeumaierSum) {
        self.add(-other.sum);
        self.add(-other.correction);
    }

    pub fn total(&self) -> f64 {
        self.sum + self.correction
    }
}

/// Neumaier sum of `values` in iteration order.
pub fn neumaier_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = NeumaierSum::new();
    for value in values {
        sum.add(value);
    }
    sum.total()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_small_terms_that_naive_and_kahan_summation_lose() {
        let values = [1.0e16, 1.0, -1.0e16];

        assert_eq!(values.iter().sum::<f64>(), 0.0);
        assert_eq!(neumaier_sum(values), 1.0);
    }

    #[test]
    fn empty_and_single_inputs_are_exact() {
        assert_eq!(neumaier_sum([]), 0.0);
        assert_eq!(neumaier_sum([-2.5]), -2.5);
    }

    #[test]
    fn running_sum_matches_the_one_shot_sum() {
        let values = [0.1, 0.2, 0.3, -0.6, 1.0e-17];
        let mut running = NeumaierSum::new();
        for value in values {
            running.add(value);
        }

        assert_eq!(running.total().to_bits(), neumaier_sum(values).to_bits());
    }

    #[test]
    fn subtracting_a_merged_sum_restores_the_original() {
        let mut window = NeumaierSum::new();
        window.add(1.0);
        let mut bin = NeumaierSum::new();
        for value in [1.0e16, 3.0, -1.0e16] {
            bin.add(value);
        }
        window.add_sum(&bin);
        assert_eq!(window.total(), 4.0);

        window.subtract_sum(&bin);
        assert_eq!(window.total(), 1.0);
    }

    #[test]
    fn non_finite_inputs_propagate() {
        assert!(neumaier_sum([1.0, f64::NAN]).is_nan());
        assert!(!neumaier_sum([f64::MAX, f64::MAX, -f64::MAX]).is_finite());
    }
}
