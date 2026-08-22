use crate::{
    api::{finite_option, qc_pipeline::permutation_labels},
    common::seeds::SeedEndpoint,
    config::AnalysisConfig,
    data::Pattern,
    errors::{MarklabError, Result},
    geom::length_scales::{
        analysis_effective_length_um, maximum_interpretable_scale_for_points_um,
    },
    inference::scalar_pvalues::{permutation_p_value, Tail},
    multiscale_residual::{
        energy::relative_scale_energies_from_field,
        territories::{ResidualTerritoryCandidate, ResidualTerritoryPlan},
    },
    output::{
        AnalysisSection, FunctionalSummary, ResidualTerritory, ScaleEnergyBand, ScaleEnergyPoint,
    },
    periodogram::raster::centered_mark_raster_for_marks,
    permutation::envelopes::GlobalEnvelope,
};

pub(super) fn scale_energy_with_envelope(
    config: &AnalysisConfig,
    pattern: &Pattern,
    local_difference_energy_fraction: f64,
    residual_energy_fraction: f64,
    block_mean_variance_fraction: f64,
) -> Result<(Vec<ScaleEnergyPoint>, AnalysisSection<FunctionalSummary>)> {
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
    let max_scale_um = maximum_interpretable_scale_for_points_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
        config.validation.largest_interpretable_scale_fraction,
    )
    .unwrap_or(0.0);
    let analysis_effective_length_um = analysis_effective_length_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
    )
    .unwrap_or(0.0);
    let bands = [
        (
            ScaleEnergyBand::LocalDifference,
            pattern.window.d_nn_mean_um.max(1.0),
        ),
        (
            ScaleEnergyBand::Residual,
            pattern.window.d_nn_mean_um.max(1.0) * 2.0,
        ),
        (
            ScaleEnergyBand::BlockMean,
            analysis_effective_length_um.max(1.0) / 4.0,
        ),
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
        || AnalysisSection::InsufficientData {
            reason: "at least one required scale-energy null curve was undefined".into(),
        },
        |envelope| {
            AnalysisSection::available(FunctionalSummary {
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
                band: bands[index].0,
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

fn scale_energy_permutation_curves(
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
    territory_plan: Option<&ResidualTerritoryPlan>,
    observed_block_mean_variance_fraction: f64,
    observed_territory_count: usize,
) -> Result<(AnalysisSection<f64>, AnalysisSection<f64>)> {
    let unavailable = || AnalysisSection::InsufficientData {
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
                AnalysisSection::Disabled
            },
        ));
    }

    let max_scale_um = maximum_interpretable_scale_for_points_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
        config.validation.largest_interpretable_scale_fraction,
    )
    .unwrap_or(0.0);
    let analysis_effective_length_um = analysis_effective_length_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
    )
    .unwrap_or(0.0);
    let block_mean_scale_um = analysis_effective_length_um.max(1.0) / 4.0;
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
        if block_mean_null_complete {
            match block_mean_variance_fraction_for_marks(pattern, &labels) {
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
            let plan = territory_plan.ok_or_else(|| {
                MarklabError::Compute(
                    "eligible residual territory inference requires a geometry plan".into(),
                )
            })?;
            territory_null.push(territories_for(config, pattern, plan, &labels)?.len() as f64);
        }
    }

    let territory_count_p_value = if !config.multiscale_residual.territory_detection {
        AnalysisSection::Disabled
    } else if !territory_eligible {
        AnalysisSection::InsufficientData {
            reason: format!(
                "no territory scale is within the maximum interpretable scale ({max_scale_um:.3} um)"
            ),
        }
    } else if let Some(territory_null) = territory_null {
        AnalysisSection::available(permutation_p_value(
            observed_territory_count as f64,
            &territory_null,
            Tail::OneSidedHigh,
            config.inference.family_wise_alpha,
        )?)
    } else {
        unavailable()
    };

    let block_mean_variance_fraction_p_value = if !block_mean_eligible {
        AnalysisSection::InsufficientData {
            reason: format!(
                "block-mean multiscale residual scale {block_mean_scale_um:.3} um exceeds the maximum interpretable scale {max_scale_um:.3} um"
            ),
        }
    } else if let Some(block_mean_null) = block_mean_null {
        AnalysisSection::available(permutation_p_value(
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

fn block_mean_variance_fraction_for_marks(pattern: &Pattern, marks: &[u8]) -> Option<f64> {
    centered_mark_raster_for_marks(pattern, marks, pattern.window.d_nn_mean_um.max(1.0))
        .and_then(|(spec, raster)| {
            relative_scale_energies_from_field(&raster, spec.width, spec.height)
        })
        .map(|energies| energies.block_mean)
        .filter(|value| value.is_finite())
}

pub(super) fn territories_for(
    config: &AnalysisConfig,
    pattern: &Pattern,
    plan: &ResidualTerritoryPlan,
    marks: &[u8],
) -> Result<Vec<ResidualTerritory>> {
    if !config.multiscale_residual.enabled || !config.multiscale_residual.territory_detection {
        return Ok(Vec::new());
    }

    let max_scale_um = maximum_interpretable_scale_for_points_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
        config.validation.largest_interpretable_scale_fraction,
    )
    .unwrap_or(0.0);
    Ok(plan
        .detect_for_marks(pattern, marks, config.multiscale_residual.min_territory_z)?
        .into_iter()
        .filter(|territory| territory.analysis_scale_um <= max_scale_um)
        .map(ResidualTerritory::from)
        .collect())
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
        }
    }
}
