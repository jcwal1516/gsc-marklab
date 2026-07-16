use crate::data::Pattern;
use crate::errors::{MmrspaceError, Result};
use crate::geom::spatial_index::mean_nearest_neighbor_distance;
use crate::inference::scalar_pvalues::{permutation_p_value, Tail};
use crate::permutation::envelopes::GlobalEnvelope;
use crate::permutation::labels::{deterministic_shuffle, permute_fixed_count_indices_into};
use crate::permutation::stratified::permute_within_strata;
use crate::spectra::kgrid::{resolvable_k_modes, KBand, KMode};

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

#[derive(Clone, Debug, PartialEq)]
pub struct PermutationWhitenedSpectrum {
    pub k_values: Vec<f64>,
    pub observed_power: Vec<f64>,
    pub median_permutation_power: Vec<f64>,
    pub whitened_power: Vec<f64>,
    pub inference_eligible: Vec<bool>,
    pub lower_global_envelope: Vec<f64>,
    pub upper_global_envelope: Vec<f64>,
    pub erl_depth: f64,
    pub n_modes: usize,
    pub n_permutations: usize,
    pub low_k_excess: f64,
    pub low_k_excess_p_value: Option<f64>,
    pub p_global: f64,
    pub dominant_k: Option<f64>,
    pub xi_um: Option<f64>,
    pub xi_stability_interval_um: Option<[f64; 2]>,
    pub xi_um_p_value: Option<f64>,
    pub alpha: Option<f64>,
    pub alpha_p_value: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SpectrumScalarReadouts {
    low_k_excess: f64,
    dominant_k: Option<f64>,
    xi_um: Option<f64>,
    xi_stability_interval_um: Option<[f64; 2]>,
    alpha: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseSum {
    pub re: f64,
    pub im: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumPermutationOptions {
    pub n_shells: usize,
    pub low_k_modes: usize,
    pub n_permutations: usize,
    pub seed: u64,
    pub family_wise_alpha: f64,
    pub max_scale_um: f64,
    pub k_shell_min: usize,
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
                    let seed =
                        options.seed ^ (perm_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
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
            return Err(MmrspaceError::Compute(
                "a required spectrum permutation could not be evaluated".into(),
            ));
        }
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut selected_indices = Vec::with_capacity(pattern.len());
        for (perm_index, powers) in permutation_mode_powers.iter_mut().enumerate() {
            let seed = options.seed ^ (perm_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
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
                return Err(MmrspaceError::Compute(format!(
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
                        options.seed ^ (perm_index as u64).wrapping_mul(0x94d0_49bb_1331_11eb),
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
            return Err(MmrspaceError::Compute(
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
                options.seed ^ (perm_index as u64).wrapping_mul(0x94d0_49bb_1331_11eb),
            );
            for (mode_index, mode) in modes.iter().copied().enumerate() {
                let Some(power) =
                    centered_structure_factor_for_values(pattern, &permuted, mode.kx, mode.ky)
                else {
                    return Err(MmrspaceError::Compute(format!(
                        "probabilistic-mark spectrum permutation {perm_index} could not be evaluated"
                    )));
                };
                powers[mode_index] = power;
            }
        }
    }

    summarize_permutation_whitening(modes, observed_mode_power, permutation_mode_powers, options)
}

pub fn resolvable_modes_for_pattern(pattern: &Pattern, n_shells: usize) -> Option<Vec<KMode>> {
    let band = resolvable_band(pattern)?;
    let modes = resolvable_k_modes(band, n_shells);
    (!modes.is_empty()).then_some(modes)
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
    let use_unmarked_subset = pattern.n_marked() > pattern.n_unmarked();
    selected_indices.clear();
    selected_indices.extend(pattern.mark.iter().copied().enumerate().filter_map(
        |(index, mark)| {
            if use_unmarked_subset {
                (mark == 0).then_some(index)
            } else {
                (mark == 1).then_some(index)
            }
        },
    ));

    powers.clear();
    powers.resize(modes.len(), 0.0);
    for (mode_index, mode) in modes.iter().copied().enumerate() {
        let total = total_phase_sum(pattern, mode.kx, mode.ky)?;
        powers[mode_index] = centered_structure_factor_for_index_subset(
            pattern,
            selected_indices,
            pattern.n_marked(),
            use_unmarked_subset,
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

    let use_unmarked_subset = pattern.n_marked() > pattern.n_unmarked();
    let selected_count = if use_unmarked_subset {
        pattern.n_unmarked()
    } else {
        pattern.n_marked()
    };
    permute_fixed_count_indices_into(pattern.len(), selected_count, seed, selected_indices).ok()?;

    powers.clear();
    powers.resize(modes.len(), 0.0);
    for (mode_index, mode) in modes.iter().copied().enumerate() {
        powers[mode_index] = centered_structure_factor_for_index_subset(
            pattern,
            selected_indices,
            pattern.n_marked(),
            use_unmarked_subset,
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

pub fn stratified_permutation_whitened_spectrum<T>(
    pattern: &Pattern,
    strata: &[T],
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
    {
        return Ok(None);
    }

    let Some(band) = resolvable_band(pattern) else {
        return Ok(None);
    };
    let modes = resolvable_k_modes(band, options.n_shells);
    if modes.is_empty() {
        return Ok(None);
    }

    let Some(observed_mode_power) = modes
        .iter()
        .map(|mode| centered_structure_factor(pattern, mode.kx, mode.ky))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };

    let mut permutation_mode_powers = vec![vec![0.0; modes.len()]; options.n_permutations];
    for (perm_index, powers) in permutation_mode_powers.iter_mut().enumerate() {
        let labels = permute_within_strata(
            &pattern.mark,
            strata,
            options.seed ^ (perm_index as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9),
        )?;
        for (mode_index, mode) in modes.iter().copied().enumerate() {
            let Some(power) =
                centered_structure_factor_for_marks(pattern, &labels, mode.kx, mode.ky)
            else {
                return Err(MmrspaceError::Compute(format!(
                    "stratified spectrum permutation {perm_index} produced an undefined mode"
                )));
            };
            powers[mode_index] = power;
        }
    }

    summarize_permutation_whitening(
        &modes,
        observed_mode_power,
        permutation_mode_powers,
        options,
    )
}

fn summarize_permutation_whitening(
    modes: &[KMode],
    observed_mode_power: Vec<f64>,
    permutation_mode_powers: Vec<Vec<f64>>,
    options: SpectrumPermutationOptions,
) -> Result<Option<PermutationWhitenedSpectrum>> {
    if modes.is_empty()
        || options.n_shells == 0
        || modes.len() != observed_mode_power.len()
        || permutation_mode_powers.is_empty()
    {
        return Ok(None);
    }

    let shell_index = nonempty_shells(modes, options.n_shells);
    if shell_index.is_empty() {
        return Ok(None);
    }
    let Some(k_values) = shell_index
        .iter()
        .map(|shell| shell_mean_k(modes, *shell))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let Some(observed_power) = shell_mean_powers(modes, &observed_mode_power, &shell_index) else {
        return Ok(None);
    };
    let Some(permutation_powers) = permutation_mode_powers
        .iter()
        .map(|powers| shell_mean_powers(modes, powers, &shell_index))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let n_curve_points = k_values.len();
    let inference_eligible = k_values
        .iter()
        .map(|k| {
            k.is_finite() && *k > 0.0 && 2.0 * std::f64::consts::PI / *k <= options.max_scale_um
        })
        .collect::<Vec<_>>();
    let eligible_positions = inference_eligible
        .iter()
        .enumerate()
        .filter_map(|(index, eligible)| eligible.then_some(index))
        .collect::<Vec<_>>();
    if eligible_positions.len() < options.k_shell_min {
        return Ok(None);
    }

    let median_permutation_power = (0..n_curve_points)
        .map(|shell_position| {
            let mut values = permutation_powers
                .iter()
                .map(|powers| powers[shell_position])
                .collect::<Vec<_>>();
            median(&mut values)
        })
        .collect::<Option<Vec<_>>>();
    let Some(median_permutation_power) = median_permutation_power else {
        return Err(MmrspaceError::Compute(
            "spectrum permutation powers contain a non-finite or empty shell".into(),
        ));
    };

    let whitened_power = observed_power
        .iter()
        .zip(median_permutation_power.iter())
        .map(|(observed, baseline)| observed / baseline.max(f64::EPSILON))
        .collect::<Vec<_>>();
    let envelope = GlobalEnvelope::from_curves_with_eligibility(
        &observed_power,
        &permutation_powers,
        options.family_wise_alpha,
        &inference_eligible,
    )?;

    let eligible_k_values = eligible_positions
        .iter()
        .map(|index| k_values[*index])
        .collect::<Vec<_>>();
    let eligible_whitened_power = eligible_positions
        .iter()
        .map(|index| whitened_power[*index])
        .collect::<Vec<_>>();
    let low_count = options.low_k_modes.min(eligible_positions.len()).max(1);
    let observed_readouts =
        spectrum_scalar_readouts(&eligible_k_values, &eligible_whitened_power, low_count);
    let permutation_readouts = permutation_powers
        .iter()
        .map(|powers| {
            let whitened = powers
                .iter()
                .zip(median_permutation_power.iter())
                .map(|(observed, baseline)| observed / baseline.max(f64::EPSILON))
                .collect::<Vec<_>>();
            let eligible_whitened = eligible_positions
                .iter()
                .map(|index| whitened[*index])
                .collect::<Vec<_>>();
            spectrum_scalar_readouts(&eligible_k_values, &eligible_whitened, low_count)
        })
        .collect::<Vec<_>>();
    let low_k_null = permutation_readouts
        .iter()
        .map(|readout| readout.low_k_excess)
        .collect::<Vec<_>>();
    let xi_null = permutation_readouts
        .iter()
        .map(|readout| readout.xi_um)
        .collect::<Option<Vec<_>>>();
    let alpha_null = permutation_readouts
        .iter()
        .map(|readout| readout.alpha)
        .collect::<Option<Vec<_>>>();
    let low_k_excess_p_value = Some(permutation_p_value(
        observed_readouts.low_k_excess,
        &low_k_null,
        Tail::OneSidedHigh,
        options.family_wise_alpha,
    )?);
    let xi_um_p_value = match (observed_readouts.xi_um, xi_null) {
        (Some(xi_um), Some(null)) => Some(permutation_p_value(
            xi_um,
            &null,
            Tail::TwoSided,
            options.family_wise_alpha,
        )?),
        _ => None,
    };
    let alpha_p_value = match (observed_readouts.alpha, alpha_null) {
        (Some(alpha), Some(null)) => Some(permutation_p_value(
            alpha,
            &null,
            Tail::TwoSided,
            options.family_wise_alpha,
        )?),
        _ => None,
    };

    Ok(Some(PermutationWhitenedSpectrum {
        k_values,
        observed_power,
        median_permutation_power,
        whitened_power,
        inference_eligible,
        lower_global_envelope: envelope.lower,
        upper_global_envelope: envelope.upper,
        erl_depth: envelope.erl_depth,
        n_modes: modes.len(),
        n_permutations: envelope.n_permutations,
        low_k_excess: observed_readouts.low_k_excess,
        low_k_excess_p_value,
        p_global: envelope.p_global,
        dominant_k: observed_readouts.dominant_k,
        xi_um: observed_readouts.xi_um,
        xi_stability_interval_um: observed_readouts.xi_stability_interval_um,
        xi_um_p_value,
        alpha: observed_readouts.alpha,
        alpha_p_value,
    }))
}

fn spectrum_scalar_readouts(
    k_values: &[f64],
    whitened_power: &[f64],
    low_count: usize,
) -> SpectrumScalarReadouts {
    if k_values.is_empty() || whitened_power.is_empty() {
        return SpectrumScalarReadouts {
            low_k_excess: 0.0,
            dominant_k: None,
            xi_um: None,
            xi_stability_interval_um: None,
            alpha: None,
        };
    }
    let low_count = low_count
        .min(whitened_power.len())
        .min(k_values.len())
        .max(1);
    let low_k_excess = whitened_power[..low_count].iter().sum::<f64>() / low_count as f64;
    let dominant = whitened_power
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, power)| *index < k_values.len() && power.is_finite())
        .max_by(|(_, left), (_, right)| left.total_cmp(right));
    let dominant_k = dominant.map(|(index, _)| k_values[index]);
    let xi_um = dominant_k.map(|k| 2.0 * std::f64::consts::PI / k);
    let xi_stability_interval_um = xi_stability_interval_um(k_values, whitened_power);
    let alpha = low_k_log_slope(&k_values[..low_count], &whitened_power[..low_count]);

    SpectrumScalarReadouts {
        low_k_excess,
        dominant_k,
        xi_um,
        xi_stability_interval_um,
        alpha,
    }
}

fn xi_stability_interval_um(k_values: &[f64], whitened_power: &[f64]) -> Option<[f64; 2]> {
    let peak = whitened_power
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    if !peak.is_finite() || peak <= 0.0 {
        return None;
    }

    let threshold = peak * 0.95;
    let mut min_xi = f64::INFINITY;
    let mut max_xi = f64::NEG_INFINITY;
    for (k, power) in k_values.iter().copied().zip(whitened_power.iter().copied()) {
        if k > 0.0 && k.is_finite() && power.is_finite() && power >= threshold {
            let xi = 2.0 * std::f64::consts::PI / k;
            min_xi = min_xi.min(xi);
            max_xi = max_xi.max(xi);
        }
    }

    (min_xi.is_finite() && max_xi.is_finite()).then_some([min_xi, max_xi])
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

fn centered_structure_factor_for_index_subset(
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

fn total_phase_sum(pattern: &Pattern, kx: f64, ky: f64) -> Option<PhaseSum> {
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

fn effective_length_um(pattern: &Pattern) -> Option<f64> {
    if pattern.window.l_eff_um.is_finite() && pattern.window.l_eff_um > 0.0 {
        return Some(pattern.window.l_eff_um);
    }
    let (min_x, max_x) = min_max(&pattern.x_um)?;
    let (min_y, max_y) = min_max(&pattern.y_um)?;
    let span = (max_x - min_x).max(max_y - min_y);
    (span > 0.0).then_some(span)
}

fn resolvable_band(pattern: &Pattern) -> Option<KBand> {
    let l_eff_um = effective_length_um(pattern)?;
    let d_nn_mean_um =
        if pattern.window.d_nn_mean_um.is_finite() && pattern.window.d_nn_mean_um > 0.0 {
            pattern.window.d_nn_mean_um
        } else {
            mean_nearest_neighbor_distance(&pattern.x_um, &pattern.y_um)?
        };
    KBand::from_window(l_eff_um, d_nn_mean_um)
}

fn nonempty_shells(modes: &[KMode], n_shells: usize) -> Vec<usize> {
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

fn shell_mean_k(modes: &[KMode], shell_index: usize) -> Option<f64> {
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

fn shell_mean_powers(modes: &[KMode], powers: &[f64], shell_index: &[usize]) -> Option<Vec<f64>> {
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

fn min_max(values: &[f64]) -> Option<(f64, f64)> {
    let mut iter = values.iter().copied().filter(|value| value.is_finite());
    let first = iter.next()?;
    let mut min = first;
    let mut max = first;
    for value in iter {
        min = min.min(value);
        max = max.max(value);
    }
    Some((min, max))
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        Some((values[mid - 1] + values[mid]) * 0.5)
    } else {
        Some(values[mid])
    }
}

fn low_k_log_slope(k_values: &[f64], powers: &[f64]) -> Option<f64> {
    if k_values.len() != powers.len() || k_values.len() < 2 {
        return None;
    }

    let pairs = k_values
        .iter()
        .copied()
        .zip(powers.iter().copied())
        .filter(|(k, p)| *k > 0.0 && *p > 0.0 && k.is_finite() && p.is_finite())
        .map(|(k, p)| (k.ln(), p.ln()))
        .collect::<Vec<_>>();
    if pairs.len() < 2 {
        return None;
    }

    let mean_x = pairs.iter().map(|(x, _)| x).sum::<f64>() / pairs.len() as f64;
    let mean_y = pairs.iter().map(|(_, y)| y).sum::<f64>() / pairs.len() as f64;
    let numerator = pairs
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>();
    let denominator = pairs
        .iter()
        .map(|(x, _)| (x - mean_x) * (x - mean_x))
        .sum::<f64>();
    (denominator > 0.0).then_some(numerator / denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PatternMeta;
    use approx::assert_abs_diff_eq;

    fn pattern(marks: Vec<u8>) -> Pattern {
        Pattern::from_arrays(
            vec![0.0, 1.0, 2.0, 3.0],
            vec![0.0, 1.0, 0.0, 1.0],
            marks,
            PatternMeta {
                case_id: "case_001".into(),
                timepoint: "post".into(),
                protein: "MSH6".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern")
    }

    #[test]
    fn marked_subset_core_matches_dense_binary_labels() {
        let pattern = pattern(vec![1, 0, 1, 0]);
        let total = total_phase_sum(&pattern, 0.7, 1.3).expect("total phase sum");
        let by_labels = centered_structure_factor_for_marks(&pattern, &pattern.mark, 0.7, 1.3)
            .expect("label structure factor");
        let by_indices = centered_structure_factor_for_index_subset(
            &pattern,
            &[0, 2],
            2,
            false,
            total,
            0.7,
            1.3,
        )
        .expect("subset structure factor");

        assert_abs_diff_eq!(by_indices, by_labels, epsilon = 1e-12);
    }

    #[test]
    fn unmarked_subset_core_matches_dense_binary_labels() {
        let pattern = pattern(vec![1, 1, 1, 0]);
        let total = total_phase_sum(&pattern, 0.7, 1.3).expect("total phase sum");
        let by_labels = centered_structure_factor_for_marks(&pattern, &pattern.mark, 0.7, 1.3)
            .expect("label structure factor");
        let by_indices =
            centered_structure_factor_for_index_subset(&pattern, &[3], 3, true, total, 0.7, 1.3)
                .expect("subset structure factor");

        assert_abs_diff_eq!(by_indices, by_labels, epsilon = 1e-12);
    }

    #[test]
    fn shell_means_group_the_production_modes() {
        let modes = vec![
            KMode {
                kx: 0.1,
                ky: 0.0,
                k: 0.1,
                shell_index: 0,
            },
            KMode {
                kx: 0.2,
                ky: 0.0,
                k: 0.2,
                shell_index: 0,
            },
            KMode {
                kx: 0.9,
                ky: 0.0,
                k: 0.9,
                shell_index: 1,
            },
            KMode {
                kx: 1.0,
                ky: 0.0,
                k: 1.0,
                shell_index: 1,
            },
        ];
        let means =
            shell_mean_powers(&modes, &[2.0, 4.0, 10.0, 14.0], &[0, 1]).expect("shell means");

        assert_abs_diff_eq!(means[0], 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(means[1], 12.0, epsilon = 1e-12);
    }

    #[test]
    fn low_k_readout_averages_only_requested_shells() {
        let readout = spectrum_scalar_readouts(&[0.1, 0.2, 0.3], &[2.0, 4.0, 100.0], 2);

        assert_abs_diff_eq!(readout.low_k_excess, 3.0, epsilon = 1e-12);
    }
}
