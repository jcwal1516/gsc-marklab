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
use crate::{
    common::matrix::F64Matrix,
    common::seeds::{derive_seed, SeedEndpoint},
    common::stats::median_average_even,
    data::Pattern,
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value, Tail},
    permutation::{labels::permute_fixed_count_into, stratified::StratifiedPermutationPlan},
    spectra::structure_factor::{centered_structure_factor, centered_structure_factor_for_marks},
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
    low_k_radius: usize,
    n_permutations: usize,
    seed: u64,
    alpha: f64,
    strata: Option<&[u32]>,
    k_chunk_modes: usize,
) -> Result<Option<PermutationAnisotropy>> {
    if pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || low_k_radius == 0
        || n_permutations == 0
        || k_chunk_modes == 0
    {
        return Ok(None);
    }

    let Some(l_eff_um) = effective_length_um(pattern) else {
        return Ok(None);
    };
    let k_step = 2.0 * std::f64::consts::PI / l_eff_um;
    let modes = low_k_modes(low_k_radius, k_step);
    if modes.is_empty() {
        return Ok(None);
    }

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
    let mut stratum_labels = Vec::with_capacity(
        stratified_plan
            .as_ref()
            .map_or(0, StratifiedPermutationPlan::maximum_stratum_size),
    );
    let mut baseline_values = Vec::with_capacity(n_permutations);
    let mut observed_tensor = AnisotropyTensor::default();
    let mut permutation_tensors = vec![AnisotropyTensor::default(); n_permutations];
    for mode_chunk in modes.chunks(k_chunk_modes) {
        observed.clear();
        for (kx, ky) in mode_chunk {
            let Some(power) = centered_structure_factor(pattern, *kx, *ky) else {
                return Ok(None);
            };
            observed.push(power);
        }
        for (permutation_index, powers) in permutation_powers.iter_rows_mut().enumerate() {
            let permutation_seed = derive_seed(seed, SeedEndpoint::Anisotropy, permutation_index);
            let permutation_result = if let Some(plan) = stratified_plan.as_ref() {
                plan.permute_into(permutation_seed, &mut labels, &mut stratum_labels)
            } else {
                permute_fixed_count_into(
                    pattern.len(),
                    pattern.n_marked(),
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
            for (mode_index, (kx, ky)) in mode_chunk.iter().copied().enumerate() {
                let Some(power) = centered_structure_factor_for_marks(pattern, &labels, kx, ky)
                else {
                    return Err(MarklabError::Compute(format!(
                        "anisotropy permutation {permutation_index} produced an undefined mode"
                    )));
                };
                powers[mode_index] = power;
            }
        }

        for (mode_index, (kx, ky)) in mode_chunk.iter().copied().enumerate() {
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
            observed_tensor.accumulate_whitened(kx, ky, observed[mode_index], baseline);
            for (tensor, powers) in permutation_tensors
                .iter_mut()
                .zip(permutation_powers.iter_rows())
            {
                tensor.accumulate_whitened(kx, ky, powers[mode_index], baseline);
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

fn low_k_modes(radius: usize, k_step: f64) -> Vec<(f64, f64)> {
    let radius_i = radius as isize;
    let mut modes = Vec::new();
    for mx in -radius_i..=radius_i {
        for my in -radius_i..=radius_i {
            if mx == 0 && my == 0 {
                continue;
            }
            let shell = ((mx * mx + my * my) as f64).sqrt();
            if shell <= radius as f64 {
                modes.push((mx as f64 * k_step, my as f64 * k_step));
            }
        }
    }
    modes
}

#[cfg(test)]
fn weighted_modes(modes: &[(f64, f64)], powers: &[f64], baselines: &[f64]) -> Vec<(f64, f64, f64)> {
    modes
        .iter()
        .copied()
        .zip(powers.iter().copied())
        .zip(baselines.iter().copied())
        .map(|(((kx, ky), power), baseline)| {
            let whitened_excess = power / baseline.max(f64::EPSILON) - 1.0;
            (kx, ky, whitened_excess.max(0.0))
        })
        .collect()
}

#[cfg(test)]
fn permutation_whitened_anisotropy_dense_reference(
    pattern: &Pattern,
    low_k_radius: usize,
    n_permutations: usize,
    seed: u64,
    alpha: f64,
    strata: Option<&[u32]>,
) -> Result<Option<PermutationAnisotropy>> {
    let Some(l_eff_um) = effective_length_um(pattern) else {
        return Ok(None);
    };
    let modes = low_k_modes(low_k_radius, 2.0 * std::f64::consts::PI / l_eff_um);
    let Some(observed) = modes
        .iter()
        .map(|(kx, ky)| centered_structure_factor(pattern, *kx, *ky))
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
        for (mode_index, (kx, ky)) in modes.iter().copied().enumerate() {
            let Some(power) = centered_structure_factor_for_marks(pattern, &labels, kx, ky) else {
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

fn effective_length_um(pattern: &Pattern) -> Option<f64> {
    if pattern.window.l_eff_um.is_finite() && pattern.window.l_eff_um > 0.0 {
        return Some(pattern.window.l_eff_um);
    }
    let min_x = pattern.x_um.iter().copied().reduce(f64::min)?;
    let max_x = pattern.x_um.iter().copied().reduce(f64::max)?;
    let min_y = pattern.y_um.iter().copied().reduce(f64::min)?;
    let max_y = pattern.y_um.iter().copied().reduce(f64::max)?;
    let span = (max_x - min_x).max(max_y - min_y);
    (span > 0.0).then_some(span)
}

#[cfg(test)]
mod tests {
    use super::{
        last_chunk_storage_dimensions, permutation_whitened_anisotropy,
        permutation_whitened_anisotropy_dense_reference, reset_chunk_observation,
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
            let actual =
                permutation_whitened_anisotropy(&pattern, 3, 7, 912_345, 0.25, None, chunk_size)
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
                3,
                7,
                912_345,
                0.25,
                Some(&strata),
                chunk_size,
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
        pattern.window.l_eff_um = 3.0;
        pattern
    }
}
