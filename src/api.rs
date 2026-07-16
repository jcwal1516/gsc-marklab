use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use crate::{
    config::{AnalysisConfig, ComponentMode, PermutationStratum, ThreadSetting},
    data::{validate::validation_flags, Pattern},
    diagnostics::beta_binomial::beta_binomial,
    errors::{MmrspaceError, Result},
    geom::{components::ComponentSummary, spatial_index::mean_nearest_neighbor_distance},
    inference::scalar_pvalues::{permutation_p_value, Tail},
    output::{
        AnisotropySummary, ComponentAnalysisSummary, DiagnosticsResult, FunctionalSummary,
        Interpretation, MarkedPatternResult, PairCorrelationPoint, PrimaryEndpoint, QcSummary,
        ScalogramPoint, SpectrumPoint, SpectrumSummary, StatusFlag, TerritoryFeature, TimingStage,
        WaveletSummary, WindowSummary,
    },
    perf::counters::{estimate_peak_memory, MemoryEstimate, MemoryInputs},
    periodogram::{
        bartlett::marked_bartlett_periodogram,
        raster::{centered_mark_raster, centered_mark_raster_for_marks},
    },
    permutation::{
        envelopes::GlobalEnvelope, labels::permute_fixed_count, stratified::permute_within_strata,
    },
    qc::stain_gradient::gradient_suspect,
    spectra::anisotropy::{permutation_whitened_anisotropy, PermutationAnisotropy},
    spectra::pair_correlation::{pair_correlation, pair_correlation_for_marks},
    spectra::structure_factor::{
        observed_power_for_modes, observed_value_power_for_modes, permutation_whitened_spectrum,
        permutation_whitened_spectrum_from_observed_modes, permutation_whitened_value_spectrum,
        permutation_whitened_value_spectrum_from_observed_modes, resolvable_modes_for_pattern,
        stratified_permutation_whitened_spectrum, PermutationWhitenedSpectrum,
        SpectrumPermutationOptions,
    },
    wavelet::{
        modwt::variance_fractions_from_field,
        territories::{detect_residual_territories, CandidateTerritory},
    },
};

mod assembly;
mod components;
mod diagnostics;
mod qc_pipeline;
mod stages;

use assembly::interpretation_for;
use components::component_results_for;
use qc_pipeline::{permutation_labels, qc_summary, stratified_confounds, validate_pattern};
use stages::{
    estimated_raster_pixels, pair_correlation_with_envelope,
    periodogram_disagrees_with_particle_spectrum, scalogram_with_envelope, territories_for,
    wavelet_scalar_p_values,
};

pub struct AnalysisEngine {
    config: AnalysisConfig,
    threads: usize,
    #[cfg(feature = "parallel")]
    pool: rayon::ThreadPool,
}

impl AnalysisEngine {
    pub fn new(config: AnalysisConfig) -> Result<Self> {
        config.validate()?;
        if config.diagnostics.graph_smoothing {
            return Err(MmrspaceError::Config(
                "graph_smoothing diagnostic requires multimodal analyze with a fused-cell graph"
                    .into(),
            ));
        }

        let threads = if config.performance.strict_repro {
            1
        } else {
            match config.performance.threads {
                ThreadSetting::Auto => std::thread::available_parallelism()
                    .map(|count| count.get())
                    .unwrap_or(1),
                ThreadSetting::Count(count) => count.max(1),
            }
        };

        #[cfg(feature = "parallel")]
        {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|i| format!("mmrspace-{i}"))
                .build()
                .map_err(|err| crate::errors::MmrspaceError::Compute(err.to_string()))?;
            Ok(Self {
                config,
                threads,
                pool,
            })
        }

        #[cfg(not(feature = "parallel"))]
        {
            Ok(Self { config, threads })
        }
    }

    pub fn analyze_pattern(&self, pattern: &Pattern) -> Result<MarkedPatternResult> {
        #[cfg(feature = "parallel")]
        {
            self.pool.install(|| self.analyze_pattern_inner(pattern))
        }

        #[cfg(not(feature = "parallel"))]
        {
            self.analyze_pattern_inner(pattern)
        }
    }

    fn analyze_pattern_inner(&self, pattern: &Pattern) -> Result<MarkedPatternResult> {
        let memory_estimate = estimate_analysis_memory(pattern, &self.config);
        memory_estimate.enforce_budget_mib(self.config.performance.memory_budget_mib)?;

        let mut timings = Vec::new();

        let (mut status_flags, configured_strata) =
            timed_stage(&mut timings, "validate", self.threads, || {
                validate_pattern(&self.config, pattern)
            })?;

        let modes = timed_stage(&mut timings, "kgrid", self.threads, || {
            resolvable_modes_for_pattern(pattern, self.config.spectrum.k_shells).unwrap_or_default()
        });
        let observed_mode_power = timed_stage(
            &mut timings,
            "structure_factor_observed",
            self.threads,
            || {
                if self.config.analysis.use_probabilistic_marks {
                    pattern.mark_prob.as_deref().and_then(|values| {
                        let values = values.iter().copied().map(f64::from).collect::<Vec<_>>();
                        observed_value_power_for_modes(pattern, &values, &modes)
                    })
                } else {
                    Some(observed_power_for_modes(pattern, &modes))
                }
            },
        );
        let spectrum = timed_stage(
            &mut timings,
            "permutation_spectra",
            self.threads,
            || -> Result<_> {
                if self.config.analysis.use_probabilistic_marks {
                    let Some(values) = pattern.mark_prob.as_deref() else {
                        return Ok(None);
                    };
                    let values = values.iter().copied().map(f64::from).collect::<Vec<_>>();
                    let Some(observed_mode_power) = observed_mode_power.clone() else {
                        return Ok(None);
                    };
                    permutation_whitened_value_spectrum_from_observed_modes(
                        pattern,
                        &values,
                        &modes,
                        observed_mode_power,
                        SpectrumPermutationOptions {
                            n_shells: self.config.spectrum.k_shells,
                            low_k_modes: self.config.spectrum.low_k_shells,
                            n_permutations: self.config.permutation.b,
                            seed: self.config.permutation.seed,
                            family_wise_alpha: self.config.inference.family_wise_alpha,
                            max_scale_um: self
                                .config
                                .validation
                                .largest_interpretable_scale_fraction
                                * pattern.window.l_eff_um,
                            k_shell_min: self.config.validation.k_shell_min,
                        },
                    )
                } else {
                    let options = SpectrumPermutationOptions {
                        n_shells: self.config.spectrum.k_shells,
                        low_k_modes: self.config.spectrum.low_k_shells,
                        n_permutations: self.config.permutation.b,
                        seed: self.config.permutation.seed,
                        family_wise_alpha: self.config.inference.family_wise_alpha,
                        max_scale_um: self.config.validation.largest_interpretable_scale_fraction
                            * pattern.window.l_eff_um,
                        k_shell_min: self.config.validation.k_shell_min,
                    };
                    if let Some(strata) = configured_strata.as_deref() {
                        stratified_permutation_whitened_spectrum(pattern, strata, options)
                    } else {
                        let Some(observed_mode_power) = observed_mode_power else {
                            return Ok(None);
                        };
                        permutation_whitened_spectrum_from_observed_modes(
                            pattern,
                            &modes,
                            observed_mode_power,
                            options,
                        )
                    }
                }
            },
        )?;

        let low_k_excess = spectrum
            .as_ref()
            .map(|value| value.low_k_excess)
            .filter(|value| value.is_finite());

        let periodogram_artifact = timed_stage(&mut timings, "periodogram", self.threads, || {
            self.config.periodogram.enabled
                && low_k_excess.is_some_and(|value| {
                    periodogram_disagrees_with_particle_spectrum(&self.config, pattern, value)
                })
        });
        if periodogram_artifact {
            status_flags.push(StatusFlag::WindowOrGriddingArtifactSuspect);
        }
        timed_stage(&mut timings, "inference", self.threads, || -> Result<()> {
            if self.config.permutation.stratified
                && spectrum
                    .as_ref()
                    .and_then(|spectrum| finite_option(spectrum.p_global))
                    .map(|p_global| p_global < self.config.inference.family_wise_alpha)
                    .unwrap_or(false)
                && stratified_confounds(&self.config, pattern)?
            {
                status_flags.push(StatusFlag::ConfoundedBySpatialStrata);
            }
            Ok(())
        })?;

        let status = if status_flags.is_empty() {
            "ok"
        } else {
            "suppressed"
        };
        let n_k_modes = spectrum.as_ref().map_or(0, |spectrum| spectrum.n_modes);
        let n_permutations = spectrum
            .as_ref()
            .map_or(0, |spectrum| spectrum.n_permutations);
        let stage_start = Instant::now();
        let (pair_correlation_curve, pair_correlation) =
            pair_correlation_with_envelope(&self.config, pattern)?;
        push_timing(
            &mut timings,
            "pair_correlation",
            stage_start.elapsed(),
            self.threads,
        );

        let stage_start = Instant::now();
        let territories = territories_for(&self.config, pattern);
        push_timing(&mut timings, "wavelet", stage_start.elapsed(), self.threads);

        let interpretation = interpretation_for(&status_flags, status, low_k_excess);

        let stage_start = Instant::now();
        let anisotropy = permutation_whitened_anisotropy(
            pattern,
            self.config.spectrum.anisotropy_low_k_shells,
            self.config.permutation.b,
            self.config.permutation.seed,
            self.config.inference.family_wise_alpha,
            configured_strata.as_deref(),
        )?;
        push_timing(
            &mut timings,
            "anisotropy",
            stage_start.elapsed(),
            self.threads,
        );

        let stage_start = Instant::now();
        let wavelet_fractions = if self.config.wavelet.enabled {
            centered_mark_raster(pattern, pattern.window.d_nn_mean_um.max(1.0)).and_then(
                |(spec, raster)| variance_fractions_from_field(&raster, spec.width, spec.height),
            )
        } else {
            None
        };
        let (wavelet, scalogram_curve, scalogram) = if !self.config.wavelet.enabled {
            (
                crate::output::AnalysisSection::Disabled,
                Vec::new(),
                crate::output::AnalysisSection::Disabled,
            )
        } else if let Some(fractions) = wavelet_fractions.filter(|fractions| {
            [fractions.fine, fractions.intermediate, fractions.coarse]
                .iter()
                .all(|value| value.is_finite())
        }) {
            let coarse_to_fine_ratio = (fractions.fine > 0.0)
                .then_some(fractions.coarse / fractions.fine)
                .filter(|value| value.is_finite());
            let (curve, scalogram) = scalogram_with_envelope(
                &self.config,
                pattern,
                fractions.fine,
                fractions.intermediate,
                fractions.coarse,
            )?;
            let (coarse_variance_fraction_p_value, territory_count_p_value) =
                wavelet_scalar_p_values(
                    &self.config,
                    pattern,
                    fractions.coarse,
                    territories.len(),
                )?;
            (
                crate::output::AnalysisSection::available(WaveletSummary {
                    fine_variance_fraction: fractions.fine,
                    intermediate_variance_fraction: fractions.intermediate,
                    coarse_variance_fraction: fractions.coarse,
                    coarse_to_fine_ratio,
                    territory_count: territories.len(),
                    coarse_variance_fraction_p_value,
                    territory_count_p_value,
                }),
                curve,
                scalogram,
            )
        } else {
            (
                crate::output::AnalysisSection::InsufficientData {
                    reason: "wavelet variance fractions could not be estimated".into(),
                },
                Vec::new(),
                crate::output::AnalysisSection::InsufficientData {
                    reason: "wavelet variance fractions could not be estimated".into(),
                },
            )
        };
        push_timing(
            &mut timings,
            "modwt_variance",
            stage_start.elapsed(),
            self.threads,
        );
        let diagnostics = diagnostics::run(&self.config, pattern, &mut timings, self.threads)?;
        annotate_timings(
            &mut timings,
            pattern,
            n_k_modes,
            n_permutations,
            memory_estimate.total_mib(),
        );

        assembly::assemble(
            &self.config,
            pattern,
            assembly::Inputs {
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
            },
        )
    }

    #[cfg(feature = "cli")]
    pub(crate) fn thread_count(&self) -> usize {
        self.threads
    }
}

fn timed_stage<T>(
    timings: &mut Vec<TimingStage>,
    stage_name: &'static str,
    threads: usize,
    f: impl FnOnce() -> T,
) -> T {
    let span = tracing::info_span!("mmrspace_stage", stage_name);
    let _enter = span.enter();
    let stage_start = Instant::now();
    let result = f();
    push_timing(timings, stage_name, stage_start.elapsed(), threads);
    result
}

fn push_timing(
    timings: &mut Vec<TimingStage>,
    stage_name: impl Into<String>,
    elapsed: Duration,
    threads: usize,
) {
    timings.push(TimingStage {
        stage_name: stage_name.into(),
        wall_ms: elapsed.as_secs_f64() * 1000.0,
        cpu_threads: threads.max(1),
        n_cells: 0,
        n_marked: 0,
        n_k_modes: 0,
        n_permutations: 0,
        estimated_peak_memory_mib: 0.0,
    });
}

fn annotate_timings(
    timings: &mut [TimingStage],
    pattern: &Pattern,
    n_k_modes: usize,
    n_permutations: usize,
    estimated_peak_memory_mib: f64,
) {
    for timing in timings {
        timing.n_cells = pattern.len();
        timing.n_marked = pattern.n_marked();
        timing.n_k_modes = n_k_modes;
        timing.n_permutations = n_permutations;
        timing.estimated_peak_memory_mib = estimated_peak_memory_mib;
    }
}

fn estimate_analysis_memory(pattern: &Pattern, config: &AnalysisConfig) -> MemoryEstimate {
    estimate_peak_memory(MemoryInputs {
        n_points: pattern.len(),
        optional_point_bytes: optional_point_bytes(pattern),
        raster_pixels: estimated_raster_pixels(pattern),
        raster_bytes_per_pixel: 4,
        active_raster_buffers: 3,
        n_shells: config.spectrum.k_shells,
        n_outputs: 4,
        n_permutations: config.permutation.b,
        n_scalar_stats: 6,
        k_chunk_modes: config.performance.k_chunk_modes,
        scratch_per_mode_bytes: 32,
    })
}

fn optional_point_bytes(pattern: &Pattern) -> usize {
    usize::from(pattern.mark_prob.is_some()) * 4
        + usize::from(pattern.tumor_probability.is_some()) * 4
        + usize::from(pattern.nucleus_area_um2.is_some()) * 4
        + usize::from(pattern.component_id.is_some()) * 4
        + usize::from(pattern.qc_bin.is_some()) * 2
        + pattern.categorical_strata.len() * 4
        + usize::from(pattern.local_dab_od.is_some()) * 4
        + usize::from(pattern.local_hematoxylin_od.is_some()) * 4
}

fn finite_option(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}
