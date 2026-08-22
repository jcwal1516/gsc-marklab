use crate::spectra::kgrid::KMode;

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

pub(super) fn shell_mean_k(modes: &[KMode], shell_index: usize) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0usize;
    for mode in modes {
        if mode.shell_index == shell_index {
            sum += mode.k;
            count += 1;
        }
    }
    (count > 0).then_some(sum / count as f64)
}

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
