use crate::{
    common::seeds::{derive_seed, SeedEndpoint},
    data::Pattern,
    errors::{MarklabError, Result},
    permutation::{labels::deterministic_shuffle, stratified::permute_within_strata},
    spectra::kgrid::KMode,
};

use super::{
    kernel::{
        centered_structure_factor_for_marks, centered_structure_factor_for_values,
        observed_power_for_modes, observed_value_power_for_modes, permutation_power_for_modes_into,
        total_phase_sums_for_modes,
    },
    modes::resolvable_modes_for_pattern,
    summaries::summarize_permutation_whitening,
    PermutationWhitenedSpectrum, SpectrumPermutationOptions,
};

pub fn permutation_whitened_spectrum(
    pattern: &Pattern,
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>> {
    if pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || options.n_shells == 0
        || options.n_permutations == 0
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
        || modes.is_empty()
        || observed_mode_power.len() != modes.len()
    {
        return Ok(None);
    }

    let mut total_phase_sums = Vec::with_capacity(modes.len());
    if total_phase_sums_for_modes(pattern, modes, &mut total_phase_sums).is_none() {
        return Ok(None);
    }
    let mut permutation_mode_powers = vec![vec![0.0; modes.len()]; options.n_permutations];

    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;

        let all_permutations_valid = permutation_mode_powers
            .par_iter_mut()
            .enumerate()
            .map_init(
                || Vec::with_capacity(pattern.len()),
                |selected_indices, (perm_index, powers)| {
                    let seed = derive_seed(options.seed, SeedEndpoint::SpectrumBinary, perm_index);
                    permutation_power_for_modes_into(
                        pattern,
                        modes,
                        &total_phase_sums,
                        seed,
                        selected_indices,
                        powers,
                    )
                    .is_some()
                },
            )
            .all(|ok| ok);
        if !all_permutations_valid {
            return Err(MarklabError::Compute(
                "a required spectrum permutation could not be evaluated".into(),
            ));
        }
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut selected_indices = Vec::with_capacity(pattern.len());
        for (perm_index, powers) in permutation_mode_powers.iter_mut().enumerate() {
            let seed = derive_seed(options.seed, SeedEndpoint::SpectrumBinary, perm_index);
            if permutation_power_for_modes_into(
                pattern,
                modes,
                &total_phase_sums,
                seed,
                &mut selected_indices,
                powers,
            )
            .is_none()
            {
                return Err(MarklabError::Compute(format!(
                    "spectrum permutation {perm_index} could not be evaluated"
                )));
            }
        }
    }

    summarize_permutation_whitening(modes, observed_mode_power, permutation_mode_powers, options)
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
        || modes.is_empty()
        || observed_mode_power.len() != modes.len()
    {
        return Ok(None);
    }

    let mut permutation_mode_powers = vec![vec![0.0; modes.len()]; options.n_permutations];

    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;

        let all_permutations_valid = permutation_mode_powers
            .par_iter_mut()
            .enumerate()
            .map_init(
                || Vec::with_capacity(values.len()),
                |permuted, (perm_index, powers)| {
                    permuted.clear();
                    permuted.extend_from_slice(values);
                    deterministic_shuffle(
                        permuted,
                        derive_seed(options.seed, SeedEndpoint::SpectrumContinuous, perm_index),
                    );
                    for (mode_index, mode) in modes.iter().copied().enumerate() {
                        let Some(power) = centered_structure_factor_for_values(
                            pattern, permuted, mode.kx, mode.ky,
                        ) else {
                            return false;
                        };
                        powers[mode_index] = power;
                    }
                    true
                },
            )
            .all(|ok| ok);
        if !all_permutations_valid {
            return Err(MarklabError::Compute(
                "a required probabilistic-mark spectrum permutation could not be evaluated".into(),
            ));
        }
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut permuted = Vec::with_capacity(values.len());
        for (perm_index, powers) in permutation_mode_powers.iter_mut().enumerate() {
            permuted.clear();
            permuted.extend_from_slice(values);
            deterministic_shuffle(
                &mut permuted,
                derive_seed(options.seed, SeedEndpoint::SpectrumContinuous, perm_index),
            );
            for (mode_index, mode) in modes.iter().copied().enumerate() {
                let Some(power) =
                    centered_structure_factor_for_values(pattern, &permuted, mode.kx, mode.ky)
                else {
                    return Err(MarklabError::Compute(format!(
                        "probabilistic-mark spectrum permutation {perm_index} could not be evaluated"
                    )));
                };
                powers[mode_index] = power;
            }
        }
    }

    summarize_permutation_whitening(modes, observed_mode_power, permutation_mode_powers, options)
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
        || modes.is_empty()
        || observed_mode_power.len() != modes.len()
    {
        return Ok(None);
    }

    let mut permutation_mode_powers = vec![vec![0.0; modes.len()]; options.n_permutations];
    for (perm_index, powers) in permutation_mode_powers.iter_mut().enumerate() {
        let labels = permute_within_strata(
            &pattern.mark,
            strata,
            derive_seed(options.seed, SeedEndpoint::SpectrumStratified, perm_index),
        )?;
        for (mode_index, mode) in modes.iter().copied().enumerate() {
            let Some(power) =
                centered_structure_factor_for_marks(pattern, &labels, mode.kx, mode.ky)
            else {
                return Err(MarklabError::Compute(format!(
                    "stratified spectrum permutation {perm_index} produced an undefined mode"
                )));
            };
            powers[mode_index] = power;
        }
    }

    summarize_permutation_whitening(modes, observed_mode_power, permutation_mode_powers, options)
}
