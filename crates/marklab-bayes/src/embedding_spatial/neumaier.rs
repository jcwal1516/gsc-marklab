#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FiniteNeumaierError {
    NonFiniteInput,
    SumOverflow,
    CorrectionOverflow,
    TotalOverflow,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FiniteNeumaierSum {
    sum: f64,
    correction: f64,
}

impl FiniteNeumaierSum {
    pub(crate) fn add(&mut self, value: f64) -> Result<(), FiniteNeumaierError> {
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

    pub(crate) fn total(self) -> Result<f64, FiniteNeumaierError> {
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

    fn previous_add(state: &mut [f64; 2], value: f64) -> Result<(), FiniteNeumaierError> {
        if !value.is_finite() {
            return Err(FiniteNeumaierError::NonFiniteInput);
        }
        let next = state[0] + value;
        if !next.is_finite() {
            return Err(FiniteNeumaierError::SumOverflow);
        }
        state[1] += if state[0].abs() >= value.abs() {
            (state[0] - next) + value
        } else {
            (value - next) + state[0]
        };
        if !state[1].is_finite() {
            return Err(FiniteNeumaierError::CorrectionOverflow);
        }
        state[0] = next;
        Ok(())
    }

    #[test]
    fn cancellation_matches_the_previous_neumaier_loop_bit_for_bit() {
        let values = [1.0e16, 1.0, -1.0e16, 3.0, -2.0, 0.25];
        let mut sum = FiniteNeumaierSum::default();
        let mut previous = [0.0_f64; 2];
        for value in values {
            assert_eq!(sum.add(value), previous_add(&mut previous, value));
        }
        assert_eq!(
            sum.total().unwrap().to_bits(),
            (previous[0] + previous[1]).to_bits()
        );
    }

    #[test]
    fn rejects_each_nonfinite_input_without_mutating_the_total() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut sum = FiniteNeumaierSum::default();
            let mut previous = [0.0_f64; 2];
            sum.add(2.0).unwrap();
            previous_add(&mut previous, 2.0).unwrap();
            assert_eq!(sum.add(value), previous_add(&mut previous, value));
            assert_eq!(sum.total().unwrap().to_bits(), 2.0_f64.to_bits());
        }
    }

    #[test]
    fn distinguishes_running_sum_and_final_total_overflow() {
        let mut running = FiniteNeumaierSum::default();
        let mut previous = [0.0_f64; 2];
        running.add(f64::MAX).unwrap();
        previous_add(&mut previous, f64::MAX).unwrap();
        assert_eq!(running.add(f64::MAX), previous_add(&mut previous, f64::MAX));

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
