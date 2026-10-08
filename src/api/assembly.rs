use crate::{
    api::{
        components::{component_results_for, ComponentAnalysisPlan},
        finite_option,
        qc_pipeline::{
            qc_summary, ConfoundingConclusion, SpectrumInference, SpectrumNullSensitivity,
        },
    },
    config::AnalysisConfig,
    errors::{MarklabError, Result},
    output::{
        AnalysisSection, AnalysisStatus, AnisotropySummary, DiagnosticsResult, FunctionalSummary,
        Interpretation, MarkPairCovariancePoint, MarkedPatternResult, MultiscaleResidualSummary,
        PrimaryEndpoint, PrimaryEndpointKind, ResidualTerritory, ScaleEnergyPoint,
        SpectrumConfoundingConclusion, SpectrumNullInferenceSummary, SpectrumNullModel,
        SpectrumNullSensitivitySummary, SpectrumPoint, SpectrumSummary, StatusFlag, TimingStage,
        WindowSummary,
    },
    spectra::{anisotropy::PermutationAnisotropy, structure_factor::PermutationWhitenedSpectrum},
};

use super::context::MarkedAnalysisContext;

pub(super) struct Inputs {
    pub(super) status: AnalysisStatus,
    pub(super) status_flags: Vec<StatusFlag>,
    pub(super) spectrum: Option<PermutationWhitenedSpectrum>,
    pub(super) spectrum_null_sensitivity: Option<SpectrumNullSensitivity>,
    pub(super) spectrum_unavailable_reason: Option<String>,
    pub(super) mark_pair_covariance: crate::output::AnalysisSection<FunctionalSummary>,
    pub(super) mark_pair_covariance_curve: Vec<MarkPairCovariancePoint>,
    pub(super) anisotropy: Option<PermutationAnisotropy>,
    pub(super) multiscale_residual: crate::output::AnalysisSection<MultiscaleResidualSummary>,
    pub(super) scale_energy: crate::output::AnalysisSection<FunctionalSummary>,
    pub(super) scale_energy_curve: Vec<ScaleEnergyPoint>,
    pub(super) territories: Vec<ResidualTerritory>,
    pub(super) diagnostics: crate::output::AnalysisSection<DiagnosticsResult>,
    pub(super) timings: Vec<TimingStage>,
    pub(super) interpretation: Interpretation,
    pub(super) component_plan: ComponentAnalysisPlan,
}

struct SpectrumAssembly {
    primary_endpoint: PrimaryEndpoint,
    spectrum: AnalysisSection<SpectrumSummary>,
    null_sensitivity: AnalysisSection<SpectrumNullSensitivitySummary>,
    curve: Vec<SpectrumPoint>,
}

pub(super) fn assemble(
    config: &AnalysisConfig,
    context: &MarkedAnalysisContext<'_>,
    inputs: Inputs,
) -> Result<MarkedPatternResult> {
    let pattern = context.pattern();
    let geometry = context.geometry();
    let Inputs {
        status,
        status_flags,
        spectrum,
        spectrum_null_sensitivity,
        spectrum_unavailable_reason,
        mark_pair_covariance,
        mark_pair_covariance_curve,
        anisotropy,
        multiscale_residual,
        scale_energy,
        scale_energy_curve,
        territories,
        diagnostics,
        timings,
        interpretation,
        component_plan,
    } = inputs;
    let includes_pooled = component_plan.includes_pooled();

    let spectrum_assembly = assemble_spectrum(
        config,
        geometry.analysis_effective_length_um,
        spectrum.as_ref(),
        spectrum_null_sensitivity,
        spectrum_unavailable_reason,
        includes_pooled,
    )?;

    let mut result = MarkedPatternResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        protein: pattern.meta.protein.clone(),
        mark_label: config.analysis.mark_label.clone(),
        status,
        status_flags,
        n_cells: context.n_cells(),
        n_marked: context.n_marked(),
        p_hat: context.prevalence(),
        window: WindowSummary {
            area_um2: geometry.area_um2,
            analysis_effective_length_um: geometry.analysis_effective_length_um,
            d_nn_mean_um: geometry.mean_nearest_neighbor_um,
        },
        qc: qc_summary(pattern),
        primary_endpoint: spectrum_assembly.primary_endpoint,
        spectrum: spectrum_assembly.spectrum,
        spectrum_null_sensitivity: spectrum_assembly.null_sensitivity,
        spectrum_curve: spectrum_assembly.curve,
        mark_pair_covariance,
        mark_pair_covariance_curve,
        anisotropy: anisotropy.map_or_else(
            || crate::output::AnalysisSection::InsufficientData {
                reason: "anisotropy could not be estimated from the eligible modes".into(),
            },
            |anisotropy| {
                crate::output::AnalysisSection::available(AnisotropySummary {
                    index: anisotropy.readout.index,
                    theta_deg: anisotropy.readout.theta_deg.and_then(finite_option),
                    p_value: finite_option(anisotropy.p_value),
                })
            },
        ),
        multiscale_residual,
        scale_energy,
        scale_energy_curve,
        residual_territories: if config.multiscale_residual.enabled {
            if config.multiscale_residual.territory_detection {
                crate::output::AnalysisSection::available(territories)
            } else {
                crate::output::AnalysisSection::Disabled
            }
        } else {
            crate::output::AnalysisSection::Disabled
        },
        component_mode_selection: component_plan.selection.clone(),
        component_results: component_results_for(config, pattern, &component_plan)?,
        diagnostics,
        timings,
        interpretation,
    };
    apply_component_mode(&mut result, includes_pooled);
    Ok(result)
}

fn assemble_spectrum(
    config: &AnalysisConfig,
    analysis_effective_length_um: f64,
    spectrum: Option<&PermutationWhitenedSpectrum>,
    sensitivity: Option<SpectrumNullSensitivity>,
    unavailable_reason: Option<String>,
    includes_pooled: bool,
) -> Result<SpectrumAssembly> {
    let p_global = spectrum.and_then(|value| finite_option(value.p_global));
    let xi_um = spectrum
        .and_then(|value| value.xi_um)
        .and_then(finite_option);
    let xi_stability_interval_um = spectrum
        .and_then(|value| value.xi_stability_interval_um)
        .filter(|interval| interval.iter().all(|value| value.is_finite()));
    let alpha = config
        .spectrum
        .fit_low_k_alpha
        .then(|| {
            spectrum
                .and_then(|value| value.alpha)
                .and_then(finite_option)
        })
        .flatten();
    let low_k_excess_p_value = spectrum
        .and_then(|value| value.low_k_excess_p_value)
        .and_then(finite_option);
    let xi_um_p_value = spectrum
        .and_then(|value| value.xi_um_p_value)
        .and_then(finite_option);
    let alpha_p_value = config
        .spectrum
        .fit_low_k_alpha
        .then(|| {
            spectrum
                .and_then(|value| value.alpha_p_value)
                .and_then(finite_option)
        })
        .flatten();
    let k_min = spectrum
        .and_then(|value| value.k_values.first().copied())
        .and_then(finite_option);
    let k_max = spectrum
        .and_then(|value| value.k_values.last().copied())
        .and_then(finite_option);
    let n_k_modes = spectrum.map_or(0, |value| value.n_modes);
    let n_permutations = spectrum.map_or(0, |value| value.n_permutations);
    let curve = spectrum
        .map(spectrum_curve)
        .transpose()?
        .unwrap_or_default();

    let primary_endpoint = PrimaryEndpoint {
        name: PrimaryEndpointKind::LowKExcess,
        value: spectrum.map_or_else(
            || AnalysisSection::InsufficientData {
                reason: unavailable_reason.clone().unwrap_or_else(|| {
                    "too few eligible spectrum shells or invalid spectrum input".into()
                }),
            },
            |value| AnalysisSection::available(value.low_k_excess),
        ),
        p_value: low_k_excess_p_value.map_or_else(
            || AnalysisSection::InsufficientData {
                reason: unavailable_reason
                    .clone()
                    .unwrap_or_else(|| "the low-k null statistic was unavailable".into()),
            },
            AnalysisSection::available,
        ),
        null: if config.permutation.stratified {
            SpectrumNullModel::StratifiedFixedPositionRandomLabeling
        } else {
            SpectrumNullModel::FixedPositionRandomLabeling
        },
    };
    let spectrum_section = spectrum.map_or_else(
        || AnalysisSection::InsufficientData {
            reason: unavailable_reason.unwrap_or_else(|| {
                format!(
                    "fewer than {} inference-eligible spectrum shells or undefined spectrum input",
                    config.validation.k_shell_min
                )
            }),
        },
        |value| {
            AnalysisSection::available(SpectrumSummary {
                max_interpretable_scale_um:
                    crate::geom::length_scales::maximum_interpretable_scale_um(
                        analysis_effective_length_um,
                        config.validation.largest_interpretable_scale_fraction,
                    )
                    .unwrap_or(0.0),
                k_min,
                k_max,
                n_k_modes,
                n_shells: config.spectrum.k_shells,
                n_permutations,
                spectral_curve_test: p_global.map_or_else(
                    || AnalysisSection::InsufficientData {
                        reason: "the spectral-curve ERL test was unavailable".into(),
                    },
                    |p_global| {
                        AnalysisSection::available(FunctionalSummary {
                            p_global: Some(p_global),
                            erl_depth: Some(value.erl_depth),
                            n_permutations,
                        })
                    },
                ),
                xi_um,
                xi_stability_interval_um,
                low_k_excess: value.low_k_excess,
                low_k_excess_p_value,
                alpha,
                xi_um_p_value,
                alpha_p_value,
            })
        },
    );

    Ok(SpectrumAssembly {
        primary_endpoint,
        spectrum: spectrum_section,
        null_sensitivity: spectrum_null_sensitivity_section(config, includes_pooled, sensitivity),
        curve,
    })
}

fn apply_component_mode(result: &mut MarkedPatternResult, includes_pooled: bool) {
    if includes_pooled {
        return;
    }
    result.primary_endpoint = PrimaryEndpoint {
        name: PrimaryEndpointKind::ComponentLowKExcess,
        value: AnalysisSection::NotApplicable,
        p_value: AnalysisSection::NotApplicable,
        null: SpectrumNullModel::ComponentSpecificFixedPositionRandomLabeling,
    };
    result.spectrum = AnalysisSection::NotApplicable;
    result.spectrum_curve.clear();
    result.mark_pair_covariance = AnalysisSection::NotApplicable;
    result.mark_pair_covariance_curve.clear();
    result.anisotropy = AnalysisSection::NotApplicable;
    result.multiscale_residual = AnalysisSection::NotApplicable;
    result.scale_energy = AnalysisSection::NotApplicable;
    result.scale_energy_curve.clear();
    result.residual_territories = AnalysisSection::NotApplicable;
}

fn spectrum_null_sensitivity_section(
    config: &AnalysisConfig,
    includes_pooled: bool,
    sensitivity: Option<SpectrumNullSensitivity>,
) -> crate::output::AnalysisSection<SpectrumNullSensitivitySummary> {
    if !includes_pooled || !config.permutation.stratified {
        return crate::output::AnalysisSection::NotApplicable;
    }
    let Some(sensitivity) = sensitivity else {
        return crate::output::AnalysisSection::InsufficientData {
            reason: "the requested spectrum null-model sensitivity was unavailable".into(),
        };
    };

    let unavailable = |reason: &str| crate::output::AnalysisSection::InsufficientData {
        reason: reason.into(),
    };
    let inference = |value: SpectrumInference| {
        crate::output::AnalysisSection::available(SpectrumNullInferenceSummary {
            p_global: value.p_global,
            low_k_excess_p_value: value.low_k_excess_p_value,
        })
    };
    let stratified_unavailable_reason = match sensitivity.conclusion {
        ConfoundingConclusion::DegenerateStratifiedNull => {
            "the stratified null is degenerate because every configured stratum is mark-homogeneous"
        }
        _ => "the stratified spectrum inference was unavailable",
    };

    crate::output::AnalysisSection::available(SpectrumNullSensitivitySummary {
        primary_null: SpectrumNullModel::StratifiedFixedPositionRandomLabeling,
        family_wise_alpha: config.inference.family_wise_alpha,
        unstratified: sensitivity.unstratified.map_or_else(
            || unavailable("the unstratified spectrum inference was unavailable"),
            inference,
        ),
        stratified: sensitivity
            .stratified
            .map_or_else(|| unavailable(stratified_unavailable_reason), inference),
        conclusion: match sensitivity.conclusion {
            ConfoundingConclusion::ConfoundedBySpatialStrata => {
                SpectrumConfoundingConclusion::ConfoundedBySpatialStrata
            }
            ConfoundingConclusion::BothSignificant => {
                SpectrumConfoundingConclusion::BothSignificant
            }
            ConfoundingConclusion::NoUnstratifiedSignal => {
                SpectrumConfoundingConclusion::NoUnstratifiedSignal
            }
            ConfoundingConclusion::DegenerateStratifiedNull => {
                SpectrumConfoundingConclusion::DegenerateStratifiedNull
            }
            ConfoundingConclusion::NotEvaluable => SpectrumConfoundingConclusion::NotEvaluable,
        },
    })
}

fn spectrum_curve(spectrum: &PermutationWhitenedSpectrum) -> Result<Vec<SpectrumPoint>> {
    (0..spectrum.k_values.len())
        .map(|index| {
            let values = [
                spectrum.k_values[index],
                spectrum.observed_power[index],
                spectrum.median_permutation_power[index],
                spectrum.whitened_power[index],
            ];
            if values.iter().any(|value| !value.is_finite()) {
                return Err(MarklabError::Compute(format!(
                    "spectrum curve point {index} contains a non-finite value"
                )));
            }
            let lower_global_envelope = spectrum
                .lower_global_envelope
                .get(index)
                .copied()
                .and_then(finite_option)
                .ok_or_else(|| {
                    MarklabError::Compute(format!(
                        "spectrum lower envelope is missing at point {index}"
                    ))
                })?;
            let upper_global_envelope = spectrum
                .upper_global_envelope
                .get(index)
                .copied()
                .and_then(finite_option)
                .ok_or_else(|| {
                    MarklabError::Compute(format!(
                        "spectrum upper envelope is missing at point {index}"
                    ))
                })?;
            Ok(SpectrumPoint {
                k: values[0],
                observed_power: values[1],
                median_permutation_power: values[2],
                whitened_power: values[3],
                inference_eligible: spectrum.inference_eligible[index],
                lower_global_envelope: Some(lower_global_envelope),
                upper_global_envelope: Some(upper_global_envelope),
            })
        })
        .collect()
}
