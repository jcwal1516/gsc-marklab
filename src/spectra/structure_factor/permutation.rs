use crate::{
    common::matrix::F64Matrix,
    common::seeds::{derive_seed, SeedEndpoint},
    data::Pattern,
    errors::{MarklabError, Result},
    permutation::{labels::deterministic_shuffle, stratified::StratifiedPermutationPlan},
    spectra::kgrid::KMode,
};

use super::{
    kernel::{
        centered_structure_factor_for_values, observed_power_for_modes,
        observed_value_power_for_modes, permutation_selected_indices_into,
        power_for_selected_modes_into, selected_indices_for_marks_into, total_phase_sums_for_modes,
        BinaryMarkContext,
    },
    modes::resolvable_modes_for_pattern,
    shells::ShellPlan,
    summaries::summarize_permutation_whitening_from_shells,
    PermutationWhitenedSpectrum, SpectrumPermutationOptions,
};

#[cfg(test)]
thread_local! {
    static LARGEST_MODE_CHUNK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LAST_SHELL_STORAGE: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
}

#[cfg(test)]
pub(super) fn reset_chunk_observation() {
    LARGEST_MODE_CHUNK.set(0);
    LAST_SHELL_STORAGE.set((0, 0, 0));
}

#[cfg(test)]
pub(super) fn largest_mode_chunk_observed() -> usize {
    LARGEST_MODE_CHUNK.get()
}

#[cfg(test)]
pub(super) fn last_shell_storage_dimensions() -> (usize, usize, usize) {
    LAST_SHELL_STORAGE.get()
}

fn observe_chunk_and_storage(chunk_len: usize, matrix: &F64Matrix, mode_count: usize) {
    #[cfg(not(test))]
    let _ = (chunk_len, matrix, mode_count);
    #[cfg(test)]
    {
        LARGEST_MODE_CHUNK.set(LARGEST_MODE_CHUNK.get().max(chunk_len));
        LAST_SHELL_STORAGE.set((matrix.row_count(), matrix.column_count(), mode_count));
    }
}

pub fn permutation_whitened_spectrum(
    pattern: &Pattern,
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>> {
    if pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || options.n_shells == 0
        || options.n_permutations == 0
        || options.k_chunk_modes == 0
    {
        return Ok(None);
    }

    let Some(modes) = resolvable_modes_for_pattern(pattern, options.n_shells) else {
        return Ok(None);
    };
    let observed_mode_power = observed_power_for_modes(pattern, &modes);

    permutation_whitened_spectrum_from_observed_modes(pattern, &modes, observed_mode_power, options)
}

pub fn permutation_whitened_value_spectrum(
    pattern: &Pattern,
    values: &[f64],
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>> {
    if pattern.len() != values.len()
        || pattern.len() < 2
        || values.iter().any(|value| !value.is_finite())
        || options.n_shells == 0
        || options.n_permutations == 0
        || options.k_chunk_modes == 0
    {
        return Ok(None);
    }

    let Some(modes) = resolvable_modes_for_pattern(pattern, options.n_shells) else {
        return Ok(None);
    };
    let Some(observed_mode_power) = observed_value_power_for_modes(pattern, values, &modes) else {
        return Ok(None);
    };

    permutation_whitened_value_spectrum_from_observed_modes(
        pattern,
        values,
        &modes,
        observed_mode_power,
        options,
    )
}

pub fn permutation_whitened_spectrum_from_observed_modes(
    pattern: &Pattern,
    modes: &[KMode],
    observed_mode_power: Vec<f64>,
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>> {
    if pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || options.n_shells == 0
        || options.n_permutations == 0
        || options.k_chunk_modes == 0
        || modes.is_empty()
        || observed_mode_power.len() != modes.len()
    {
        return Ok(None);
    }

    let Some(context) = BinaryMarkContext::new(pattern.len(), pattern.n_marked()) else {
        return Ok(None);
    };
    let Some(shell_plan) = ShellPlan::new(modes, options.n_shells) else {
        return Ok(None);
    };
    let Some(observed_shell_power) = shell_plan.aggregate_mode_powers(&observed_mode_power) else {
        return Ok(None);
    };
    let mut permutation_shell_powers =
        F64Matrix::zeros(options.n_permutations, shell_plan.shell_count()).ok_or_else(|| {
            MarklabError::Compute("invalid spectrum shell storage dimensions".into())
        })?;
    let mut total_phase_sums = Vec::with_capacity(options.k_chunk_modes.min(modes.len()));
    let mut mode_offset = 0usize;
    for mode_chunk in modes.chunks(options.k_chunk_modes) {
        if total_phase_sums_for_modes(pattern, mode_chunk, &mut total_phase_sums).is_none() {
            return Ok(None);
        }
        observe_chunk_and_storage(mode_chunk.len(), &permutation_shell_powers, modes.len());

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;

            let all_permutations_valid = permutation_shell_powers
                .values_mut()
                .par_chunks_mut(shell_plan.shell_count())
                .enumerate()
                .map_init(
                    || {
                        (
                            Vec::with_capacity(pattern.len()),
                            Vec::with_capacity(mode_chunk.len()),
                        )
                    },
                    |(selected_indices, powers), (permutation_index, shell_sums)| {
                        let seed = derive_seed(
                            options.seed,
                            SeedEndpoint::SpectrumBinary,
                            permutation_index,
                        );
                        permutation_selected_indices_into(
                            pattern.len(),
                            context,
                            seed,
                            selected_indices,
                        )
                        .and_then(|()| {
                            power_for_selected_modes_into(
                                pattern,
                                mode_chunk,
                                &total_phase_sums,
                                selected_indices,
                                context,
                                powers,
                            )
                        })
                        .and_then(|()| {
                            shell_plan.accumulate_mode_chunk(shell_sums, mode_offset, powers)
                        })
                        .is_some()
                    },
                )
                .all(|valid| valid);
            if !all_permutations_valid {
                return Err(MarklabError::Compute(
                    "a required spectrum permutation could not be evaluated".into(),
                ));
            }
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut selected_indices = Vec::with_capacity(pattern.len());
            let mut powers = Vec::with_capacity(mode_chunk.len());
            for (permutation_index, shell_sums) in
                permutation_shell_powers.iter_rows_mut().enumerate()
            {
                let seed = derive_seed(
                    options.seed,
                    SeedEndpoint::SpectrumBinary,
                    permutation_index,
                );
                let valid = permutation_selected_indices_into(
                    pattern.len(),
                    context,
                    seed,
                    &mut selected_indices,
                )
                .and_then(|()| {
                    power_for_selected_modes_into(
                        pattern,
                        mode_chunk,
                        &total_phase_sums,
                        &selected_indices,
                        context,
                        &mut powers,
                    )
                })
                .and_then(|()| shell_plan.accumulate_mode_chunk(shell_sums, mode_offset, &powers));
                if valid.is_none() {
                    return Err(MarklabError::Compute(format!(
                        "spectrum permutation {permutation_index} could not be evaluated"
                    )));
                }
            }
        }

        mode_offset += mode_chunk.len();
    }
    shell_plan
        .normalize_matrix(&mut permutation_shell_powers)
        .ok_or_else(|| MarklabError::Compute("invalid spectrum shell aggregation".into()))?;

    summarize_permutation_whitening_from_shells(
        &shell_plan,
        observed_shell_power,
        permutation_shell_powers,
        modes.len(),
        options,
    )
}

pub fn permutation_whitened_value_spectrum_from_observed_modes(
    pattern: &Pattern,
    values: &[f64],
    modes: &[KMode],
    observed_mode_power: Vec<f64>,
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>> {
    if pattern.len() != values.len()
        || pattern.len() < 2
        || values.iter().any(|value| !value.is_finite())
        || options.n_shells == 0
        || options.n_permutations == 0
        || options.k_chunk_modes == 0
        || modes.is_empty()
        || observed_mode_power.len() != modes.len()
    {
        return Ok(None);
    }

    let Some(shell_plan) = ShellPlan::new(modes, options.n_shells) else {
        return Ok(None);
    };
    let Some(observed_shell_power) = shell_plan.aggregate_mode_powers(&observed_mode_power) else {
        return Ok(None);
    };
    let mut permutation_shell_powers =
        F64Matrix::zeros(options.n_permutations, shell_plan.shell_count()).ok_or_else(|| {
            MarklabError::Compute("invalid probabilistic-mark shell storage dimensions".into())
        })?;
    let mut mode_offset = 0usize;
    for mode_chunk in modes.chunks(options.k_chunk_modes) {
        observe_chunk_and_storage(mode_chunk.len(), &permutation_shell_powers, modes.len());

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;

            let all_permutations_valid = permutation_shell_powers
                .values_mut()
                .par_chunks_mut(shell_plan.shell_count())
                .enumerate()
                .map_init(
                    || {
                        (
                            Vec::with_capacity(values.len()),
                            Vec::with_capacity(mode_chunk.len()),
                        )
                    },
                    |(permuted, powers), (permutation_index, shell_sums)| {
                        permuted.clear();
                        permuted.extend_from_slice(values);
                        deterministic_shuffle(
                            permuted,
                            derive_seed(
                                options.seed,
                                SeedEndpoint::SpectrumContinuous,
                                permutation_index,
                            ),
                        );
                        powers.clear();
                        for mode in mode_chunk {
                            let Some(power) = centered_structure_factor_for_values(
                                pattern, permuted, mode.kx, mode.ky,
                            ) else {
                                return false;
                            };
                            powers.push(power);
                        }
                        shell_plan
                            .accumulate_mode_chunk(shell_sums, mode_offset, powers)
                            .is_some()
                    },
                )
                .all(|valid| valid);
            if !all_permutations_valid {
                return Err(MarklabError::Compute(
                    "a required probabilistic-mark spectrum permutation could not be evaluated"
                        .into(),
                ));
            }
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut permuted = Vec::with_capacity(values.len());
            let mut powers = Vec::with_capacity(mode_chunk.len());
            for (permutation_index, shell_sums) in
                permutation_shell_powers.iter_rows_mut().enumerate()
            {
                permuted.clear();
                permuted.extend_from_slice(values);
                deterministic_shuffle(
                    &mut permuted,
                    derive_seed(
                        options.seed,
                        SeedEndpoint::SpectrumContinuous,
                        permutation_index,
                    ),
                );
                powers.clear();
                for mode in mode_chunk {
                    let Some(power) =
                        centered_structure_factor_for_values(pattern, &permuted, mode.kx, mode.ky)
                    else {
                        return Err(MarklabError::Compute(format!(
                            "probabilistic-mark spectrum permutation {permutation_index} could not be evaluated"
                        )));
                    };
                    powers.push(power);
                }
                if shell_plan
                    .accumulate_mode_chunk(shell_sums, mode_offset, &powers)
                    .is_none()
                {
                    return Err(MarklabError::Compute(format!(
                        "probabilistic-mark spectrum permutation {permutation_index} could not be aggregated"
                    )));
                }
            }
        }

        mode_offset += mode_chunk.len();
    }
    shell_plan
        .normalize_matrix(&mut permutation_shell_powers)
        .ok_or_else(|| {
            MarklabError::Compute("invalid probabilistic-mark shell aggregation".into())
        })?;

    summarize_permutation_whitening_from_shells(
        &shell_plan,
        observed_shell_power,
        permutation_shell_powers,
        modes.len(),
        options,
    )
}

pub fn stratified_permutation_whitened_spectrum_from_observed_modes<T>(
    pattern: &Pattern,
    strata: &[T],
    modes: &[KMode],
    observed_mode_power: Vec<f64>,
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>>
where
    T: Copy + Ord + Into<u64>,
{
    if pattern.len() != strata.len()
        || pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || options.n_shells == 0
        || options.n_permutations == 0
        || options.k_chunk_modes == 0
        || modes.is_empty()
        || observed_mode_power.len() != modes.len()
    {
        return Ok(None);
    }

    let Some(context) = BinaryMarkContext::new(pattern.len(), pattern.n_marked()) else {
        return Ok(None);
    };
    let stratified_plan = StratifiedPermutationPlan::new(&pattern.mark, strata)?;
    let Some(shell_plan) = ShellPlan::new(modes, options.n_shells) else {
        return Ok(None);
    };
    let Some(observed_shell_power) = shell_plan.aggregate_mode_powers(&observed_mode_power) else {
        return Ok(None);
    };
    let mut permutation_shell_powers =
        F64Matrix::zeros(options.n_permutations, shell_plan.shell_count()).ok_or_else(|| {
            MarklabError::Compute("invalid stratified spectrum shell storage dimensions".into())
        })?;
    let mut total_phase_sums = Vec::with_capacity(options.k_chunk_modes.min(modes.len()));
    let mut selected_indices = Vec::with_capacity(pattern.len());
    let mut labels = Vec::with_capacity(pattern.len());
    let mut stratum_labels = Vec::with_capacity(stratified_plan.maximum_stratum_size());
    let mut powers = Vec::with_capacity(options.k_chunk_modes.min(modes.len()));
    let mut mode_offset = 0usize;
    for mode_chunk in modes.chunks(options.k_chunk_modes) {
        if total_phase_sums_for_modes(pattern, mode_chunk, &mut total_phase_sums).is_none() {
            return Ok(None);
        }
        observe_chunk_and_storage(mode_chunk.len(), &permutation_shell_powers, modes.len());
        for (permutation_index, shell_sums) in permutation_shell_powers.iter_rows_mut().enumerate()
        {
            stratified_plan.permute_into(
                derive_seed(
                    options.seed,
                    SeedEndpoint::SpectrumStratified,
                    permutation_index,
                ),
                &mut labels,
                &mut stratum_labels,
            )?;
            selected_indices_for_marks_into(
                &labels,
                context.use_unmarked_subset(),
                &mut selected_indices,
            )
            .and_then(|()| {
                power_for_selected_modes_into(
                    pattern,
                    mode_chunk,
                    &total_phase_sums,
                    &selected_indices,
                    context,
                    &mut powers,
                )
            })
            .and_then(|()| shell_plan.accumulate_mode_chunk(shell_sums, mode_offset, &powers))
            .ok_or_else(|| {
                MarklabError::Compute(format!(
                    "stratified spectrum permutation {permutation_index} produced an undefined mode"
                ))
            })?;
        }
        mode_offset += mode_chunk.len();
    }
    shell_plan
        .normalize_matrix(&mut permutation_shell_powers)
        .ok_or_else(|| {
            MarklabError::Compute("invalid stratified spectrum shell aggregation".into())
        })?;

    summarize_permutation_whitening_from_shells(
        &shell_plan,
        observed_shell_power,
        permutation_shell_powers,
        modes.len(),
        options,
    )
}
