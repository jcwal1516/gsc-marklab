use super::*;

pub(super) fn pair_correlation_with_envelope(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> Result<(
    Vec<PairCorrelationPoint>,
    crate::output::AnalysisSection<FunctionalSummary>,
)> {
    let bin_width_um = pattern.window.d_nn_mean_um.max(1.0);
    let max_r_um =
        (pattern.window.l_eff_um * config.validation.largest_interpretable_scale_fraction).max(1.0);
    let Some(observed_bins) = pair_correlation(pattern, bin_width_um, max_r_um) else {
        return Ok((
            Vec::new(),
            crate::output::AnalysisSection::InsufficientData {
                reason: "pair correlation could not be estimated".into(),
            },
        ));
    };

    let observed_values = observed_bins
        .iter()
        .map(|bin| bin.value)
        .collect::<Vec<_>>();
    if observed_values.iter().any(|value| !value.is_finite()) {
        return Err(MarklabError::Compute(
            "observed pair-correlation curve contains a non-finite value".into(),
        ));
    }
    let permutation_curves = pair_correlation_permutation_curves(
        config,
        pattern,
        bin_width_um,
        max_r_um,
        observed_values.len(),
    )?;
    let envelope = match permutation_curves {
        Some(permutation_curves) => Some(GlobalEnvelope::from_curves(
            &observed_values,
            &permutation_curves,
            config.inference.family_wise_alpha,
        )?),
        None => None,
    };
    let summary = envelope.as_ref().map_or_else(
        || crate::output::AnalysisSection::InsufficientData {
            reason: "at least one required pair-correlation null curve was undefined".into(),
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
            let value = observed_values[index];
            let envelope_bounds = envelope.as_ref().and_then(|envelope| {
                let lower = envelope.lower.get(index).copied().and_then(finite_option)?;
                let upper = envelope.upper.get(index).copied().and_then(finite_option)?;
                Some((lower, upper))
            });
            if !bin.r_min_um.is_finite() || !bin.r_max_um.is_finite() {
                return Err(MarklabError::Compute(format!(
                    "pair-correlation bin {index} has non-finite bounds"
                )));
            }
            Ok(PairCorrelationPoint {
                r_min_um: bin.r_min_um,
                r_max_um: bin.r_max_um,
                value,
                inference_eligible: bin.r_max_um <= max_r_um,
                lower_global_envelope: envelope_bounds.map(|bounds| bounds.0),
                upper_global_envelope: envelope_bounds.map(|bounds| bounds.1),
                count: bin.count,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok((points, summary))
}

pub(super) fn pair_correlation_permutation_curves(
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
        let labels = permutation_labels(config, pattern, perm_index, 0x2d35_8dcc_aa6c_78a5)?;
        let Some(bins) = pair_correlation_for_marks(pattern, &labels, bin_width_um, max_r_um)
        else {
            return Ok(None);
        };
        if bins.len() != expected_len || bins.iter().any(|bin| !bin.value.is_finite()) {
            return Ok(None);
        }
        curves.push(bins.into_iter().map(|bin| bin.value).collect());
    }
    Ok(Some(curves))
}

pub(super) fn scalogram_with_envelope(
    config: &AnalysisConfig,
    pattern: &Pattern,
    fine_variance_fraction: f64,
    intermediate_variance_fraction: f64,
    coarse_variance_fraction: f64,
) -> Result<(
    Vec<ScalogramPoint>,
    crate::output::AnalysisSection<FunctionalSummary>,
)> {
    let observed_values = vec![
        fine_variance_fraction,
        intermediate_variance_fraction,
        coarse_variance_fraction,
    ];
    if observed_values.iter().any(|value| !value.is_finite()) {
        return Err(MarklabError::Compute(
            "observed scalogram contains a non-finite value".into(),
        ));
    }
    let permutation_curves = scalogram_permutation_curves(config, pattern, observed_values.len())?;
    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    let bands = [
        ("fine", pattern.window.d_nn_mean_um.max(1.0)),
        ("intermediate", pattern.window.d_nn_mean_um.max(1.0) * 2.0),
        ("coarse", pattern.window.l_eff_um.max(1.0) / 4.0),
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
            reason: "at least one required scalogram null curve was undefined".into(),
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
            ScalogramPoint {
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

pub(super) fn scalogram_permutation_curves(
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
        let labels = permutation_labels(config, pattern, perm_index, 0x8a5c_62d7_3d1f_4c0b)?;
        let Some((spec, raster)) = centered_mark_raster_for_marks(pattern, &labels, cell_size_um)
        else {
            return Ok(None);
        };
        let Some(fractions) = variance_fractions_from_field(&raster, spec.width, spec.height)
        else {
            return Ok(None);
        };
        let curve = vec![fractions.fine, fractions.intermediate, fractions.coarse];
        if curve.len() != expected_len || curve.iter().any(|value| !value.is_finite()) {
            return Ok(None);
        }
        curves.push(curve);
    }
    Ok(Some(curves))
}

pub(super) fn wavelet_scalar_p_values(
    config: &AnalysisConfig,
    pattern: &Pattern,
    observed_coarse_variance_fraction: f64,
    observed_territory_count: usize,
) -> Result<(
    crate::output::AnalysisSection<f64>,
    crate::output::AnalysisSection<f64>,
)> {
    let unavailable = || crate::output::AnalysisSection::InsufficientData {
        reason: "the required wavelet null statistic was undefined".into(),
    };
    if !config.wavelet.enabled
        || pattern.len() < 2
        || pattern.n_marked() == 0
        || pattern.n_unmarked() == 0
        || config.permutation.b == 0
    {
        return Ok((
            unavailable(),
            if config.wavelet.territory_detection {
                unavailable()
            } else {
                crate::output::AnalysisSection::Disabled
            },
        ));
    }

    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    let coarse_scale_um = pattern.window.l_eff_um.max(1.0) / 4.0;
    let coarse_eligible = coarse_scale_um <= max_scale_um;
    let territory_eligible = pattern.window.d_nn_mean_um.max(1.0) <= max_scale_um;

    let mut coarse_null = coarse_eligible.then(|| Vec::with_capacity(config.permutation.b));
    let mut coarse_null_complete = coarse_eligible;
    let mut territory_null = (config.wavelet.territory_detection && territory_eligible)
        .then(|| Vec::with_capacity(config.permutation.b));
    for permutation_index in 0..config.permutation.b {
        let labels = permutation_labels(config, pattern, permutation_index, 0xd6e8_feb8_6659_fd93)?;
        let mut permuted = pattern.clone();
        permuted.mark = labels.into_boxed_slice();

        if coarse_null_complete {
            match coarse_variance_fraction_for(&permuted) {
                Some(coarse_fraction) => coarse_null
                    .as_mut()
                    .expect("eligible coarse endpoint has null storage")
                    .push(coarse_fraction),
                None => {
                    coarse_null = None;
                    coarse_null_complete = false;
                }
            }
        }
        if let Some(territory_null) = territory_null.as_mut() {
            territory_null.push(territories_for(config, &permuted).len() as f64);
        }
    }

    let territory_count_p_value = if !config.wavelet.territory_detection {
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

    let coarse_variance_fraction_p_value = if !coarse_eligible {
        crate::output::AnalysisSection::InsufficientData {
            reason: format!(
                "coarse wavelet scale {coarse_scale_um:.3} um exceeds the maximum interpretable scale {max_scale_um:.3} um"
            ),
        }
    } else if let Some(coarse_null) = coarse_null {
        crate::output::AnalysisSection::available(permutation_p_value(
            observed_coarse_variance_fraction,
            &coarse_null,
            Tail::OneSidedHigh,
            config.inference.family_wise_alpha,
        )?)
    } else {
        unavailable()
    };

    Ok((coarse_variance_fraction_p_value, territory_count_p_value))
}

pub(super) fn coarse_variance_fraction_for(pattern: &Pattern) -> Option<f64> {
    centered_mark_raster(pattern, pattern.window.d_nn_mean_um.max(1.0))
        .and_then(|(spec, raster)| variance_fractions_from_field(&raster, spec.width, spec.height))
        .map(|fractions| fractions.coarse)
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
        marked_bartlett_periodogram(pattern, cell_size_um, config.spectrum.low_k_shells)
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

pub(super) fn territories_for(config: &AnalysisConfig, pattern: &Pattern) -> Vec<TerritoryFeature> {
    if !config.wavelet.enabled || !config.wavelet.territory_detection {
        return Vec::new();
    }

    let max_scale_um =
        config.validation.largest_interpretable_scale_fraction * pattern.window.l_eff_um;
    detect_residual_territories(pattern, config.wavelet.min_territory_z)
        .into_iter()
        .filter(|territory| territory.scale_um <= max_scale_um)
        .map(TerritoryFeature::from)
        .collect()
}

impl From<CandidateTerritory> for TerritoryFeature {
    fn from(candidate: CandidateTerritory) -> Self {
        Self {
            center_x_um: candidate.center_x_um,
            center_y_um: candidate.center_y_um,
            radius_um: candidate.radius_um,
            scale_um: candidate.scale_um,
            z_or_power: candidate.z_or_power,
            supporting_cells: candidate.supporting_cells,
            component_id: candidate.component_id,
            qc_overlap_fraction: candidate.qc_overlap_fraction,
        }
    }
}
