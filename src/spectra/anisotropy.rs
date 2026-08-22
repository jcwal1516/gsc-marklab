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

pub fn anisotropy_from_weighted_modes(modes: &[(f64, f64, f64)]) -> Option<AnisotropyReadout> {
    let mut xx = 0.0;
    let mut xy = 0.0;
    let mut yy = 0.0;

    for (kx, ky, weight) in modes.iter().copied() {
        if !kx.is_finite() || !ky.is_finite() || !weight.is_finite() {
            continue;
        }
        let w = weight.max(0.0);
        xx += w * kx * kx;
        xy += w * kx * ky;
        yy += w * ky * ky;
    }

    let trace = xx + yy;
    if trace <= 0.0 {
        return None;
    }

    let delta = ((xx - yy) * (xx - yy) + 4.0 * xy * xy).sqrt();
    let lambda_max = 0.5 * (trace + delta);
    let lambda_min = 0.5 * (trace - delta);
    let index = if lambda_min <= f64::EPSILON {
        (lambda_max / f64::EPSILON).min(1.0e15)
    } else {
        lambda_max / lambda_min
    };
    let theta_rad = 0.5 * (2.0 * xy).atan2(xx - yy);

    Some(AnisotropyReadout {
        index,
        theta_deg: Some(theta_rad.to_degrees()),
    })
}

use crate::{
    common::seeds::{derive_seed, SeedEndpoint},
    common::stats::median_average_even,
    data::Pattern,
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value, Tail},
    permutation::{labels::permute_fixed_count, stratified::permute_within_strata},
    spectra::structure_factor::{centered_structure_factor, centered_structure_factor_for_marks},
};

pub(crate) fn permutation_whitened_anisotropy(
    pattern: &Pattern,
    low_k_radius: usize,
    n_permutations: usize,
    seed: u64,
    alpha: f64,
    strata: Option<&[u32]>,
) -> Result<Option<PermutationAnisotropy>> {
    if pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || low_k_radius == 0
        || n_permutations == 0
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

    let Some(observed) = modes
        .iter()
        .map(|(kx, ky)| centered_structure_factor(pattern, *kx, *ky))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let mut permutation_powers = vec![vec![0.0; modes.len()]; n_permutations];
    for (perm_index, powers) in permutation_powers.iter_mut().enumerate() {
        let permutation_seed = derive_seed(seed, SeedEndpoint::Anisotropy, perm_index);
        let labels = if let Some(strata) = strata {
            permute_within_strata(&pattern.mark, strata, permutation_seed)
        } else {
            permute_fixed_count(pattern.len(), pattern.n_marked(), permutation_seed)
        }
        .map_err(|error| {
            MarklabError::Compute(format!(
                "anisotropy permutation {perm_index} failed: {error}"
            ))
        })?;
        for (mode_index, (kx, ky)) in modes.iter().copied().enumerate() {
            let Some(power) = centered_structure_factor_for_marks(pattern, &labels, kx, ky) else {
                return Err(MarklabError::Compute(format!(
                    "anisotropy permutation {perm_index} produced an undefined mode"
                )));
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
        return Err(MarklabError::Compute(
            "anisotropy permutation baseline is undefined".into(),
        ));
    };

    let observed_modes = weighted_modes(&modes, &observed, &baselines);
    let Some(observed_readout) = anisotropy_from_weighted_modes(&observed_modes) else {
        return Ok(None);
    };
    let Some(null) = permutation_powers
        .iter()
        .map(|powers| {
            let weighted = weighted_modes(&modes, powers, &baselines);
            anisotropy_from_weighted_modes(&weighted)
        })
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
