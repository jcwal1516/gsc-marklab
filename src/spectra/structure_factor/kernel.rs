use crate::{
    data::Pattern, permutation::labels::permute_fixed_count_indices_into, spectra::kgrid::KMode,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseSum {
    pub re: f64,
    pub im: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BinaryMarkContext {
    n_marked: usize,
    use_unmarked_subset: bool,
    selected_count: usize,
}

impl BinaryMarkContext {
    pub(super) fn new(n_cells: usize, n_marked: usize) -> Option<Self> {
        if n_cells < 2 || n_marked == 0 || n_marked >= n_cells {
            return None;
        }
        let n_unmarked = n_cells - n_marked;
        let use_unmarked_subset = n_marked > n_unmarked;
        Some(Self {
            n_marked,
            use_unmarked_subset,
            selected_count: if use_unmarked_subset {
                n_unmarked
            } else {
                n_marked
            },
        })
    }

    pub(super) fn use_unmarked_subset(self) -> bool {
        self.use_unmarked_subset
    }
}

pub fn centered_structure_factor(pattern: &Pattern, kx: f64, ky: f64) -> Option<f64> {
    if pattern.is_empty() || !kx.is_finite() || !ky.is_finite() {
        return None;
    }

    let p_hat = pattern.p_hat();
    let mut re = 0.0;
    let mut im = 0.0;
    for ((x, y), mark) in pattern
        .x_um
        .iter()
        .copied()
        .zip(pattern.y_um.iter().copied())
        .zip(pattern.mark.iter().copied())
    {
        let centered = f64::from(mark) - p_hat;
        let phase = -(kx * x + ky * y);
        re += centered * phase.cos();
        im += centered * phase.sin();
    }
    Some((re * re + im * im) / pattern.len() as f64)
}

pub fn observed_power_for_modes(pattern: &Pattern, modes: &[KMode]) -> Vec<f64> {
    let mut selected_indices = Vec::with_capacity(pattern.len());
    let mut powers = Vec::with_capacity(modes.len());
    observed_power_for_modes_into(pattern, modes, &mut selected_indices, &mut powers);
    powers
}

pub fn observed_power_for_modes_into(
    pattern: &Pattern,
    modes: &[KMode],
    selected_indices: &mut Vec<usize>,
    powers: &mut Vec<f64>,
) -> Option<()> {
    let context = BinaryMarkContext::new(pattern.len(), pattern.n_marked())?;
    selected_indices.clear();
    selected_indices_for_marks_into(&pattern.mark, context.use_unmarked_subset, selected_indices)?;

    powers.clear();
    powers.resize(modes.len(), 0.0);
    for (mode_index, mode) in modes.iter().copied().enumerate() {
        let total = total_phase_sum(pattern, mode.kx, mode.ky)?;
        powers[mode_index] = centered_structure_factor_for_index_subset(
            pattern,
            selected_indices,
            context.n_marked,
            context.use_unmarked_subset,
            total,
            mode.kx,
            mode.ky,
        )?;
    }
    Some(())
}

pub fn total_phase_sums_for_modes(
    pattern: &Pattern,
    modes: &[KMode],
    sums: &mut Vec<PhaseSum>,
) -> Option<()> {
    sums.clear();
    sums.reserve(modes.len());
    for mode in modes {
        sums.push(total_phase_sum(pattern, mode.kx, mode.ky)?);
    }
    Some(())
}

#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
pub fn permutation_power_for_modes_into(
    pattern: &Pattern,
    modes: &[KMode],
    total_phase_sums: &[PhaseSum],
    seed: u64,
    selected_indices: &mut Vec<usize>,
    powers: &mut Vec<f64>,
) -> Option<()> {
    if modes.len() != total_phase_sums.len() {
        return None;
    }
    let context = BinaryMarkContext::new(pattern.len(), pattern.n_marked())?;
    permutation_selected_indices_into(pattern.len(), context, seed, selected_indices)?;

    power_for_selected_modes_into(
        pattern,
        modes,
        total_phase_sums,
        selected_indices,
        context,
        powers,
    )
}

pub(super) fn permutation_selected_indices_into(
    n_cells: usize,
    context: BinaryMarkContext,
    seed: u64,
    selected_indices: &mut Vec<usize>,
) -> Option<()> {
    if n_cells < context.selected_count {
        return None;
    }
    permute_fixed_count_indices_into(n_cells, context.selected_count, seed, selected_indices).ok()
}

pub(super) fn selected_indices_for_marks_into(
    marks: &[u8],
    use_unmarked_subset: bool,
    selected_indices: &mut Vec<usize>,
) -> Option<()> {
    if marks.iter().any(|mark| *mark != 0 && *mark != 1) {
        return None;
    }
    selected_indices.clear();
    selected_indices.extend(
        marks
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, mark)| {
                if use_unmarked_subset {
                    (mark == 0).then_some(index)
                } else {
                    (mark == 1).then_some(index)
                }
            }),
    );
    Some(())
}

pub(super) fn power_for_selected_modes_into(
    pattern: &Pattern,
    modes: &[KMode],
    total_phase_sums: &[PhaseSum],
    selected_indices: &[usize],
    context: BinaryMarkContext,
    powers: &mut Vec<f64>,
) -> Option<()> {
    if modes.len() != total_phase_sums.len() || pattern.len() < context.n_marked {
        return None;
    }

    powers.clear();
    powers.resize(modes.len(), 0.0);
    for (mode_index, mode) in modes.iter().copied().enumerate() {
        powers[mode_index] = centered_structure_factor_for_index_subset(
            pattern,
            selected_indices,
            context.n_marked,
            context.use_unmarked_subset,
            total_phase_sums[mode_index],
            mode.kx,
            mode.ky,
        )?;
    }
    Some(())
}

pub fn observed_value_power_for_modes(
    pattern: &Pattern,
    values: &[f64],
    modes: &[KMode],
) -> Option<Vec<f64>> {
    modes
        .iter()
        .map(|mode| centered_structure_factor_for_values(pattern, values, mode.kx, mode.ky))
        .collect()
}

pub fn centered_structure_factor_for_marks(
    pattern: &Pattern,
    marks: &[u8],
    kx: f64,
    ky: f64,
) -> Option<f64> {
    if pattern.len() != marks.len()
        || marks.is_empty()
        || marks.iter().any(|mark| *mark != 0 && *mark != 1)
        || !kx.is_finite()
        || !ky.is_finite()
    {
        return None;
    }
    let n_marked = marks.iter().filter(|mark| **mark == 1).count();
    let use_unmarked_subset = n_marked > marks.len().saturating_sub(n_marked);
    let selected_indices = marks
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, mark)| {
            if use_unmarked_subset {
                (mark == 0).then_some(index)
            } else {
                (mark == 1).then_some(index)
            }
        })
        .collect::<Vec<_>>();
    let total = total_phase_sum(pattern, kx, ky)?;
    centered_structure_factor_for_index_subset(
        pattern,
        &selected_indices,
        n_marked,
        use_unmarked_subset,
        total,
        kx,
        ky,
    )
}

pub fn centered_structure_factor_for_values(
    pattern: &Pattern,
    values: &[f64],
    kx: f64,
    ky: f64,
) -> Option<f64> {
    if pattern.len() != values.len()
        || values.is_empty()
        || values.iter().any(|value| !value.is_finite())
        || !kx.is_finite()
        || !ky.is_finite()
    {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let mut re = 0.0;
    let mut im = 0.0;
    for ((x, y), value) in pattern
        .x_um
        .iter()
        .copied()
        .zip(pattern.y_um.iter().copied())
        .zip(values.iter().copied())
    {
        let centered = value - mean;
        let phase = -(kx * x + ky * y);
        re += centered * phase.cos();
        im += centered * phase.sin();
    }
    Some((re * re + im * im) / values.len() as f64)
}

pub(super) fn centered_structure_factor_for_index_subset(
    pattern: &Pattern,
    selected_indices: &[usize],
    n_marked: usize,
    selected_are_unmarked: bool,
    total: PhaseSum,
    kx: f64,
    ky: f64,
) -> Option<f64> {
    if pattern.is_empty()
        || n_marked > pattern.len()
        || selected_indices.iter().any(|index| *index >= pattern.len())
        || !kx.is_finite()
        || !ky.is_finite()
    {
        return None;
    }
    let selected = selected_phase_sum(pattern, selected_indices, kx, ky)?;
    let p_hat = n_marked as f64 / pattern.len() as f64;
    let (re, im) = if selected_are_unmarked {
        (
            (1.0 - p_hat) * total.re - selected.re,
            (1.0 - p_hat) * total.im - selected.im,
        )
    } else {
        (
            selected.re - p_hat * total.re,
            selected.im - p_hat * total.im,
        )
    };
    Some((re * re + im * im) / pattern.len() as f64)
}

pub(super) fn total_phase_sum(pattern: &Pattern, kx: f64, ky: f64) -> Option<PhaseSum> {
    if pattern.is_empty() || !kx.is_finite() || !ky.is_finite() {
        return None;
    }
    let mut sum = PhaseSum { re: 0.0, im: 0.0 };
    for (x, y) in pattern
        .x_um
        .iter()
        .copied()
        .zip(pattern.y_um.iter().copied())
    {
        let phase = -(kx * x + ky * y);
        sum.re += phase.cos();
        sum.im += phase.sin();
    }
    Some(sum)
}

fn selected_phase_sum(
    pattern: &Pattern,
    selected_indices: &[usize],
    kx: f64,
    ky: f64,
) -> Option<PhaseSum> {
    if selected_indices.iter().any(|index| *index >= pattern.len())
        || !kx.is_finite()
        || !ky.is_finite()
    {
        return None;
    }
    let mut sum = PhaseSum { re: 0.0, im: 0.0 };
    for index in selected_indices {
        let phase = -(kx * pattern.x_um[*index] + ky * pattern.y_um[*index]);
        sum.re += phase.cos();
        sum.im += phase.sin();
    }
    Some(sum)
}
