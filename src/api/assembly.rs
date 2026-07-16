use super::*;

pub(super) struct Inputs {
    pub(super) status: &'static str,
    pub(super) status_flags: Vec<StatusFlag>,
    pub(super) spectrum: Option<PermutationWhitenedSpectrum>,
    pub(super) pair_correlation: crate::output::AnalysisSection<FunctionalSummary>,
    pub(super) pair_correlation_curve: Vec<PairCorrelationPoint>,
    pub(super) anisotropy: Option<PermutationAnisotropy>,
    pub(super) wavelet: crate::output::AnalysisSection<WaveletSummary>,
    pub(super) scalogram: crate::output::AnalysisSection<FunctionalSummary>,
    pub(super) scalogram_curve: Vec<ScalogramPoint>,
    pub(super) territories: Vec<TerritoryFeature>,
    pub(super) diagnostics: crate::output::AnalysisSection<DiagnosticsResult>,
    pub(super) timings: Vec<TimingStage>,
    pub(super) interpretation: Interpretation,
}

pub(super) fn assemble(
    config: &AnalysisConfig,
    pattern: &Pattern,
    inputs: Inputs,
) -> Result<MarkedPatternResult> {
    let Inputs {
        status,
        status_flags,
        spectrum,
        pair_correlation,
        pair_correlation_curve,
        anisotropy,
        wavelet,
        scalogram,
        scalogram_curve,
        territories,
        diagnostics,
        timings,
        interpretation,
    } = inputs;

    let spectral_curve_p_global = spectrum
        .as_ref()
        .and_then(|spectrum| finite_option(spectrum.p_global));
    let xi_um = spectrum
        .as_ref()
        .and_then(|spectrum| spectrum.xi_um)
        .and_then(finite_option);
    let xi_stability_interval_um = spectrum
        .as_ref()
        .and_then(|spectrum| spectrum.xi_stability_interval_um)
        .filter(|interval| interval.iter().all(|value| value.is_finite()));
    let alpha = config
        .spectrum
        .fit_low_k_alpha
        .then(|| {
            spectrum
                .as_ref()
                .and_then(|spectrum| spectrum.alpha)
                .and_then(finite_option)
        })
        .flatten();
    let low_k_excess_p_value = spectrum
        .as_ref()
        .and_then(|spectrum| spectrum.low_k_excess_p_value)
        .and_then(finite_option);
    let xi_um_p_value = spectrum
        .as_ref()
        .and_then(|spectrum| spectrum.xi_um_p_value)
        .and_then(finite_option);
    let alpha_p_value = config
        .spectrum
        .fit_low_k_alpha
        .then(|| {
            spectrum
                .as_ref()
                .and_then(|spectrum| spectrum.alpha_p_value)
                .and_then(finite_option)
        })
        .flatten();
    let k_min = spectrum
        .as_ref()
        .and_then(|spectrum| spectrum.k_values.first().copied())
        .and_then(finite_option);
    let k_max = spectrum
        .as_ref()
        .and_then(|spectrum| spectrum.k_values.last().copied())
        .and_then(finite_option);
    let n_k_modes = spectrum.as_ref().map_or(0, |spectrum| spectrum.n_modes);
    let n_permutations = spectrum
        .as_ref()
        .map_or(0, |spectrum| spectrum.n_permutations);
    let spectrum_curve = spectrum
        .as_ref()
        .map(spectrum_curve)
        .transpose()?
        .unwrap_or_default();

    Ok(MarkedPatternResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        protein: pattern.meta.protein.clone(),
        mark_label: config.analysis.mark_label.clone(),
        status: status.into(),
        status_flags,
        n_cells: pattern.len(),
        n_marked: pattern.n_marked(),
        p_hat: pattern.p_hat(),
        window: WindowSummary {
            area_um2: pattern.window.area_um2,
            l_eff_um: pattern.window.l_eff_um,
            d_nn_mean_um: pattern.window.d_nn_mean_um,
        },
        qc: qc_summary(pattern),
        primary_endpoint: PrimaryEndpoint {
            name: "low_k_excess".into(),
            value: spectrum.as_ref().map_or_else(
                || crate::output::AnalysisSection::InsufficientData {
                    reason: "too few eligible spectrum shells or invalid spectrum input".into(),
                },
                |value| crate::output::AnalysisSection::available(value.low_k_excess),
            ),
            p_value: low_k_excess_p_value.map_or_else(
                || crate::output::AnalysisSection::InsufficientData {
                    reason: "the low-k null statistic was unavailable".into(),
                },
                crate::output::AnalysisSection::available,
            ),
            null: "fixed_position_random_labeling".into(),
        },
        spectrum: spectrum.as_ref().map_or_else(
            || crate::output::AnalysisSection::InsufficientData {
                reason: format!(
                    "fewer than {} inference-eligible spectrum shells or undefined spectrum input",
                    config.validation.k_shell_min
                ),
            },
            |spectrum_value| {
                crate::output::AnalysisSection::available(SpectrumSummary {
                    max_interpretable_scale_um: config
                        .validation
                        .largest_interpretable_scale_fraction
                        * pattern.window.l_eff_um,
                    k_min,
                    k_max,
                    n_k_modes,
                    n_shells: config.spectrum.k_shells,
                    n_permutations,
                    spectral_curve_test: match (
                        spectral_curve_p_global,
                        Some(spectrum_value.erl_depth),
                    ) {
                        (Some(p_global), Some(erl_depth)) => {
                            crate::output::AnalysisSection::available(FunctionalSummary {
                                p_global: Some(p_global),
                                erl_depth: Some(erl_depth),
                                n_permutations,
                            })
                        }
                        _ => crate::output::AnalysisSection::InsufficientData {
                            reason: "the spectral-curve ERL test was unavailable".into(),
                        },
                    },
                    xi_um,
                    xi_stability_interval_um,
                    low_k_excess: spectrum_value.low_k_excess,
                    low_k_excess_p_value,
                    alpha,
                    xi_um_p_value,
                    alpha_p_value,
                })
            },
        ),
        spectrum_curve,
        pair_correlation,
        pair_correlation_curve,
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
        wavelet,
        scalogram,
        scalogram_curve,
        wavelet_territories: if config.wavelet.enabled {
            if config.wavelet.territory_detection {
                crate::output::AnalysisSection::available(territories)
            } else {
                crate::output::AnalysisSection::Disabled
            }
        } else {
            crate::output::AnalysisSection::Disabled
        },
        registration: crate::output::AnalysisSection::NotApplicable,
        fused_cell_summary: crate::output::AnalysisSection::NotApplicable,
        fused_cells: Vec::new(),
        neighborhood_enrichment: crate::output::AnalysisSection::NotApplicable,
        cross_interaction_curves: crate::output::AnalysisSection::NotApplicable,
        territory_profiles: crate::output::AnalysisSection::NotApplicable,
        territory_comparisons: crate::output::AnalysisSection::NotApplicable,
        prepost_curve_tests: Vec::new(),
        component_results: crate::output::AnalysisSection::available(component_results_for(
            config, pattern,
        )?),
        diagnostics,
        timings,
        interpretation,
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
                return Err(MmrspaceError::Compute(format!(
                    "spectrum curve point {index} contains a non-finite value"
                )));
            }
            let lower_global_envelope = spectrum
                .lower_global_envelope
                .get(index)
                .copied()
                .and_then(finite_option)
                .ok_or_else(|| {
                    MmrspaceError::Compute(format!(
                        "spectrum lower envelope is missing at point {index}"
                    ))
                })?;
            let upper_global_envelope = spectrum
                .upper_global_envelope
                .get(index)
                .copied()
                .and_then(finite_option)
                .ok_or_else(|| {
                    MmrspaceError::Compute(format!(
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

pub(super) fn interpretation_for(
    status_flags: &[StatusFlag],
    status: &str,
    low_k_excess: Option<f64>,
) -> Interpretation {
    if status != "ok" {
        if status_flags.contains(&StatusFlag::InternalControlFailureOverlap)
            || status_flags.contains(&StatusFlag::StainGradientSuspect)
        {
            return Interpretation {
                class: "suppressed_qc_artifact".into(),
                text: "Spatial organization is present in the mark field but overlaps IHC/QC artifact structure; biologic interpretation is suppressed. This is not a clonality result.".into(),
            };
        }
        return Interpretation {
            class: "suppressed".into(),
            text: "Numeric diagnostics are emitted, but strong biologic interpretation is suppressed. This is not a clonality result.".into(),
        };
    }

    let Some(low_k_excess) = low_k_excess else {
        return Interpretation {
            class: "insufficient_data".into(),
            text: "Spectrum inference is unavailable at interpretable scales; no spatial organization claim is made. This is not a clonality result.".into(),
        };
    };

    if low_k_excess >= 1.25 {
        Interpretation {
            class: "coarse_clustered".into(),
            text: "The configured MMR-IHC phenotype shows coarse-scale spatial organization relative to random labeling. This is not a clonality result.".into(),
        }
    } else if low_k_excess <= 0.80 {
        Interpretation {
            class: "low_k_suppressed_or_dispersed".into(),
            text: "The configured MMR-IHC phenotype shows low-k suppression / dispersed pattern relative to random labeling. This is not a clonality result.".into(),
        }
    } else {
        Interpretation {
            class: "random_like".into(),
            text: "The configured MMR-IHC phenotype is random-like relative to fixed-position random labeling. This is not a clonality result.".into(),
        }
    }
}
