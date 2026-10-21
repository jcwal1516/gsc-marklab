#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Stage at which finite compensated summation failed.
pub enum FiniteNeumaierError {
    /// An input was NaN or infinite; the accumulator is unchanged.
    NonFiniteInput,
    /// Adding a finite input overflowed the running sum.
    SumOverflow,
    /// Updating the compensation produced a non-finite value.
    CorrectionOverflow,
    /// Combining the running sum and compensation overflowed.
    TotalOverflow,
}

#[derive(Clone, Copy, Debug, Default)]
/// Neumaier summation that rejects non-finite inputs and arithmetic overflow.
pub struct FiniteNeumaierSum {
    sum: f64,
    correction: f64,
}

impl FiniteNeumaierSum {
    /// Add one finite term, reporting the stage of any arithmetic failure.
    pub fn add(&mut self, value: f64) -> Result<(), FiniteNeumaierError> {
        if !value.is_finite() {
            return Err(FiniteNeumaierError::NonFiniteInput);
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(FiniteNeumaierError::SumOverflow);
        }
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        if !self.correction.is_finite() {
            return Err(FiniteNeumaierError::CorrectionOverflow);
        }
        self.sum = next;
        Ok(())
    }

    /// Return the finite compensated total.
    pub fn total(self) -> Result<f64, FiniteNeumaierError> {
        let total = self.sum + self.correction;
        total
            .is_finite()
            .then_some(total)
            .ok_or(FiniteNeumaierError::TotalOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_recovers_the_exact_total() {
        // 1 + 3 - 2 + 0.25 is exact in binary; naive summation loses the 1 and the 3.
        let values = [1.0e16, 1.0, -1.0e16, 3.0, -2.0, 0.25];
        let mut sum = FiniteNeumaierSum::default();
        for value in values {
            sum.add(value).unwrap();
        }
        assert_eq!(values.iter().sum::<f64>(), 1.25);
        assert_eq!(sum.total(), Ok(2.25));
        assert_eq!(
            sum.total().unwrap().to_bits(),
            crate::summation::neumaier_sum(values).to_bits()
        );
    }

    #[test]
    fn rejects_each_nonfinite_input_without_mutating_the_total() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut sum = FiniteNeumaierSum::default();
            sum.add(2.0).unwrap();
            assert_eq!(sum.add(value), Err(FiniteNeumaierError::NonFiniteInput));
            assert_eq!(sum.total().unwrap().to_bits(), 2.0_f64.to_bits());
        }
    }

    #[test]
    fn distinguishes_running_sum_and_final_total_overflow() {
        let mut running = FiniteNeumaierSum::default();
        running.add(f64::MAX).unwrap();
        assert_eq!(running.add(f64::MAX), Err(FiniteNeumaierError::SumOverflow));
        assert_eq!(running.total(), Ok(f64::MAX));

        let final_total = FiniteNeumaierSum {
            sum: f64::MAX,
            correction: f64::MAX,
        };
        assert_eq!(final_total.total(), Err(FiniteNeumaierError::TotalOverflow));
    }

    #[test]
    fn reports_nonfinite_correction_state_separately() {
        let mut sum = FiniteNeumaierSum {
            sum: 0.0,
            correction: f64::INFINITY,
        };
        assert_eq!(sum.add(0.0), Err(FiniteNeumaierError::CorrectionOverflow));
    }
}
