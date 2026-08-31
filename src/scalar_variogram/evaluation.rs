use super::{ScalarVariogramBin, ScalarVariogramError, ScalarVariogramRow};

pub(super) fn evaluate_curve(
    x: &[f64],
    y: &[f64],
    values: &[f64],
    bins: &[ScalarVariogramBin],
) -> Result<Vec<ScalarVariogramRow>, ScalarVariogramError> {
    if x.len() != y.len() || x.len() != values.len() {
        return Err(ScalarVariogramError::RowCountMismatch {
            expected: x.len(),
            observed: values.len(),
        });
    }
    let mut counts = vec![0_usize; bins.len()];
    let mut sums = vec![CompensatedSum::default(); bins.len()];
    for left in 0..x.len() {
        for right in (left + 1)..x.len() {
            let distance = (x[left] - x[right]).hypot(y[left] - y[right]);
            if let Some(bin) = find_bin(distance, bins) {
                let difference = values[left] - values[right];
                let contribution = 0.5 * difference * difference;
                if !contribution.is_finite() {
                    return Err(ScalarVariogramError::NumericalFailure);
                }
                counts[bin] = counts[bin]
                    .checked_add(1)
                    .ok_or(ScalarVariogramError::SizeOverflow)?;
                sums[bin].add(contribution);
            }
        }
    }
    bins.iter()
        .enumerate()
        .map(|(index, bin)| {
            let semivariance = if counts[index] == 0 {
                None
            } else {
                let value = sums[index].total() / counts[index] as f64;
                if !value.is_finite() {
                    return Err(ScalarVariogramError::NumericalFailure);
                }
                Some(value)
            };
            Ok(ScalarVariogramRow {
                lower_um: bin.lower_um,
                upper_um: bin.upper_um,
                upper_inclusive: index + 1 == bins.len(),
                pair_count: counts[index],
                semivariance,
            })
        })
        .collect()
}

fn find_bin(distance: f64, bins: &[ScalarVariogramBin]) -> Option<usize> {
    bins.iter().enumerate().position(|(index, bin)| {
        distance >= bin.lower_um
            && (distance < bin.upper_um || (index + 1 == bins.len() && distance <= bin.upper_um))
    })
}

#[derive(Clone, Copy, Default)]
struct CompensatedSum {
    sum: f64,
    correction: f64,
}

impl CompensatedSum {
    fn add(&mut self, value: f64) {
        let adjusted = value - self.correction;
        let next = self.sum + adjusted;
        self.correction = (next - self.sum) - adjusted;
        self.sum = next;
    }

    fn total(self) -> f64 {
        self.sum
    }
}
