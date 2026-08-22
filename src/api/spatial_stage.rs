use crate::{
    config::AnalysisConfig,
    data::Pattern,
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multiscale_residual::energy::relative_scale_energies_from_field,
    multiscale_residual::territories::ResidualTerritoryPlan,
    output::{
        AnalysisSection, FunctionalSummary, MarkPairCovariancePoint, MultiscaleResidualSummary,
        ResidualTerritory, ScaleEnergyPoint, TimingStage,
    },
    perf::counters::enforce_storage_budget,
    periodogram::raster::centered_mark_raster,
    spectra::anisotropy::{
        permutation_whitened_anisotropy, AnisotropyPermutationOptions, PermutationAnisotropy,
    },
};

use super::{
    context::MarkedAnalysisContext,
    stages::{
        mark_pair_covariance_with_envelope, multiscale_residual_scalar_p_values,
        periodogram_disagrees_with_particle_spectrum, scale_energy_with_envelope, territories_for,
    },
    timed_stage,
};

pub(super) struct Output {
    pub(super) periodogram_artifact: bool,
    pub(super) mark_pair_covariance: AnalysisSection<FunctionalSummary>,
    pub(super) mark_pair_covariance_curve: Vec<MarkPairCovariancePoint>,
    pub(super) anisotropy: Option<PermutationAnisotropy>,
    pub(super) multiscale_residual: AnalysisSection<MultiscaleResidualSummary>,
    pub(super) scale_energy: AnalysisSection<FunctionalSummary>,
    pub(super) scale_energy_curve: Vec<ScaleEnergyPoint>,
    pub(super) territories: Vec<ResidualTerritory>,
    pub(super) estimated_geometry_storage_bytes: usize,
}

pub(super) struct ExecutionContext<'a> {
    pub(super) geometry_budget_bytes: usize,
    pub(super) timings: &'a mut Vec<TimingStage>,
    pub(super) threads: usize,
}

pub(super) fn run(
    config: &AnalysisConfig,
    analysis_context: &MarkedAnalysisContext<'_>,
    includes_pooled: bool,
    configured_strata: Option<&[u32]>,
    low_k_excess: Option<f64>,
    context: ExecutionContext<'_>,
) -> Result<Output> {
    let pattern = analysis_context.pattern();
    let ExecutionContext {
        geometry_budget_bytes,
        timings,
        threads,
    } = context;
    let spatial_index = timed_stage(timings, "spatial_index", threads, || -> Result<_> {
        if !includes_pooled {
            return Ok(None);
        }
        enforce_storage_budget(
            "spatial index",
            SpatialIndex2D::estimated_storage_bytes_for_len(pattern.len()),
            geometry_budget_bytes,
        )?;
        Ok(Some(SpatialIndex2D::new(&pattern.x_um, &pattern.y_um)?))
    })?;
    let periodogram_artifact = timed_stage(timings, "periodogram", threads, || {
        config.periodogram.enabled
            && low_k_excess.is_some_and(|value| {
                periodogram_disagrees_with_particle_spectrum(config, pattern, value)
            })
    });
    let (mark_pair_covariance_curve, mark_pair_covariance, pair_geometry_storage_bytes) =
        timed_stage(timings, "mark_pair_covariance", threads, || -> Result<_> {
            if includes_pooled {
                let index = spatial_index.as_ref().ok_or_else(|| {
                    MarklabError::Compute(
                        "pooled mark-pair covariance requires a spatial index".into(),
                    )
                })?;
                mark_pair_covariance_with_envelope(config, pattern, index, geometry_budget_bytes)
            } else {
                Ok((Vec::new(), AnalysisSection::NotApplicable, 0))
            }
        })?;
    let (territory_plan, territories, territory_geometry_storage_bytes) =
        timed_stage(timings, "multiscale_residual", threads, || -> Result<_> {
            if includes_pooled
                && config.multiscale_residual.enabled
                && config.multiscale_residual.territory_detection
            {
                let index = spatial_index.as_ref().ok_or_else(|| {
                    MarklabError::Compute(
                        "pooled residual territories require a spatial index".into(),
                    )
                })?;
                let max_scale_um = config.validation.largest_interpretable_scale_fraction
                    * pattern.window.l_eff_um;
                let index_storage_bytes = index.estimated_storage_bytes();
                let plan = ResidualTerritoryPlan::new_with_index(
                    pattern,
                    index,
                    max_scale_um,
                    geometry_budget_bytes.saturating_sub(index_storage_bytes),
                )?;
                let territories = territories_for(config, pattern, &plan, &pattern.mark)?;
                let storage_bytes =
                    index_storage_bytes.saturating_add(plan.estimated_storage_bytes());
                Ok((Some(plan), territories, storage_bytes))
            } else {
                Ok((
                    None,
                    Vec::new(),
                    spatial_index
                        .as_ref()
                        .map_or(0, SpatialIndex2D::estimated_storage_bytes),
                ))
            }
        })?;
    let anisotropy = timed_stage(timings, "anisotropy", threads, || -> Result<_> {
        if includes_pooled {
            permutation_whitened_anisotropy(
                pattern,
                configured_strata,
                AnisotropyPermutationOptions {
                    low_k_radius: config.spectrum.anisotropy_low_k_shells,
                    n_permutations: config.permutation.b,
                    seed: config.permutation.seed,
                    alpha: config.inference.family_wise_alpha,
                    k_chunk_modes: config.performance.k_chunk_modes,
                    n_marked: analysis_context.n_marked(),
                },
            )
        } else {
            Ok(None)
        }
    })?;
    let (multiscale_residual, scale_energy_curve, scale_energy) =
        timed_stage(timings, "multiscale_residual_energy", threads, || {
            multiscale_analysis(
                config,
                pattern,
                includes_pooled,
                territory_plan.as_ref(),
                territories.len(),
            )
        })?;

    Ok(Output {
        periodogram_artifact,
        mark_pair_covariance,
        mark_pair_covariance_curve,
        anisotropy,
        multiscale_residual,
        scale_energy,
        scale_energy_curve,
        territories,
        estimated_geometry_storage_bytes: pair_geometry_storage_bytes
            .max(territory_geometry_storage_bytes),
    })
}

fn multiscale_analysis(
    config: &AnalysisConfig,
    pattern: &Pattern,
    includes_pooled: bool,
    territory_plan: Option<&ResidualTerritoryPlan>,
    territory_count: usize,
) -> Result<(
    AnalysisSection<MultiscaleResidualSummary>,
    Vec<ScaleEnergyPoint>,
    AnalysisSection<FunctionalSummary>,
)> {
    if !includes_pooled {
        return Ok((
            AnalysisSection::NotApplicable,
            Vec::new(),
            AnalysisSection::NotApplicable,
        ));
    }
    if !config.multiscale_residual.enabled {
        return Ok((
            AnalysisSection::Disabled,
            Vec::new(),
            AnalysisSection::Disabled,
        ));
    }
    let relative_scale_energies =
        centered_mark_raster(pattern, pattern.window.d_nn_mean_um.max(1.0)).and_then(
            |(spec, raster)| relative_scale_energies_from_field(&raster, spec.width, spec.height),
        );
    let Some(energies) = relative_scale_energies.filter(|energies| {
        [
            energies.local_difference,
            energies.residual,
            energies.block_mean,
        ]
        .iter()
        .all(|value| value.is_finite())
    }) else {
        let reason = "multiscale residual scale energies could not be estimated".to_string();
        return Ok((
            AnalysisSection::InsufficientData {
                reason: reason.clone(),
            },
            Vec::new(),
            AnalysisSection::InsufficientData { reason },
        ));
    };

    let block_mean_to_local_difference_ratio = (energies.local_difference > 0.0)
        .then_some(energies.block_mean / energies.local_difference)
        .filter(|value| value.is_finite());
    let (curve, scale_energy) = scale_energy_with_envelope(
        config,
        pattern,
        energies.local_difference,
        energies.residual,
        energies.block_mean,
    )?;
    let (block_mean_variance_fraction_p_value, territory_count_p_value) =
        multiscale_residual_scalar_p_values(
            config,
            pattern,
            territory_plan,
            energies.block_mean,
            territory_count,
        )?;
    Ok((
        AnalysisSection::available(MultiscaleResidualSummary {
            local_difference_energy_fraction: energies.local_difference,
            residual_energy_fraction: energies.residual,
            block_mean_variance_fraction: energies.block_mean,
            block_mean_to_local_difference_ratio,
            territory_count,
            block_mean_variance_fraction_p_value,
            territory_count_p_value,
        }),
        curve,
        scale_energy,
    ))
}
