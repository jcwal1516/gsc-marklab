use crate::{
    common::stats::median_average_even,
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value, Tail},
    permutation::envelopes::GlobalEnvelope,
    spectra::kgrid::KMode,
};

use super::{
    shells::{nonempty_shells, shell_mean_k, shell_mean_powers},
    PermutationWhitenedSpectrum, SpectrumPermutationOptions,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SpectrumScalarReadouts {
    pub(super) low_k_excess: f64,
    pub(super) dominant_k: Option<f64>,
    pub(super) xi_um: Option<f64>,
    pub(super) xi_stability_interval_um: Option<[f64; 2]>,
    pub(super) alpha: Option<f64>,
}

pub(super) fn summarize_permutation_whitening(
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

    // Preserve the historical observed whitening baseline: the median of the
    // B permutation curves.
    let median_permutation_power = (0..n_curve_points)
        .map(|shell_position| {
            let mut values = permutation_powers
                .iter()
                .map(|powers| powers[shell_position])
                .collect::<Vec<_>>();
            median_average_even(&mut values)
        })
        .collect::<Option<Vec<_>>>();
    let Some(median_permutation_power) = median_permutation_power else {
        return Err(MarklabError::Compute(
            "spectrum permutation powers contain a non-finite or empty shell".into(),
        ));
    };
    // Each permutation uses its corresponding leave-one-out baseline: the
    // observed curve plus the other B - 1 permutation curves. Reusing the
    // observed baseline would privilege that run and break rank-test symmetry.
    let mut permutation_baselines = vec![vec![0.0; n_curve_points]; permutation_powers.len()];
    for shell_position in 0..n_curve_points {
        let mut values = Vec::with_capacity(permutation_powers.len() + 1);
        values.push(observed_power[shell_position]);
        values.extend(
            permutation_powers
                .iter()
                .map(|powers| powers[shell_position]),
        );
        let Some(baselines) = leave_one_out_medians(&values) else {
            return Err(MarklabError::Compute(
                "spectrum powers contain a non-finite or empty leave-one-out shell".into(),
            ));
        };
        debug_assert_eq!(median_permutation_power[shell_position], baselines[0]);
        for (permutation_index, baseline) in baselines.into_iter().skip(1).enumerate() {
            permutation_baselines[permutation_index][shell_position] = baseline;
        }
    }

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
        .zip(permutation_baselines.iter())
        .map(|(powers, baselines)| {
            let whitened = powers
                .iter()
                .zip(baselines.iter())
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

pub(super) fn spectrum_scalar_readouts(
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

pub(super) fn leave_one_out_medians(values: &[f64]) -> Option<Vec<f64>> {
    if values.len() < 2 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| values[*left].total_cmp(&values[*right]));
    let sorted_values = order.iter().map(|index| values[*index]).collect::<Vec<_>>();
    let mut sorted_positions = vec![0; values.len()];
    for (position, original_index) in order.into_iter().enumerate() {
        sorted_positions[original_index] = position;
    }
    let remaining_len = values.len() - 1;
    let middle = remaining_len / 2;
    Some(
        sorted_positions
            .into_iter()
            .map(|removed_position| {
                if !remaining_len.is_multiple_of(2) {
                    if removed_position <= middle {
                        sorted_values[middle + 1]
                    } else {
                        sorted_values[middle]
                    }
                } else if removed_position < middle {
                    (sorted_values[middle] + sorted_values[middle + 1]) * 0.5
                } else if removed_position == middle {
                    (sorted_values[middle - 1] + sorted_values[middle + 1]) * 0.5
                } else {
                    (sorted_values[middle - 1] + sorted_values[middle]) * 0.5
                }
            })
            .collect(),
    )
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
