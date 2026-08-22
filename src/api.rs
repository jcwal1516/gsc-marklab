use std::time::{Duration, Instant};

use crate::{
    config::{AnalysisConfig, ThreadSetting},
    data::Pattern,
    diagnostics::beta_posterior::beta_posterior_group_summary,
    errors::{MarklabError, Result},
    output::{Interpretation, MarkedPatternResult, StatusFlag, TimingStage},
    perf::counters::{estimate_peak_memory, MemoryEstimate, MemoryInputs},
};

mod assembly;
mod components;
mod context;
mod diagnostics;
mod qc_pipeline;
mod spatial_stage;
mod spectrum_stage;
mod stages;

#[cfg(test)]
mod context_tests {
    use super::context::MarkedAnalysisContext;
    use crate::{data::PatternMeta, Pattern};

    #[test]
    fn marked_analysis_context_caches_counts_prevalence_and_geometry() {
        let mut pattern = Pattern::from_arrays(
            vec![0.0, 1.0, 2.0, 3.0],
            vec![0.0, 0.0, 0.0, 0.0],
            vec![1, 0, 1, 0],
            PatternMeta {
                case_id: "context".into(),
                timepoint: "post".into(),
                protein: "MSH6".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern");
        pattern.window.area_um2 = 40.0;
        pattern.window.l_eff_um = 4.0;
        pattern.window.d_nn_mean_um = 1.0;

        let context = MarkedAnalysisContext::new(&pattern);

        assert_eq!(context.n_cells(), 4);
        assert_eq!(context.n_marked(), 2);
        assert_eq!(context.n_unmarked(), 2);
        assert_eq!(context.prevalence(), 0.5);
        assert_eq!(context.geometry().area_um2, 40.0);
        assert_eq!(context.geometry().effective_length_um, 4.0);
        assert_eq!(context.geometry().mean_nearest_neighbor_um, 1.0);
        assert!(std::ptr::eq(context.pattern(), &pattern));
    }
}

use assembly::interpretation_for;
use components::component_analysis_plan;
use context::MarkedAnalysisContext;
use qc_pipeline::validate_pattern;
use stages::estimated_raster_pixels;

pub struct AnalysisEngine {
    config: AnalysisConfig,
    threads: usize,
    #[cfg(feature = "parallel")]
    pool: rayon::ThreadPool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkedAnalysisRun {
    pub result: MarkedPatternResult,
    pub actual_thread_count: usize,
}

impl AnalysisEngine {
    pub fn new(config: AnalysisConfig) -> Result<Self> {
        config.validate()?;
        if config.diagnostics.graph_smoothing {
            return Err(MarklabError::Config(
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
                .thread_name(|i| format!("marklab-{i}"))
                .build()
                .map_err(|err| crate::errors::MarklabError::Compute(err.to_string()))?;
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
        Ok(self.analyze_pattern_run(pattern)?.result)
    }

    pub fn analyze_pattern_run(&self, pattern: &Pattern) -> Result<MarkedAnalysisRun> {
        #[cfg(feature = "parallel")]
        let result = self.pool.install(|| self.analyze_pattern_inner(pattern))?;

        #[cfg(not(feature = "parallel"))]
        let result = self.analyze_pattern_inner(pattern)?;

        Ok(MarkedAnalysisRun {
            result,
            actual_thread_count: self.threads,
        })
    }

    fn analyze_pattern_inner(&self, pattern: &Pattern) -> Result<MarkedPatternResult> {
        let analysis_context = MarkedAnalysisContext::new(pattern);
        let memory_estimate = estimate_analysis_memory(pattern, &self.config, self.threads);
        memory_estimate.enforce_budget_mib(self.config.performance.memory_budget_mib)?;
        let configured_memory_bytes = self
            .config
            .performance
            .memory_budget_mib
            .saturating_mul(1024 * 1024);
        let geometry_budget_bytes =
            configured_memory_bytes.saturating_sub(memory_estimate.total_bytes);

        let mut timings = Vec::new();

        let (mut status_flags, configured_strata) =
            timed_stage(&mut timings, "validate", self.threads, || {
                validate_pattern(&self.config, &analysis_context)
            })?;
        let component_plan = component_analysis_plan(&self.config, pattern);
        let includes_pooled = component_plan.includes_pooled();

        let spectrum_stage::Output {
            spectrum,
            null_sensitivity: spectrum_null_sensitivity,
            unavailable_reason: spectrum_unavailable_reason,
        } = spectrum_stage::run(
            &self.config,
            &analysis_context,
            includes_pooled,
            configured_strata.as_deref(),
            &mut timings,
            self.threads,
        )?;

        timed_stage(&mut timings, "inference", self.threads, || {
            spectrum_stage::apply_null_sensitivity_status(
                spectrum_null_sensitivity,
                &mut status_flags,
            );
        });

        let low_k_excess = spectrum
            .as_ref()
            .map(|value| value.low_k_excess)
            .filter(|value| value.is_finite());

        let spatial = spatial_stage::run(
            &self.config,
            &analysis_context,
            includes_pooled,
            configured_strata.as_deref(),
            low_k_excess,
            spatial_stage::ExecutionContext {
                geometry_budget_bytes,
                timings: &mut timings,
                threads: self.threads,
            },
        )?;
        if spatial.periodogram_artifact {
            status_flags.push(StatusFlag::WindowOrGriddingArtifactSuspect);
        }
        let status = if status_flags.is_empty() {
            "ok"
        } else {
            "suppressed"
        };
        let n_k_modes = spectrum.as_ref().map_or(0, |spectrum| spectrum.n_modes);
        let n_permutations = spectrum
            .as_ref()
            .map_or(0, |spectrum| spectrum.n_permutations);
        let interpretation = if includes_pooled {
            interpretation_for(&status_flags, status, low_k_excess)
        } else {
            Interpretation {
                class: "separate_components".into(),
                text: "Component spectra are reported separately; no pooled primary endpoint or pooled interpretation was calculated.".into(),
            }
        };

        let diagnostics = diagnostics::run(&self.config, pattern, &mut timings, self.threads)?;
        let estimated_peak_memory_mib = memory_estimate
            .total_bytes
            .saturating_add(spatial.estimated_geometry_storage_bytes)
            as f64
            / (1024.0 * 1024.0);
        annotate_timings(
            &mut timings,
            &analysis_context,
            n_k_modes,
            n_permutations,
            estimated_peak_memory_mib,
        );

        assembly::assemble(
            &self.config,
            &analysis_context,
            assembly::Inputs {
                status,
                status_flags,
                spectrum,
                spectrum_null_sensitivity,
                spectrum_unavailable_reason,
                mark_pair_covariance: spatial.mark_pair_covariance,
                mark_pair_covariance_curve: spatial.mark_pair_covariance_curve,
                anisotropy: spatial.anisotropy,
                multiscale_residual: spatial.multiscale_residual,
                scale_energy: spatial.scale_energy,
                scale_energy_curve: spatial.scale_energy_curve,
                territories: spatial.territories,
                diagnostics,
                timings,
                interpretation,
                component_plan,
            },
        )
    }
}

fn timed_stage<T>(
    timings: &mut Vec<TimingStage>,
    stage_name: &'static str,
    threads: usize,
    f: impl FnOnce() -> T,
) -> T {
    let span = tracing::info_span!("marklab_stage", stage_name);
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
    context: &MarkedAnalysisContext<'_>,
    n_k_modes: usize,
    n_permutations: usize,
    estimated_peak_memory_mib: f64,
) {
    for timing in timings {
        timing.n_cells = context.n_cells();
        timing.n_marked = context.n_marked();
        timing.n_k_modes = n_k_modes;
        timing.n_permutations = n_permutations;
        timing.estimated_peak_memory_mib = estimated_peak_memory_mib;
    }
}

fn estimate_analysis_memory(
    pattern: &Pattern,
    config: &AnalysisConfig,
    worker_threads: usize,
) -> MemoryEstimate {
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
        worker_threads,
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
