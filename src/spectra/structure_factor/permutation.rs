use crate::{
    common::matrix::F64Matrix,
    common::seeds::{derive_seed, SeedEndpoint},
    data::Pattern,
    errors::{MarklabError, Result},
    memory_budget::spectrum_phase_cache_modes,
    permutation::{labels::deterministic_shuffle, stratified::StratifiedPermutationPlan},
    spectra::kgrid::KMode,
};

use super::{
    kernel::{
        centered_structure_factor_for_values, observed_power_for_modes,
        observed_value_power_for_modes, permutation_selected_indices_into,
        power_for_selected_modes_into, selected_indices_for_marks_into, total_phase_sums_for_modes,
        BinaryMarkContext, PhaseCache, PhaseSum,
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

#[derive(Clone, Copy)]
enum MarkField<'a> {
    Binary(BinaryMarkContext),
    Continuous(&'a [f64]),
    Stratified {
        context: BinaryMarkContext,
        plan: &'a StratifiedPermutationPlan,
    },
}

enum MarkFieldScratch {
    Binary {
        selected_indices: Vec<usize>,
        powers: Vec<f64>,
    },
    Continuous {
        permuted_values: Vec<f64>,
        powers: Vec<f64>,
    },
    Stratified {
        labels: Vec<u8>,
        stratum_labels: Vec<u8>,
        selected_indices: Vec<usize>,
        powers: Vec<f64>,
    },
}

#[derive(Clone, Copy)]
struct PreparedModeChunk<'a> {
    modes: &'a [KMode],
    totals: &'a [PhaseSum],
    phases: Option<&'a PhaseCache>,
}

impl MarkFieldScratch {
    fn powers(&self) -> &[f64] {
        match self {
            Self::Binary { powers, .. }
            | Self::Continuous { powers, .. }
            | Self::Stratified { powers, .. } => powers,
        }
    }
}

impl MarkField<'_> {
    fn benefits_from_phase_cache(self, point_count: usize, permutations: usize) -> bool {
        let selected = match self {
            Self::Binary(context) | Self::Stratified { context, .. } => context.selected_count(),
            Self::Continuous(_) => point_count,
        };
        // Do not precompute all cells for a tiny number of sparse subset visits.
        selected.saturating_mul(permutations) > point_count.saturating_mul(2)
    }

    fn scratch(self, cell_count: usize, chunk_size: usize) -> MarkFieldScratch {
        match self {
            Self::Binary(_) => MarkFieldScratch::Binary {
                selected_indices: Vec::with_capacity(cell_count),
                powers: Vec::with_capacity(chunk_size),
            },
            Self::Continuous(_) => MarkFieldScratch::Continuous {
                permuted_values: Vec::with_capacity(cell_count),
                powers: Vec::with_capacity(chunk_size),
            },
            Self::Stratified { plan, .. } => MarkFieldScratch::Stratified {
                labels: Vec::with_capacity(cell_count),
                stratum_labels: Vec::with_capacity(plan.maximum_stratum_size()),
                selected_indices: Vec::with_capacity(cell_count),
                powers: Vec::with_capacity(chunk_size),
            },
        }
    }

    fn prepare_mode_chunk(
        self,
        pattern: &Pattern,
        mode_chunk: &[KMode],
        total_phase_sums: &mut Vec<super::kernel::PhaseSum>,
        phases: Option<&PhaseCache>,
    ) -> Option<()> {
        match self {
            Self::Binary(_) | Self::Stratified { .. } => {
                if let Some(phases) = phases {
                    phases.total_phase_sums_into(total_phase_sums);
                    Some(())
                } else {
                    total_phase_sums_for_modes(pattern, mode_chunk, total_phase_sums)
                }
            }
            Self::Continuous(_) => Some(()),
        }
    }

    fn evaluate_permutation_chunk(
        self,
        pattern: &Pattern,
        chunk: PreparedModeChunk<'_>,
        base_seed: u64,
        permutation_index: usize,
        scratch: &mut MarkFieldScratch,
    ) -> Option<()> {
        match (self, scratch) {
            (
                Self::Binary(context),
                MarkFieldScratch::Binary {
                    selected_indices,
                    powers,
                },
            ) => {
                permutation_selected_indices_into(
                    pattern.len(),
                    context,
                    derive_seed(base_seed, SeedEndpoint::SpectrumBinary, permutation_index),
                    selected_indices,
                )?;
                if let Some(phases) = chunk.phases {
                    phases.binary_powers_into(selected_indices, context, chunk.totals, powers)?;
                } else {
                    power_for_selected_modes_into(
                        pattern,
                        chunk.modes,
                        chunk.totals,
                        selected_indices,
                        context,
                        powers,
                    )?;
                }
                Some(())
            }
            (
                Self::Continuous(values),
                MarkFieldScratch::Continuous {
                    permuted_values,
                    powers,
                },
            ) => {
                permuted_values.clear();
                permuted_values.extend_from_slice(values);
                deterministic_shuffle(
                    permuted_values,
                    derive_seed(
                        base_seed,
                        SeedEndpoint::SpectrumContinuous,
                        permutation_index,
                    ),
                );
                if let Some(phases) = chunk.phases {
                    phases.value_powers_into(permuted_values, powers)?;
                } else {
                    powers.clear();
                    for mode in chunk.modes {
                        powers.push(centered_structure_factor_for_values(
                            pattern,
                            permuted_values,
                            mode.kx,
                            mode.ky,
                        )?);
                    }
                }
                Some(())
            }
            (
                Self::Stratified { context, plan },
                MarkFieldScratch::Stratified {
                    labels,
                    stratum_labels,
                    selected_indices,
                    powers,
                },
            ) => {
                plan.permute_into(
                    derive_seed(
                        base_seed,
                        SeedEndpoint::SpectrumStratified,
                        permutation_index,
                    ),
                    labels,
                    stratum_labels,
                )
                .ok()?;
                selected_indices_for_marks_into(
                    labels,
                    context.use_unmarked_subset(),
                    selected_indices,
                )?;
                if let Some(phases) = chunk.phases {
                    phases.binary_powers_into(selected_indices, context, chunk.totals, powers)?;
                } else {
                    power_for_selected_modes_into(
                        pattern,
                        chunk.modes,
                        chunk.totals,
                        selected_indices,
                        context,
                        powers,
                    )?;
                }
                Some(())
            }
            _ => None,
        }
    }

    fn evaluation_error(self) -> &'static str {
        match self {
            Self::Binary(_) => "a required spectrum permutation could not be evaluated",
            Self::Continuous(_) => {
                "a required probabilistic-mark spectrum permutation could not be evaluated"
            }
            Self::Stratified { .. } => {
                "a required stratified spectrum permutation could not be evaluated"
            }
        }
    }
}

fn permutation_shell_powers(
    pattern: &Pattern,
    modes: &[KMode],
    shell_plan: &ShellPlan,
    mark_field: MarkField<'_>,
    options: SpectrumPermutationOptions,
) -> Result<F64Matrix> {
    let mut shell_powers = F64Matrix::zeros(options.n_permutations, shell_plan.shell_count())
        .ok_or_else(|| MarklabError::Compute("invalid spectrum shell storage dimensions".into()))?;
    let requested_chunk_capacity = options.k_chunk_modes.min(modes.len());
    let cached_modes = spectrum_phase_cache_modes(pattern.len(), requested_chunk_capacity);
    let use_cache = cached_modes > 0
        && mark_field.benefits_from_phase_cache(pattern.len(), options.n_permutations);
    let chunk_capacity = if use_cache {
        cached_modes
    } else {
        requested_chunk_capacity
    };
    let mut phase_cache = use_cache.then(|| PhaseCache::new(pattern.len(), chunk_capacity));
    let mut total_phase_sums = Vec::with_capacity(chunk_capacity);
    #[cfg(not(feature = "parallel"))]
    let mut scratch = mark_field.scratch(pattern.len(), chunk_capacity);
    let mut mode_offset = 0usize;

    for mode_chunk in modes.chunks(chunk_capacity) {
        if let Some(phases) = &mut phase_cache {
            phases.prepare(pattern, mode_chunk).ok_or_else(|| {
                MarklabError::Compute("spectrum phase cache could not be prepared".into())
            })?;
        }
        if mark_field
            .prepare_mode_chunk(
                pattern,
                mode_chunk,
                &mut total_phase_sums,
                phase_cache.as_ref(),
            )
            .is_none()
        {
            return Err(MarklabError::Compute(
                "spectrum mode chunk could not be prepared".into(),
            ));
        }
        observe_chunk_and_storage(mode_chunk.len(), &shell_powers, modes.len());
        let prepared = PreparedModeChunk {
            modes: mode_chunk,
            totals: &total_phase_sums,
            phases: phase_cache.as_ref(),
        };

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;

            let all_permutations_valid = shell_powers
                .values_mut()
                .par_chunks_mut(shell_plan.shell_count())
                .enumerate()
                .map_init(
                    || mark_field.scratch(pattern.len(), mode_chunk.len()),
                    |scratch, (permutation_index, shell_sums)| {
                        mark_field
                            .evaluate_permutation_chunk(
                                pattern,
                                prepared,
                                options.seed,
                                permutation_index,
                                scratch,
                            )
                            .and_then(|()| {
                                shell_plan.accumulate_mode_chunk(
                                    shell_sums,
                                    mode_offset,
                                    scratch.powers(),
                                )
                            })
                            .is_some()
                    },
                )
                .all(|valid| valid);
            if !all_permutations_valid {
                return Err(MarklabError::Compute(mark_field.evaluation_error().into()));
            }
        }

        #[cfg(not(feature = "parallel"))]
        for (permutation_index, shell_sums) in shell_powers.iter_rows_mut().enumerate() {
            mark_field
                .evaluate_permutation_chunk(
                    pattern,
                    prepared,
                    options.seed,
                    permutation_index,
                    &mut scratch,
                )
                .and_then(|()| {
                    shell_plan.accumulate_mode_chunk(shell_sums, mode_offset, scratch.powers())
                })
                .ok_or_else(|| {
                    MarklabError::Compute(format!(
                        "{} at index {permutation_index}",
                        mark_field.evaluation_error()
                    ))
                })?;
        }

        mode_offset += mode_chunk.len();
    }
    shell_plan
        .normalize_matrix(&mut shell_powers)
        .ok_or_else(|| MarklabError::Compute("invalid spectrum shell aggregation".into()))?;
    Ok(shell_powers)
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
    let permutation_shell_powers = permutation_shell_powers(
        pattern,
        modes,
        &shell_plan,
        MarkField::Binary(context),
        options,
    )?;

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
    let permutation_shell_powers = permutation_shell_powers(
        pattern,
        modes,
        &shell_plan,
        MarkField::Continuous(values),
        options,
    )?;

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
    let permutation_shell_powers = permutation_shell_powers(
        pattern,
        modes,
        &shell_plan,
        MarkField::Stratified {
            context,
            plan: &stratified_plan,
        },
        options,
    )?;

    summarize_permutation_whitening_from_shells(
        &shell_plan,
        observed_shell_power,
        permutation_shell_powers,
        modes.len(),
        options,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PatternMeta;

    #[test]
    fn permutation_shells_match_direct_phase_evaluation_for_every_mark_field() {
        let n = 11;
        let mut pattern = Pattern::from_arrays(
            (0..n).map(|i| i as f64 * 0.73).collect(),
            (0..n).map(|i| (i % 4) as f64 * 1.17).collect(),
            vec![0; n],
            PatternMeta {
                case_id: "phase-cache".into(),
                timepoint: "post".into(),
                protein: "mark".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .unwrap();
        let modes = [(0.3_f64, 0.7_f64), (-1.1, 0.9), (2.3, -0.4), (-0.1, -3.2)]
            .into_iter()
            .enumerate()
            .map(|(index, (kx, ky))| KMode {
                kx,
                ky,
                k: kx.hypot(ky),
                shell_index: index / 2,
            })
            .collect::<Vec<_>>();
        let values = (0..n).map(|i| (i % 5) as f64 - 1.75).collect::<Vec<_>>();
        let strata = (0..n).map(|i| (i % 3) as u8).collect::<Vec<_>>();
        let shell_plan = ShellPlan::new(&modes, 2).unwrap();
        for marked in [1, n / 2, n - 3, n - 1] {
            pattern.mark = (0..n).map(|i| u8::from(i < marked)).collect();
            let context = BinaryMarkContext::new(n, marked).unwrap();
            let plan = StratifiedPermutationPlan::new(&pattern.mark, &strata).unwrap();
            for field in [
                MarkField::Binary(context),
                MarkField::Continuous(&values),
                MarkField::Stratified {
                    context,
                    plan: &plan,
                },
            ] {
                let options = SpectrumPermutationOptions {
                    n_shells: 2,
                    low_k_modes: 1,
                    fit_low_k_alpha: true,
                    n_permutations: 19,
                    seed: 123,
                    family_wise_alpha: 0.1,
                    max_scale_um: f64::INFINITY,
                    k_shell_min: 1,
                    k_chunk_modes: 3,
                };
                let mut totals = Vec::new();
                field
                    .prepare_mode_chunk(&pattern, &modes, &mut totals, None)
                    .unwrap();
                let mut scratch = field.scratch(n, modes.len());
                let mut expected = F64Matrix::zeros(options.n_permutations, 2).unwrap();
                for (permutation, row) in expected.iter_rows_mut().enumerate() {
                    field
                        .evaluate_permutation_chunk(
                            &pattern,
                            PreparedModeChunk {
                                modes: &modes,
                                totals: &totals,
                                phases: None,
                            },
                            options.seed,
                            permutation,
                            &mut scratch,
                        )
                        .unwrap();
                    row.copy_from_slice(
                        &shell_plan.aggregate_mode_powers(scratch.powers()).unwrap(),
                    );
                }
                for chunk in [1, 3, modes.len()] {
                    let actual = permutation_shell_powers(
                        &pattern,
                        &modes,
                        &shell_plan,
                        field,
                        SpectrumPermutationOptions {
                            k_chunk_modes: chunk,
                            ..options
                        },
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "marked={marked}, chunk={chunk}");
                }
            }
        }
    }
}
