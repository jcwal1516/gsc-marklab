use crate::{
    api::{finite_option, qc_pipeline::permutation_labels},
    common::seeds::SeedEndpoint,
    config::AnalysisConfig,
    data::Pattern,
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value, Tail},
    multiscale_residual::{
        energy::relative_scale_energies_from_field,
        territories::{detect_residual_territories, ResidualTerritoryCandidate},
    },
    output::{FunctionalSummary, MarkPairCovariancePoint, ResidualTerritory, ScaleEnergyPoint},
    periodogram::raster::{centered_mark_raster, centered_mark_raster_for_marks},
    periodogram::tapered::hann_tapered_raster_periodogram,
    permutation::envelopes::GlobalEnvelope,
    spectra::mark_pair_covariance::{mark_pair_covariance, mark_pair_covariance_for_marks},
};

pub(super) fn mark_pair_covariance_with_envelope(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> Result<(
    Vec<MarkPairCovariancePoint>,
    crate::output::AnalysisSection<FunctionalSummary>,
)> {
    let bin_width_um = pattern.window.d_nn_mean_um.max(1.0);
    let max_r_um =
        (pattern.window.l_eff_um * config.validation.largest_interpretable_scale_fraction).max(1.0);
    let Some(observed_bins) = mark_pair_covariance(pattern, bin_width_um, max_r_um) else {
        return Ok((
            Vec::new(),
            crate::output::AnalysisSection::InsufficientData {
                reason: "mark-pair covariance could not be estimated".into(),
            },
        ));
    };

    let observed_values = observed_bins
        .iter()
        .map(|bin| bin.value.unwrap_or(0.0))
        .collect::<Vec<_>>();
    let inference_eligible = observed_bins
        .iter()
        .map(|bin| bin.value.is_some() && bin.r_max_um <= max_r_um)
        .collect::<Vec<_>>();
    if observed_bins
        .iter()
        .filter_map(|bin| bin.value)
        .any(|value| !value.is_finite())
    {
        return Err(MarklabError::Compute(
            "observed mark-pair-covariance curve contains a non-finite value".into(),
        ));
    }
    let permutation_curves = mark_pair_covariance_permutation_curves(
        config,
        pattern,
        bin_width_um,
        max_r_um,
        observed_values.len(),
    )?;
    let envelope = match permutation_curves {
        Some(permutation_curves) if inference_eligible.iter().any(|eligible| *eligible) => {
            Some(GlobalEnvelope::from_curves_with_eligibility(
                &observed_values,
                &permutation_curves,
                config.inference.family_wise_alpha,
                &inference_eligible,
            )?)
        }
        None => None,
        Some(_) => None,
    };
    let summary = envelope.as_ref().map_or_else(
        || crate::output::AnalysisSection::InsufficientData {
            reason: "at least one required mark-pair-covariance null curve was undefined".into(),
        },
        |envelope| {
            crate::output::AnalysisSection::available(FunctionalSummary {
                p_global: finite_option(envelope.p_global),
                erl_depth: Some(envelope.erl_depth),
                n_permutations: envelope.n_permutations,
            })
        },
    );

    let points = observed_bins
        .into_iter()
        .enumerate()
        .map(|(index, bin)| {
            let envelope_bounds = bin.value.and(envelope.as_ref()).and_then(|envelope| {
                let lower = envelope.lower.get(index).copied().and_then(finite_option)?;
                let upper = envelope.upper.get(index).copied().and_then(finite_option)?;
                Some((lower, upper))
            });
            if !bin.r_min_um.is_finite() || !bin.r_max_um.is_finite() {
                return Err(MarklabError::Compute(format!(
                    "mark-pair-covariance bin {index} has non-finite bounds"
                )));
            }
            Ok(MarkPairCovariancePoint {
                r_min_um: bin.r_min_um,
                r_max_um: bin.r_max_um,
                covariance: bin.value,
                inference_eligible: inference_eligible[index],
                lower_global_envelope: envelope_bounds.map(|bounds| bounds.0),
                upper_global_envelope: envelope_bounds.map(|bounds| bounds.1),
                pair_count: bin.count,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok((points, summary))
}

pub(super) fn mark_pair_covariance_permutation_curves(
    config: &AnalysisConfig,
    pattern: &Pattern,
    bin_width_um: f64,
    max_r_um: f64,
    expected_len: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    if config.permutation.b == 0 || pattern.n_marked() == 0 || pattern.n_unmarked() == 0 {
        return Ok(None);
    }

    let mut curves = Vec::with_capacity(config.permutation.b);
    for perm_index in 0..config.permutation.b {
        let labels = permutation_labels(
            config,
            pattern,
            perm_index,
            SeedEndpoint::MarkPairCovariance,
        )?;
        let Some(bins) = mark_pair_covariance_for_marks(pattern, &labels, bin_width_um, max_r_um)
        else {
            return Ok(None);
        };
        if bins.len() != expected_len
            || bins
                .iter()
                .filter_map(|bin| bin.value)
                .any(|value| !value.is_finite())
        {
            return Ok(None);
        }
        curves.push(
            bins.into_iter()
                .map(|bin| bin.value.unwrap_or(0.0))
                .collect(),
        );
    }
    Ok(Some(curves))
}

pub(super) fn scale_energy_with_envelope(
    config: &AnalysisConfig,
    pattern: &Pattern,
    local_difference_energy_fraction: f64,
    residual_energy_fraction: f64,
    block_mean_variance_fraction: f64,
) -> Result<(
    Vec<ScaleEnergyPoint>,
    crate::output::AnalysisSection<FunctionalSummary>,
)> {
    let observed_values = vec![
        local_difference_energy_fraction,
        residual_energy_fraction,
        block_mean_variance_fraction,
    ];
    if observed_values.iter().any(|value| !value.is_finite()) {
        return Err(MarklabError::Compute(
            "observed scale-energy curve contains a non-finite value".into(),
        ));
    }
    let permutation_curves =
        scale_energy_permutation_curves(config, pattern, observed_values.len())?;
    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    let bands = [
        ("local_difference", pattern.window.d_nn_mean_um.max(1.0)),
        ("residual", pattern.window.d_nn_mean_um.max(1.0) * 2.0),
        ("block_mean", pattern.window.l_eff_um.max(1.0) / 4.0),
    ];
    let eligibility = bands
        .iter()
        .map(|(_, scale_um)| *scale_um <= max_scale_um)
        .collect::<Vec<_>>();
    let envelope = match permutation_curves {
        _ if !eligibility.iter().any(|eligible| *eligible) => None,
        Some(permutation_curves) => Some(GlobalEnvelope::from_curves_with_eligibility(
            &observed_values,
            &permutation_curves,
            config.inference.family_wise_alpha,
            &eligibility,
        )?),
        None => None,
    };
    let summary = envelope.as_ref().map_or_else(
        || crate::output::AnalysisSection::InsufficientData {
            reason: "at least one required scale-energy null curve was undefined".into(),
        },
        |envelope| {
            crate::output::AnalysisSection::available(FunctionalSummary {
                p_global: finite_option(envelope.p_global),
                erl_depth: Some(envelope.erl_depth),
                n_permutations: envelope.n_permutations,
            })
        },
    );

    let points = observed_values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, energy_fraction)| {
            let envelope_bounds = envelope.as_ref().and_then(|envelope| {
                let lower = envelope.lower.get(index).copied().and_then(finite_option)?;
                let upper = envelope.upper.get(index).copied().and_then(finite_option)?;
                Some((lower, upper))
            });
            ScaleEnergyPoint {
                band: bands[index].0.into(),
                scale_um: bands[index].1,
                energy_fraction,
                inference_eligible: eligibility[index],
                lower_global_envelope: envelope_bounds.map(|bounds| bounds.0),
                upper_global_envelope: envelope_bounds.map(|bounds| bounds.1),
            }
        })
        .collect::<Vec<_>>();

    Ok((points, summary))
}

pub(super) fn scale_energy_permutation_curves(
    config: &AnalysisConfig,
    pattern: &Pattern,
    expected_len: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    if config.permutation.b == 0 || pattern.n_marked() == 0 || pattern.n_unmarked() == 0 {
        return Ok(None);
    }

    let cell_size_um = pattern.window.d_nn_mean_um.max(1.0);
    let mut curves = Vec::with_capacity(config.permutation.b);
    for perm_index in 0..config.permutation.b {
        let labels = permutation_labels(config, pattern, perm_index, SeedEndpoint::ScaleEnergy)?;
        let Some((spec, raster)) = centered_mark_raster_for_marks(pattern, &labels, cell_size_um)
        else {
            return Ok(None);
        };
        let Some(energies) = relative_scale_energies_from_field(&raster, spec.width, spec.height)
        else {
            return Ok(None);
        };
        let curve = vec![
            energies.local_difference,
            energies.residual,
            energies.block_mean,
        ];
        if curve.len() != expected_len || curve.iter().any(|value| !value.is_finite()) {
            return Ok(None);
        }
        curves.push(curve);
    }
    Ok(Some(curves))
}

pub(super) fn multiscale_residual_scalar_p_values(
    config: &AnalysisConfig,
    pattern: &Pattern,
    observed_block_mean_variance_fraction: f64,
    observed_territory_count: usize,
) -> Result<(
    crate::output::AnalysisSection<f64>,
    crate::output::AnalysisSection<f64>,
)> {
    let unavailable = || crate::output::AnalysisSection::InsufficientData {
        reason: "the required multiscale residual null statistic was undefined".into(),
    };
    if !config.multiscale_residual.enabled
        || pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || config.permutation.b == 0
    {
        return Ok((
            unavailable(),
            if config.multiscale_residual.territory_detection {
                unavailable()
            } else {
                crate::output::AnalysisSection::Disabled
            },
        ));
    }

    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    let block_mean_scale_um = pattern.window.l_eff_um.max(1.0) / 4.0;
    let block_mean_eligible = block_mean_scale_um <= max_scale_um;
    let territory_eligible = pattern.window.d_nn_mean_um.max(1.0) <= max_scale_um;

    let mut block_mean_null = block_mean_eligible.then(|| Vec::with_capacity(config.permutation.b));
    let mut block_mean_null_complete = block_mean_eligible;
    let mut territory_null = (config.multiscale_residual.territory_detection && territory_eligible)
        .then(|| Vec::with_capacity(config.permutation.b));
    for permutation_index in 0..config.permutation.b {
        let labels = permutation_labels(
            config,
            pattern,
            permutation_index,
            SeedEndpoint::ResidualTerritory,
        )?;
        let mut permuted = pattern.clone();
        permuted.mark = labels.into_boxed_slice();

        if block_mean_null_complete {
            match block_mean_variance_fraction_for(&permuted) {
                Some(block_mean_fraction) => block_mean_null
                    .as_mut()
                    .expect("eligible block-mean endpoint has null storage")
                    .push(block_mean_fraction),
                None => {
                    block_mean_null = None;
                    block_mean_null_complete = false;
                }
            }
        }
        if let Some(territory_null) = territory_null.as_mut() {
            territory_null.push(territories_for(config, &permuted).len() as f64);
        }
    }

    let territory_count_p_value = if !config.multiscale_residual.territory_detection {
        crate::output::AnalysisSection::Disabled
    } else if !territory_eligible {
        crate::output::AnalysisSection::InsufficientData {
            reason: format!(
                "no territory scale is within the maximum interpretable scale ({max_scale_um:.3} um)"
            ),
        }
    } else if let Some(territory_null) = territory_null {
        crate::output::AnalysisSection::available(permutation_p_value(
            observed_territory_count as f64,
            &territory_null,
            Tail::OneSidedHigh,
            config.inference.family_wise_alpha,
        )?)
    } else {
        unavailable()
    };

    let block_mean_variance_fraction_p_value = if !block_mean_eligible {
        crate::output::AnalysisSection::InsufficientData {
            reason: format!(
                "block-mean multiscale residual scale {block_mean_scale_um:.3} um exceeds the maximum interpretable scale {max_scale_um:.3} um"
            ),
        }
    } else if let Some(block_mean_null) = block_mean_null {
        crate::output::AnalysisSection::available(permutation_p_value(
            observed_block_mean_variance_fraction,
            &block_mean_null,
            Tail::OneSidedHigh,
            config.inference.family_wise_alpha,
        )?)
    } else {
        unavailable()
    };

    Ok((
        block_mean_variance_fraction_p_value,
        territory_count_p_value,
    ))
}

pub(super) fn block_mean_variance_fraction_for(pattern: &Pattern) -> Option<f64> {
    centered_mark_raster(pattern, pattern.window.d_nn_mean_um.max(1.0))
        .and_then(|(spec, raster)| {
            relative_scale_energies_from_field(&raster, spec.width, spec.height)
        })
        .map(|energies| energies.block_mean)
        .filter(|value| value.is_finite())
}

pub(super) fn estimated_raster_pixels(pattern: &Pattern) -> usize {
    let cell_size = pattern.window.d_nn_mean_um.max(1.0);
    let side = (pattern.window.l_eff_um.max(cell_size) / cell_size)
        .ceil()
        .max(1.0) as usize;
    side.saturating_mul(side).max(pattern.len())
}

pub(super) fn periodogram_disagrees_with_particle_spectrum(
    config: &AnalysisConfig,
    pattern: &Pattern,
    low_k_excess: f64,
) -> bool {
    let cell_size_um = pattern.window.d_nn_mean_um.max(1.0) * 0.5;
    let Some(periodogram) =
        hann_tapered_raster_periodogram(pattern, cell_size_um, config.spectrum.low_k_shells)
    else {
        return false;
    };

    let coarse_grid = periodogram.raster_width < 4 || periodogram.raster_height < 4;
    let low_k_mismatch = low_k_excess >= 1.25 && periodogram.normalized_low_k_power <= 0.75;
    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    let grid_exceeds_interpretable_scale = pattern.window.d_nn_mean_um >= max_scale_um;
    coarse_grid && (grid_exceeds_interpretable_scale || low_k_excess >= 1.10 || low_k_mismatch)
}

pub(super) fn territories_for(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> Vec<ResidualTerritory> {
    if !config.multiscale_residual.enabled || !config.multiscale_residual.territory_detection {
        return Vec::new();
    }

    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    detect_residual_territories(pattern, config.multiscale_residual.min_territory_z)
        .into_iter()
        .filter(|territory| territory.analysis_scale_um <= max_scale_um)
        .map(ResidualTerritory::from)
        .collect()
}

impl From<ResidualTerritoryCandidate> for ResidualTerritory {
    fn from(candidate: ResidualTerritoryCandidate) -> Self {
        Self {
            center_x_um: candidate.center_x_um,
            center_y_um: candidate.center_y_um,
            radius_um: candidate.radius_um,
            analysis_scale_um: candidate.analysis_scale_um,
            residual_score: candidate.residual_score,
            supporting_marked_cells: candidate.supporting_marked_cells,
            component_id: candidate.component_id,
            qc_overlap_fraction: candidate.qc_overlap_fraction,
        }
    }
}
