use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab::{ObservationWindow2D, ObservationWindowLimits};
use marklab_topology::sha256_hex;
use serde::Deserialize;
use serde_json::Value;

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Clone)]
pub(crate) struct PreparedAdvancedBayes {
    pub input_bytes: Vec<u8>,
    pub lock_bytes: Vec<u8>,
    pub worker_bytes: Vec<u8>,
    pub request_bytes: Vec<u8>,
    repository: PathBuf,
    worker_path: PathBuf,
    request: Value,
    timeout_seconds: u64,
    expected_format: &'static str,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveWindowSpdeSpec {
    window_frame: String,
    window: Value,
    base_resolution: u32,
    boundary_refinement_levels: u32,
    maximum_relative_area_error: f64,
    alpha: u32,
    kappa: f64,
    tau: f64,
    factor_observations: Vec<AdaptiveWindowSpdeObservation>,
    factor_count: u32,
    factor_noise_sd: f64,
    maximum_iterations: u32,
    maximum_vertices: usize,
    maximum_triangles: usize,
    maximum_boundary_segment_triangle_checks: u64,
    maximum_projection_triangle_visits: u64,
    memory_budget_mib: usize,
    maximum_result_bytes: usize,
    timeout_seconds: u64,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveWindowSpdeObservation {
    region_id: String,
    coordinates: [f64; 2],
    values: Vec<f64>,
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct AdvancedBayesCli {
    #[command(subcommand)]
    command: BayesTopLevel,
}

#[derive(Debug, Subcommand)]
enum BayesTopLevel {
    Bayes {
        #[command(subcommand)]
        command: AdvancedBayesCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AdvancedBayesCommand {
    HmcNormal {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdvancedCluster {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpdeSuite {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdaptiveWindowSpde {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match AdvancedBayesCli::parse().command {
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::HmcNormal { input, out },
        } => run("hmc_normal", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::AdvancedCluster { input, out },
        } => run("advanced_cluster", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::SpdeSuite { input, out },
        } => run("spde_suite", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::AdaptiveWindowSpde { input, out },
        } => run("adaptive_window_spde", input, out),
    }
}

fn run(mode: &str, input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let prepared = prepare(mode, &input)?;
    let result = execute(&prepared)?;
    publish_json(&out, &result)
}

pub(crate) fn prepare(
    mode: &str,
    input: &std::path::Path,
) -> Result<PreparedAdvancedBayes, TopologyCliError> {
    let input_bytes = read_input(input)?;
    let spec: Value = serde_json::from_slice(&input_bytes)?;
    let timeout = spec
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            TopologyCliError::Input(
                "advanced Bayesian input requires integer timeout_seconds".into(),
            )
        })?;
    if !(1..=3_600).contains(&timeout) || !spec.is_object() {
        return Err(TopologyCliError::Input(
            "advanced Bayesian timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worker_path = repository.join("workers/python/marklab_scipy_advanced_bayes_worker.py");
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker = read_required(&worker_path)?;
    let adaptive_window = if mode == "adaptive_window_spde" {
        let typed: AdaptiveWindowSpdeSpec = serde_json::from_value(spec.clone())?;
        let geometry = &typed.window;
        let geometry_bytes = serde_json::to_vec(geometry)?;
        let geometry_text = std::str::from_utf8(&geometry_bytes).map_err(|_| {
            TopologyCliError::Input("adaptive SPDE window serialization is not UTF-8".into())
        })?;
        Some(
            ObservationWindow2D::from_geojson_str(
                geometry_text,
                ObservationWindowLimits::default(),
            )
            .map_err(|error| TopologyCliError::Input(error.to_string()))?,
        )
    } else {
        None
    };
    let mut request = serde_json::json!({
        "format":"marklab.scipy_advanced_bayes_request", "version":1, "mode":mode,
        "backend":{"name":"numpy_scipy_advanced_bayes","scipy_version":"1.18.1","numpy_version":"2.4.6","python_version":"3.12","license":"BSD-3-Clause","environment_lock_sha256":sha256_hex(&lock),"worker_sha256":sha256_hex(&worker)},
        "spec":spec, "input_sha256":sha256_hex(&input_bytes)
    });
    if let Some(window) = adaptive_window {
        request["canonical_window_sha256"] =
            Value::String(window.descriptor().logical_digest.to_string());
        request["canonical_window_area"] = Value::from(window.area_um2());
    }
    let request_bytes = serde_json::to_vec(&request)?;
    let expected = match mode {
        "hmc_normal" => "marklab.fixed_step_hmc_normal_mean",
        "advanced_cluster" => "marklab.advanced_cluster_and_gibbs_models",
        "spde_suite" => "marklab.rectangular_spde_suite",
        "adaptive_window_spde" => "marklab.adaptive_window_spde",
        _ => {
            return Err(TopologyCliError::Input(format!(
                "unknown advanced Bayesian mode {mode}"
            )))
        }
    };
    Ok(PreparedAdvancedBayes {
        input_bytes,
        lock_bytes: lock,
        worker_bytes: worker,
        request_bytes,
        repository,
        worker_path,
        request,
        timeout_seconds: timeout,
        expected_format: expected,
    })
}

pub(crate) fn execute(prepared: &PreparedAdvancedBayes) -> Result<Value, TopologyCliError> {
    let response = run_worker(
        &prepared.repository,
        &prepared.worker_path,
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: Value = serde_json::from_slice(&response)?;
    validate_result(prepared, &result)?;
    Ok(result)
}

pub(crate) fn validate_result(
    prepared: &PreparedAdvancedBayes,
    result: &Value,
) -> Result<(), TopologyCliError> {
    if result["format"] != prepared.expected_format
        || result["backend"] != prepared.request["backend"]
        || result["request_sha256"] != sha256_hex(&prepared.request_bytes)
        || (prepared.expected_format == "marklab.adaptive_window_spde"
            && (result["window"]["canonical_sha256"]
                != prepared.request["canonical_window_sha256"]
                || result["window"]["exact_area"] != prepared.request["canonical_window_area"]))
    {
        return Err(TopologyCliError::Backend(
            "advanced Bayesian result identity mismatch".into(),
        ));
    }
    Ok(())
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
