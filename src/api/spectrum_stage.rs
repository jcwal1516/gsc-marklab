use crate::{
    config::AnalysisConfig,
    data::Pattern,
    errors::Result,
    output::{StatusFlag, TimingStage},
    spectra::structure_factor::{
        observed_power_for_modes, observed_value_power_for_modes,
        permutation_whitened_spectrum_from_observed_modes,
        permutation_whitened_value_spectrum_from_observed_modes, resolvable_modes_for_pattern,
        stratified_permutation_whitened_spectrum_from_observed_modes, PermutationWhitenedSpectrum,
        SpectrumPermutationOptions,
    },
};

use super::{
    qc_pipeline::{
        spectrum_null_sensitivity, strata_are_mark_homogeneous, ConfoundingConclusion,
        SpectrumNullSensitivity,
    },
    timed_stage,
};

pub(super) struct Output {
    pub(super) spectrum: Option<PermutationWhitenedSpectrum>,
    pub(super) null_sensitivity: Option<SpectrumNullSensitivity>,
    pub(super) unavailable_reason: Option<String>,
}

pub(super) fn run(
    config: &AnalysisConfig,
    pattern: &Pattern,
    includes_pooled: bool,
    configured_strata: Option<&[u32]>,
    timings: &mut Vec<TimingStage>,
    threads: usize,
) -> Result<Output> {
    let modes = timed_stage(timings, "kgrid", threads, || {
        includes_pooled
            .then(|| resolvable_modes_for_pattern(pattern, config.spectrum.k_shells))
            .flatten()
            .unwrap_or_default()
    });
    let observed_mode_power = timed_stage(timings, "structure_factor_observed", threads, || {
        if !includes_pooled {
            None
        } else if config.analysis.use_probabilistic_marks {
            pattern.mark_prob.as_deref().and_then(|values| {
                let values = values.iter().copied().map(f64::from).collect::<Vec<_>>();
                observed_value_power_for_modes(pattern, &values, &modes)
            })
        } else {
            Some(observed_power_for_modes(pattern, &modes))
        }
    });
    timed_stage(timings, "permutation_spectra", threads, || {
        execute_permutations(
            config,
            pattern,
            includes_pooled,
            configured_strata,
            &modes,
            observed_mode_power,
        )
    })
}

pub(super) fn apply_null_sensitivity_status(
    sensitivity: Option<SpectrumNullSensitivity>,
    status_flags: &mut Vec<StatusFlag>,
) {
    let Some(sensitivity) = sensitivity else {
        return;
    };
    tracing::debug!(
        unstratified_p_global = ?sensitivity
            .unstratified
            .map(|inference| inference.p_global),
        unstratified_low_k_p = ?sensitivity
            .unstratified
            .and_then(|inference| inference.low_k_excess_p_value),
        stratified_p_global = ?sensitivity.stratified.map(|inference| inference.p_global),
        stratified_low_k_p = ?sensitivity
            .stratified
            .and_then(|inference| inference.low_k_excess_p_value),
        conclusion = ?sensitivity.conclusion,
        "spectrum null-model sensitivity"
    );
    match sensitivity.conclusion {
        ConfoundingConclusion::ConfoundedBySpatialStrata => {
            status_flags.push(StatusFlag::ConfoundedBySpatialStrata);
        }
        ConfoundingConclusion::DegenerateStratifiedNull => {
            status_flags.push(StatusFlag::DegenerateSpatialStrataNull);
        }
        ConfoundingConclusion::BothSignificant
        | ConfoundingConclusion::NoUnstratifiedSignal
        | ConfoundingConclusion::NotEvaluable => {}
    }
}

fn execute_permutations(
    config: &AnalysisConfig,
    pattern: &Pattern,
    includes_pooled: bool,
    configured_strata: Option<&[u32]>,
    modes: &[crate::spectra::kgrid::KMode],
    observed_mode_power: Option<Vec<f64>>,
) -> Result<Output> {
    if !includes_pooled {
        return Ok(Output {
            spectrum: None,
            null_sensitivity: None,
            unavailable_reason: None,
        });
    }
    let options = permutation_options(config, pattern);
    if config.analysis.use_probabilistic_marks {
        let Some(values) = pattern.mark_prob.as_deref() else {
            return Ok(unavailable());
        };
        let values = values.iter().copied().map(f64::from).collect::<Vec<_>>();
        let Some(observed_mode_power) = observed_mode_power else {
            return Ok(unavailable());
        };
        let spectrum = permutation_whitened_value_spectrum_from_observed_modes(
            pattern,
            &values,
            modes,
            observed_mode_power,
            options,
        )?;
        return Ok(Output {
            spectrum,
            null_sensitivity: None,
            unavailable_reason: None,
        });
    }

    let Some(observed_mode_power) = observed_mode_power else {
        return Ok(unavailable());
    };
    let Some(strata) = configured_strata else {
        let spectrum = permutation_whitened_spectrum_from_observed_modes(
            pattern,
            modes,
            observed_mode_power,
            options,
        )?;
        return Ok(Output {
            spectrum,
            null_sensitivity: None,
            unavailable_reason: None,
        });
    };

    let stratified_degenerate = strata_are_mark_homogeneous(&pattern.mark, strata);
    let unstratified = permutation_whitened_spectrum_from_observed_modes(
        pattern,
        modes,
        observed_mode_power.clone(),
        options,
    )?;
    let stratified = if stratified_degenerate {
        None
    } else {
        stratified_permutation_whitened_spectrum_from_observed_modes(
            pattern,
            strata,
            modes,
            observed_mode_power,
            options,
        )?
    };
    let sensitivity = spectrum_null_sensitivity(
        unstratified.as_ref(),
        stratified.as_ref(),
        config.inference.family_wise_alpha,
        stratified_degenerate,
    );
    let unavailable_reason = stratified_degenerate.then(|| {
        "stratified spectrum null is degenerate because every configured stratum is mark-homogeneous"
            .to_string()
    });
    Ok(Output {
        spectrum: stratified,
        null_sensitivity: Some(sensitivity),
        unavailable_reason,
    })
}

fn permutation_options(config: &AnalysisConfig, pattern: &Pattern) -> SpectrumPermutationOptions {
    SpectrumPermutationOptions {
        n_shells: config.spectrum.k_shells,
        low_k_modes: config.spectrum.low_k_shells,
        n_permutations: config.permutation.b,
        seed: config.permutation.seed,
        family_wise_alpha: config.inference.family_wise_alpha,
        max_scale_um: config.validation.largest_interpretable_scale_fraction
            * pattern.window.l_eff_um,
        k_shell_min: config.validation.k_shell_min,
    }
}

fn unavailable() -> Output {
    Output {
        spectrum: None,
        null_sensitivity: None,
        unavailable_reason: None,
    }
}
