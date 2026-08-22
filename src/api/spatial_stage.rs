use crate::{
    config::AnalysisConfig,
    data::Pattern,
    errors::Result,
    multiscale_residual::energy::relative_scale_energies_from_field,
    output::{
        AnalysisSection, FunctionalSummary, MarkPairCovariancePoint, MultiscaleResidualSummary,
        ResidualTerritory, ScaleEnergyPoint, TimingStage,
    },
    periodogram::raster::centered_mark_raster,
    spectra::anisotropy::{permutation_whitened_anisotropy, PermutationAnisotropy},
};

use super::{
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
}

pub(super) fn run(
    config: &AnalysisConfig,
    pattern: &Pattern,
    includes_pooled: bool,
    configured_strata: Option<&[u32]>,
    low_k_excess: Option<f64>,
    timings: &mut Vec<TimingStage>,
    threads: usize,
) -> Result<Output> {
    let periodogram_artifact = timed_stage(timings, "periodogram", threads, || {
        config.periodogram.enabled
            && low_k_excess.is_some_and(|value| {
                periodogram_disagrees_with_particle_spectrum(config, pattern, value)
            })
    });
    let (mark_pair_covariance_curve, mark_pair_covariance) =
        timed_stage(timings, "mark_pair_covariance", threads, || -> Result<_> {
            if includes_pooled {
                mark_pair_covariance_with_envelope(config, pattern)
            } else {
                Ok((Vec::new(), AnalysisSection::NotApplicable))
            }
        })?;
    let territories = timed_stage(timings, "multiscale_residual", threads, || {
        if includes_pooled {
            territories_for(config, pattern)
        } else {
            Vec::new()
        }
    });
    let anisotropy = timed_stage(timings, "anisotropy", threads, || -> Result<_> {
        if includes_pooled {
            permutation_whitened_anisotropy(
                pattern,
                config.spectrum.anisotropy_low_k_shells,
                config.permutation.b,
                config.permutation.seed,
                config.inference.family_wise_alpha,
                configured_strata,
            )
        } else {
            Ok(None)
        }
    })?;
    let (multiscale_residual, scale_energy_curve, scale_energy) =
        timed_stage(timings, "multiscale_residual_energy", threads, || {
            multiscale_analysis(config, pattern, includes_pooled, territories.len())
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
    })
}

fn multiscale_analysis(
    config: &AnalysisConfig,
    pattern: &Pattern,
    includes_pooled: bool,
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
        multiscale_residual_scalar_p_values(config, pattern, energies.block_mean, territory_count)?;
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
