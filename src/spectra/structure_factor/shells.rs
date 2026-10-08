use crate::{common::matrix::F64Matrix, spectra::kgrid::KMode};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShellPlan {
    mode_to_position: Box<[usize]>,
    mode_counts: Box<[usize]>,
    k_values: Box<[f64]>,
}

impl ShellPlan {
    pub(super) fn new(modes: &[KMode], n_shells: usize) -> Option<Self> {
        if modes.is_empty() || n_shells == 0 {
            return None;
        }
        let shell_index = nonempty_shells(modes, n_shells);
        if shell_index.is_empty() {
            return None;
        }
        let mut shell_to_position = vec![usize::MAX; n_shells];
        for (position, shell) in shell_index.iter().copied().enumerate() {
            shell_to_position[shell] = position;
        }
        let mode_to_position = modes
            .iter()
            .map(|mode| shell_to_position.get(mode.shell_index).copied())
            .collect::<Option<Vec<_>>>()?;
        if mode_to_position.contains(&usize::MAX) {
            return None;
        }
        let mut mode_counts = vec![0usize; shell_index.len()];
        let mut k_sums = vec![0.0; shell_index.len()];
        for (mode, position) in modes.iter().zip(mode_to_position.iter().copied()) {
            if !mode.k.is_finite() {
                return None;
            }
            mode_counts[position] += 1;
            k_sums[position] += mode.k;
        }
        let k_values = k_sums
            .into_iter()
            .zip(mode_counts.iter().copied())
            .map(|(sum, count)| (count > 0).then_some(sum / count as f64))
            .collect::<Option<Vec<_>>>()?;

        Some(Self {
            mode_to_position: mode_to_position.into_boxed_slice(),
            mode_counts: mode_counts.into_boxed_slice(),
            k_values: k_values.into_boxed_slice(),
        })
    }

    pub(super) fn mode_count(&self) -> usize {
        self.mode_to_position.len()
    }

    pub(super) fn shell_count(&self) -> usize {
        self.mode_counts.len()
    }

    pub(super) fn k_values(&self) -> &[f64] {
        &self.k_values
    }

    pub(super) fn aggregate_mode_powers(&self, powers: &[f64]) -> Option<Vec<f64>> {
        if powers.len() != self.mode_count() || powers.iter().any(|power| !power.is_finite()) {
            return None;
        }
        let mut sums = vec![0.0; self.shell_count()];
        self.accumulate_mode_chunk(&mut sums, 0, powers)?;
        self.normalize_row(&mut sums)?;
        Some(sums)
    }

    pub(super) fn accumulate_mode_chunk(
        &self,
        shell_sums: &mut [f64],
        mode_offset: usize,
        powers: &[f64],
    ) -> Option<()> {
        if shell_sums.len() != self.shell_count()
            || mode_offset.checked_add(powers.len())? > self.mode_count()
            || powers.iter().any(|power| !power.is_finite())
        {
            return None;
        }
        for (mode_index, power) in (mode_offset..).zip(powers.iter().copied()) {
            shell_sums[*self.mode_to_position.get(mode_index)?] += power;
        }
        Some(())
    }

    pub(super) fn normalize_matrix(&self, matrix: &mut F64Matrix) -> Option<()> {
        if matrix.column_count() != self.shell_count() {
            return None;
        }
        for row in matrix.iter_rows_mut() {
            self.normalize_row(row)?;
        }
        Some(())
    }

    fn normalize_row(&self, row: &mut [f64]) -> Option<()> {
        if row.len() != self.shell_count() {
            return None;
        }
        for (value, count) in row.iter_mut().zip(self.mode_counts.iter().copied()) {
            if count == 0 {
                return None;
            }
            *value /= count as f64;
        }
        Some(())
    }
}

pub(super) fn nonempty_shells(modes: &[KMode], n_shells: usize) -> Vec<usize> {
    let mut counts = vec![0usize; n_shells];
    for mode in modes {
        if mode.shell_index < n_shells {
            counts[mode.shell_index] += 1;
        }
    }
    counts
        .into_iter()
        .enumerate()
        .filter_map(|(shell, count)| (count > 0).then_some(shell))
        .collect()
}

#[cfg(test)]
pub(super) fn shell_mean_powers(
    modes: &[KMode],
    powers: &[f64],
    shell_index: &[usize],
) -> Option<Vec<f64>> {
    if modes.len() != powers.len() {
        return None;
    }
    let mut output = Vec::with_capacity(shell_index.len());
    for shell in shell_index {
        let mut sum = 0.0;
        let mut count = 0usize;
        for (mode, power) in modes.iter().zip(powers.iter().copied()) {
            if mode.shell_index == *shell && power.is_finite() {
                sum += power;
                count += 1;
            }
        }
        output.push(if count == 0 { 0.0 } else { sum / count as f64 });
    }
    Some(output)
}
