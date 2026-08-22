#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnisotropyReadout {
    pub index: f64,
    pub theta_deg: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PermutationAnisotropy {
    pub readout: AnisotropyReadout,
    pub p_value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AnisotropyPermutationOptions {
    pub(crate) low_k_radius: usize,
    pub(crate) n_permutations: usize,
    pub(crate) seed: u64,
    pub(crate) alpha: f64,
    pub(crate) k_chunk_modes: usize,
    pub(crate) n_marked: usize,
}

#[cfg(test)]
pub fn anisotropy_from_weighted_modes(modes: &[(f64, f64, f64)]) -> Option<AnisotropyReadout> {
    let mut tensor = AnisotropyTensor::default();

    for (kx, ky, weight) in modes.iter().copied() {
        if !kx.is_finite() || !ky.is_finite() || !weight.is_finite() {
            continue;
        }
        tensor.accumulate_weight(kx, ky, weight.max(0.0));
    }

    tensor.readout()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct AnisotropyTensor {
    xx: f64,
    xy: f64,
    yy: f64,
}

impl AnisotropyTensor {
    fn accumulate_whitened(&mut self, kx: f64, ky: f64, power: f64, baseline: f64) {
        let whitened_excess = power / baseline.max(f64::EPSILON) - 1.0;
        self.accumulate_weight(kx, ky, whitened_excess.max(0.0));
    }

    fn accumulate_weight(&mut self, kx: f64, ky: f64, weight: f64) {
        self.xx += weight * kx * kx;
        self.xy += weight * kx * ky;
        self.yy += weight * ky * ky;
    }

    fn readout(self) -> Option<AnisotropyReadout> {
        let trace = self.xx + self.yy;
        if trace <= 0.0 {
            return None;
        }

        let delta = ((self.xx - self.yy) * (self.xx - self.yy) + 4.0 * self.xy * self.xy).sqrt();
        let lambda_max = 0.5 * (trace + delta);
        let lambda_min = 0.5 * (trace - delta);
        let index = if lambda_min <= f64::EPSILON {
            (lambda_max / f64::EPSILON).min(1.0e15)
        } else {
            lambda_max / lambda_min
        };
        let theta_rad = 0.5 * (2.0 * self.xy).atan2(self.xx - self.yy);

        Some(AnisotropyReadout {
            index,
            theta_deg: Some(theta_rad.to_degrees()),
        })
    }
}

#[cfg(test)]
use crate::permutation::{labels::permute_fixed_count, stratified::permute_within_strata};
#[cfg(test)]
use crate::spectra::structure_factor::{
    centered_structure_factor, centered_structure_factor_for_marks,
};
use crate::{
    common::matrix::F64Matrix,
    common::seeds::{derive_seed, SeedEndpoint},
    common::stats::median_average_even,
    data::Pattern,
    errors::{MarklabError, Result},
    geom::length_scales::analysis_effective_length_um,
    inference::scalar_pvalues::{permutation_p_value, Tail},
    permutation::{labels::permute_fixed_count_into, stratified::StratifiedPermutationPlan},
    spectra::kgrid::KMode,
    spectra::structure_factor::kernel::{
        centered_structure_factor_with_prevalence, power_for_selected_modes_into,
        selected_indices_for_marks_into, total_phase_sums_for_modes, BinaryMarkContext,
    },
};

#[cfg(test)]
thread_local! {
    static CHUNK_STORAGE: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
}

#[cfg(test)]
fn reset_chunk_observation() {
    CHUNK_STORAGE.set((0, 0, 0));
}

#[cfg(test)]
fn last_chunk_storage_dimensions() -> (usize, usize, usize) {
    CHUNK_STORAGE.get()
}

fn observe_chunk_storage(matrix: &F64Matrix, mode_count: usize) {
    #[cfg(not(test))]
    let _ = (matrix, mode_count);
    #[cfg(test)]
    CHUNK_STORAGE.set((
        matrix.row_count(),
        CHUNK_STORAGE.get().1.max(matrix.column_count()),
        mode_count,
    ));
}

pub(crate) fn permutation_whitened_anisotropy(
    pattern: &Pattern,
    strata: Option<&[u32]>,
    options: AnisotropyPermutationOptions,
) -> Result<Option<PermutationAnisotropy>> {
    let AnisotropyPermutationOptions {
        low_k_radius,
        n_permutations,
        seed,
        alpha,
        k_chunk_modes,
        n_marked,
    } = options;
    if pattern.len() < 2
        || n_marked == 0
        || n_marked == pattern.len()
        || low_k_radius == 0
        || n_permutations == 0
        || k_chunk_modes == 0
    {
        return Ok(None);
    }

    let Some(analysis_effective_length_um) = analysis_effective_length_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
    ) else {
        return Ok(None);
    };
    let k_step = 2.0 * std::f64::consts::PI / analysis_effective_length_um;
    let modes = low_k_modes(low_k_radius, k_step);
    if modes.is_empty() {
        return Ok(None);
    }
    let Some(mark_context) = BinaryMarkContext::new(pattern.len(), n_marked) else {
        return Ok(None);
    };
    let p_hat = n_marked as f64 / pattern.len() as f64;

    let stratified_plan = strata
        .map(|strata| StratifiedPermutationPlan::new(&pattern.mark, strata))
        .transpose()?;
    let chunk_capacity = k_chunk_modes.min(modes.len());
    let mut permutation_powers = F64Matrix::zeros(n_permutations, chunk_capacity)
        .ok_or_else(|| MarklabError::Compute("invalid anisotropy chunk dimensions".into()))?;
    observe_chunk_storage(&permutation_powers, modes.len());
    let mut observed = Vec::with_capacity(chunk_capacity);
    let mut labels = Vec::with_capacity(pattern.len());
    let mut fixed_indices = Vec::with_capacity(pattern.len());
    let mut selected_indices = Vec::with_capacity(pattern.len());
    let mut powers_scratch = Vec::with_capacity(chunk_capacity);
    let mut total_phase_sums = Vec::with_capacity(chunk_capacity);
    let mut stratum_labels = Vec::with_capacity(
        stratified_plan
            .as_ref()
            .map_or(0, StratifiedPermutationPlan::maximum_stratum_size),
    );
    let mut baseline_values = Vec::with_capacity(n_permutations);
    let mut observed_tensor = AnisotropyTensor::default();
    let mut permutation_tensors = vec![AnisotropyTensor::default(); n_permutations];
    for mode_chunk in modes.chunks(k_chunk_modes) {
        total_phase_sums_for_modes(pattern, mode_chunk, &mut total_phase_sums)
            .ok_or_else(|| MarklabError::Compute("anisotropy mode chunk is invalid".into()))?;
        observed.clear();
        for mode in mode_chunk {
            observed.push(
                centered_structure_factor_with_prevalence(pattern, p_hat, mode.kx, mode.ky)
                    .ok_or_else(|| {
                        MarklabError::Compute("observed anisotropy is undefined".into())
                    })?,
            );
        }
        for (permutation_index, powers) in permutation_powers.iter_rows_mut().enumerate() {
            let permutation_seed = derive_seed(seed, SeedEndpoint::Anisotropy, permutation_index);
            let permutation_result = if let Some(plan) = stratified_plan.as_ref() {
                plan.permute_into(permutation_seed, &mut labels, &mut stratum_labels)
            } else {
                permute_fixed_count_into(
                    pattern.len(),
                    n_marked,
                    permutation_seed,
                    &mut fixed_indices,
                    &mut labels,
                )
            };
            permutation_result.map_err(|error| {
                MarklabError::Compute(format!(
                    "anisotropy permutation {permutation_index} failed: {error}"
                ))
            })?;
            selected_indices_for_marks_into(
                &labels,
                mark_context.use_unmarked_subset(),
                &mut selected_indices,
            )
            .and_then(|()| {
                power_for_selected_modes_into(
                    pattern,
                    mode_chunk,
                    &total_phase_sums,
                    &selected_indices,
                    mark_context,
                    &mut powers_scratch,
                )
            })
            .ok_or_else(|| {
                MarklabError::Compute(format!(
                    "anisotropy permutation {permutation_index} produced an undefined mode"
                ))
            })?;
            powers[..mode_chunk.len()].copy_from_slice(&powers_scratch);
        }

        for (mode_index, mode) in mode_chunk.iter().enumerate() {
            baseline_values.clear();
            baseline_values.extend(
                permutation_powers
                    .iter_rows()
                    .map(|powers| powers[mode_index]),
            );
            let Some(baseline) = median_average_even(&mut baseline_values) else {
                return Err(MarklabError::Compute(
                    "anisotropy permutation baseline is undefined".into(),
                ));
            };
            observed_tensor.accumulate_whitened(mode.kx, mode.ky, observed[mode_index], baseline);
            for (tensor, powers) in permutation_tensors
                .iter_mut()
                .zip(permutation_powers.iter_rows())
            {
                tensor.accumulate_whitened(mode.kx, mode.ky, powers[mode_index], baseline);
            }
        }
    }

    let Some(observed_readout) = observed_tensor.readout() else {
        return Ok(None);
    };
    let Some(null) = permutation_tensors
        .into_iter()
        .map(AnisotropyTensor::readout)
        .map(|readout| readout.map(|value| value.index))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let p_value = permutation_p_value(observed_readout.index, &null, Tail::OneSidedHigh, alpha)?;

    Ok(Some(PermutationAnisotropy {
        readout: observed_readout,
        p_value,
    }))
}

fn low_k_modes(radius: usize, k_step: f64) -> Vec<KMode> {
    let radius_i = radius as isize;
    let mut modes = Vec::new();
    for mx in -radius_i..=radius_i {
        for my in -radius_i..=radius_i {
            if mx == 0 && my == 0 {
                continue;
            }
            let shell = ((mx * mx + my * my) as f64).sqrt();
            if shell <= radius as f64 {
                let kx = mx as f64 * k_step;
                let ky = my as f64 * k_step;
                modes.push(KMode {
                    kx,
                    ky,
                    k: kx.hypot(ky),
                    shell_index: shell.floor() as usize,
                });
            }
        }
    }
    modes
}

#[cfg(test)]
fn weighted_modes(modes: &[KMode], powers: &[f64], baselines: &[f64]) -> Vec<(f64, f64, f64)> {
    modes
        .iter()
        .zip(powers.iter().copied())
        .zip(baselines.iter().copied())
        .map(|((mode, power), baseline)| {
            let whitened_excess = power / baseline.max(f64::EPSILON) - 1.0;
            (mode.kx, mode.ky, whitened_excess.max(0.0))
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn permutation_whitened_anisotropy_dense_reference(
    pattern: &Pattern,
    low_k_radius: usize,
    n_permutations: usize,
    seed: u64,
    alpha: f64,
    strata: Option<&[u32]>,
) -> Result<Option<PermutationAnisotropy>> {
    let Some(analysis_effective_length_um) = analysis_effective_length_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
    ) else {
        return Ok(None);
    };
    let modes = low_k_modes(
        low_k_radius,
        2.0 * std::f64::consts::PI / analysis_effective_length_um,
    );
    let Some(observed) = modes
        .iter()
        .map(|mode| centered_structure_factor(pattern, mode.kx, mode.ky))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let mut permutation_powers = vec![vec![0.0; modes.len()]; n_permutations];
    for (permutation_index, powers) in permutation_powers.iter_mut().enumerate() {
        let permutation_seed = derive_seed(seed, SeedEndpoint::Anisotropy, permutation_index);
        let labels = if let Some(strata) = strata {
            permute_within_strata(&pattern.mark, strata, permutation_seed)
        } else {
            permute_fixed_count(pattern.len(), pattern.n_marked(), permutation_seed)
        }?;
        for (mode_index, mode) in modes.iter().enumerate() {
            let Some(power) =
                centered_structure_factor_for_marks(pattern, &labels, mode.kx, mode.ky)
            else {
                return Ok(None);
            };
            powers[mode_index] = power;
        }
    }
    let baselines = (0..modes.len())
        .map(|mode_index| {
            let mut values = permutation_powers
                .iter()
                .map(|powers| powers[mode_index])
                .collect::<Vec<_>>();
            median_average_even(&mut values)
        })
        .collect::<Option<Vec<_>>>();
    let Some(baselines) = baselines else {
        return Ok(None);
    };
    let observed_readout =
        anisotropy_from_weighted_modes(&weighted_modes(&modes, &observed, &baselines));
    let null = permutation_powers
        .iter()
        .map(|powers| {
            anisotropy_from_weighted_modes(&weighted_modes(&modes, powers, &baselines))
                .map(|readout| readout.index)
        })
        .collect::<Option<Vec<_>>>();
    let (Some(readout), Some(null)) = (observed_readout, null) else {
        return Ok(None);
    };
    let p_value = permutation_p_value(readout.index, &null, Tail::OneSidedHigh, alpha)?;
    Ok(Some(PermutationAnisotropy { readout, p_value }))
}

#[cfg(test)]
mod tests {
    use super::{
        last_chunk_storage_dimensions, permutation_whitened_anisotropy,
        permutation_whitened_anisotropy_dense_reference, reset_chunk_observation,
        AnisotropyPermutationOptions,
    };
    use crate::{data::PatternMeta, Pattern};

    #[test]
    fn chunked_anisotropy_matches_dense_reference_and_bounds_storage() {
        let pattern = test_pattern();
        let expected =
            permutation_whitened_anisotropy_dense_reference(&pattern, 3, 7, 912_345, 0.25, None)
                .expect("dense reference")
                .expect("anisotropy available");

        for chunk_size in [1, 3, 1_000] {
            reset_chunk_observation();
            let actual = permutation_whitened_anisotropy(
                &pattern,
                None,
                AnisotropyPermutationOptions {
                    low_k_radius: 3,
                    n_permutations: 7,
                    seed: 912_345,
                    alpha: 0.25,
                    k_chunk_modes: chunk_size,
                    n_marked: pattern.n_marked(),
                },
            )
            .expect("chunked anisotropy")
            .expect("anisotropy available");
            assert_eq!(actual, expected);
            let (rows, largest_columns, mode_count) = last_chunk_storage_dimensions();
            assert_eq!(rows, 7);
            assert!(largest_columns <= chunk_size.min(mode_count));
            if chunk_size < mode_count {
                assert!(largest_columns < mode_count);
            }
        }
    }

    #[test]
    fn chunked_stratified_anisotropy_matches_dense_reference() {
        let pattern = test_pattern();
        let strata = [0_u32, 0, 0, 1, 1, 1, 2, 2, 2];
        let expected = permutation_whitened_anisotropy_dense_reference(
            &pattern,
            3,
            7,
            912_345,
            0.25,
            Some(&strata),
        )
        .expect("dense reference")
        .expect("anisotropy available");

        for chunk_size in [1, 4, 1_000] {
            let actual = permutation_whitened_anisotropy(
                &pattern,
                Some(&strata),
                AnisotropyPermutationOptions {
                    low_k_radius: 3,
                    n_permutations: 7,
                    seed: 912_345,
                    alpha: 0.25,
                    k_chunk_modes: chunk_size,
                    n_marked: pattern.n_marked(),
                },
            )
            .expect("chunked anisotropy")
            .expect("anisotropy available");
            assert_eq!(actual, expected);
        }
    }

    fn test_pattern() -> Pattern {
        let mut pattern = Pattern::from_arrays(
            vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            vec![1, 0, 0, 0, 1, 0, 0, 0, 1],
            PatternMeta {
                case_id: "anisotropy".into(),
                timepoint: "post".into(),
                protein: "MSH6".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern");
        pattern.window.analysis_effective_length_um = 3.0;
        pattern
    }
}
