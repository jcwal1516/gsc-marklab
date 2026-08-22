use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use crate::{
    geom::mask::TumorMask,
    io::{intermediates::write_analysis_intermediates, load_pattern_path_with_diagnostics},
    AnalysisConfig, AnalysisEngine, MarkedPatternResult, MarklabError, OutputWriter, Result,
    ThreadSetting, TimingStage,
};

use super::{AnalyzeRequest, LogLevel, ObservabilityOptions};

#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
fn start_heap_profiler(path: Option<&Path>) -> Result<Option<dhat::Profiler>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(Some(dhat::Profiler::builder().file_name(path).build()))
}

#[cfg(not(all(feature = "dhat-heap", not(feature = "allocator-mimalloc"))))]
fn start_heap_profiler(path: Option<&Path>) -> Result<Option<()>> {
    if path.is_some() {
        return Err(MarklabError::Validation(
            "--heap-profile requires a binary built with the dhat-heap feature and without allocator-mimalloc".into(),
        ));
    }
    Ok(None)
}

pub(super) fn run(request: AnalyzeRequest) -> Result<()> {
    let AnalyzeRequest {
        cells,
        mask,
        config,
        out,
        threads,
        observability,
        heap_profile,
    } = request;
    let _heap_profiler = start_heap_profiler(heap_profile.as_deref())?;
    init_logging(observability.log);

    let load_start = Instant::now();
    let load_span = tracing::info_span!("marklab_stage", stage_name = "load");
    let load_enter = load_span.enter();
    let config_path = config;
    let mut config = AnalysisConfig::from_toml_path(&config_path)?;
    if let Some(threads) = threads {
        config.performance.threads = ThreadSetting::Count(threads);
    }

    let mask_path = mask;
    let mask_text = fs::read_to_string(&mask_path)?;
    let mask = TumorMask::from_geojson_str(&mask_text)?;
    let load_result = load_pattern_path_with_diagnostics(&cells, &mask)?;
    drop(load_enter);
    let load_elapsed = load_start.elapsed();
    let pattern = load_result.pattern;
    let output = config.output.clone();
    let save_intermediates = config.performance.save_intermediates;
    let permutation_count = config.permutation.b;
    let permutation_seed = config.permutation.seed;
    let strict_repro = config.performance.strict_repro;
    let intermediate_config = config.clone();
    let engine = AnalysisEngine::new(config)?;
    let mut run = engine.analyze_pattern_run(&pattern)?;
    let timing_context = TimingContext::from_result(&run.result, run.actual_thread_count);
    prepend_load_timings(
        &mut run.result.timings,
        LoadStageDurations {
            load: load_elapsed,
            mask_filter: load_result.diagnostics.mask_filter,
            nearest_neighbor: load_result.diagnostics.nearest_neighbor,
        },
        timing_context,
    );

    let mut writer_output = output.clone();
    if output.write_run_manifest {
        writer_output.write_run_manifest = false;
    }
    let run_manifest = output.write_run_manifest.then(|| {
        serde_json::json!({
            "command": "analyze",
            "program": "marklab",
            "crate_version": env!("CARGO_PKG_VERSION"),
            "format_version": crate::RESULT_FORMAT_VERSION,
            "inputs": {
                "cells": cells.to_string_lossy(),
                "mask": mask_path.to_string_lossy(),
                "config": config_path.to_string_lossy(),
            },
            "execution": {
                "thread_count": run.actual_thread_count,
                "requested_threads": threads,
                "permutations": permutation_count,
                "permutation_seed": permutation_seed,
                "strict_repro": strict_repro,
                "save_intermediates": save_intermediates,
                "log_level": observability.log.map(LogLevel::as_str),
                "heap_profile": heap_profile.as_ref().map(|path| path.to_string_lossy().to_string()),
            },
            "result": {
                "case_id": &run.result.case_id,
                "timepoint": &run.result.timepoint,
                "protein": &run.result.protein,
                "status": &run.result.status,
                "status_flags": &run.result.status_flags,
                "n_cells": run.result.n_cells,
                "n_marked": run.result.n_marked,
                "p_hat": run.result.p_hat,
            },
            "output": {
                "write_parquet_curves": output.write_parquet_curves,
                "write_geojson_territories": output.write_geojson_territories,
                "write_figures": output.write_figures,
                "write_run_manifest": output.write_run_manifest,
            },
            "timings_stage_count": run.result.timings.len(),
        })
    });
    OutputWriter::write_marked_run(run, &out, &writer_output)?;
    if save_intermediates {
        write_analysis_intermediates(&out, &pattern, &intermediate_config)?;
    }
    write_observability_outputs(&out, &observability)?;
    if let Some(run_manifest) = run_manifest {
        fs::write(
            out.join("run_manifest.json"),
            serde_json::to_string_pretty(&run_manifest)?,
        )?;
    }

    Ok(())
}

fn init_logging(log: Option<LogLevel>) {
    if let Some(log) = log {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(log.as_str())
            .json()
            .try_init();
    }
}

#[derive(Clone, Copy, Debug)]
struct LoadStageDurations {
    load: Duration,
    mask_filter: Duration,
    nearest_neighbor: Duration,
}

#[derive(Clone, Copy, Debug)]
struct TimingContext {
    threads: usize,
    n_cells: usize,
    n_marked: usize,
    n_k_modes: usize,
    n_permutations: usize,
    estimated_peak_memory_mib: f64,
}

impl TimingContext {
    fn from_result(result: &MarkedPatternResult, threads: usize) -> Self {
        Self {
            threads,
            n_cells: result.n_cells,
            n_marked: result.n_marked,
            n_k_modes: result.spectrum.value().map_or(0, |value| value.n_k_modes),
            n_permutations: result
                .spectrum
                .value()
                .map_or(0, |value| value.n_permutations),
            estimated_peak_memory_mib: result
                .timings
                .first()
                .map(|stage| stage.estimated_peak_memory_mib)
                .unwrap_or(0.0),
        }
    }
}

fn prepend_load_timings(
    timings: &mut Vec<TimingStage>,
    durations: LoadStageDurations,
    context: TimingContext,
) {
    let mut prefixed = vec![
        timing_stage("load", durations.load, context),
        timing_stage("mask_filter", durations.mask_filter, context),
        timing_stage("nearest_neighbor", durations.nearest_neighbor, context),
    ];
    prefixed.append(timings);
    *timings = prefixed;
}

fn timing_stage(
    stage_name: impl Into<String>,
    elapsed: Duration,
    context: TimingContext,
) -> TimingStage {
    TimingStage {
        stage_name: stage_name.into(),
        wall_ms: elapsed.as_secs_f64() * 1000.0,
        cpu_threads: context.threads.max(1),
        n_cells: context.n_cells,
        n_marked: context.n_marked,
        n_k_modes: context.n_k_modes,
        n_permutations: context.n_permutations,
        estimated_peak_memory_mib: context.estimated_peak_memory_mib,
    }
}

fn write_observability_outputs(out: &Path, observability: &ObservabilityOptions) -> Result<()> {
    let timings_path = out.join("timings.json");
    let timings_text = fs::read_to_string(&timings_path)?;

    if let Some(path) = observability.timings.as_deref() {
        write_with_parent(path, &timings_text)?;
    }

    if let Some(path) = observability.trace_json.as_deref() {
        let timings_json: serde_json::Value = serde_json::from_str(&timings_text)?;
        let stages = timings_json["stages"].as_array().ok_or_else(|| {
            MarklabError::Validation("timings.json does not contain a stages array".into())
        })?;
        let log_level = observability.log.map(LogLevel::as_str).unwrap_or("info");
        let mut jsonl = String::new();
        for stage in stages {
            let mut event = stage.clone();
            if let Some(object) = event.as_object_mut() {
                object.insert("event".into(), serde_json::json!("stage_timing"));
                object.insert("log_level".into(), serde_json::json!(log_level));
            }
            jsonl.push_str(&serde_json::to_string(&event)?);
            jsonl.push('\n');
        }
        write_with_parent(path, &jsonl)?;
    }

    Ok(())
}

fn write_with_parent(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}
