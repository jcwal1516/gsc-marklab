use std::{collections::HashSet, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_topology::sha256_hex;
use serde::{Deserialize, Serialize};

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct NeuralCli {
    #[command(subcommand)]
    command: NeuralTopLevel,
}

#[derive(Debug, Subcommand)]
enum NeuralTopLevel {
    Neural {
        #[command(subcommand)]
        command: NeuralCommand,
    },
}

#[derive(Debug, Subcommand)]
enum NeuralCommand {
    PointProcess {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    PointSetGenerators {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Sbi {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ValidateGenerative {
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        model_repeat: PathBuf,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NeuralPoint {
    coordinates: [f64; 2],
    mark: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NeuralPattern {
    pattern_id: String,
    patient_id: String,
    split: String,
    window: [f64; 4],
    context: Vec<f64>,
    points: Vec<NeuralPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NeuralPointArchitecture {
    hidden_units: usize,
    activation: String,
    intensity_link: String,
    mark_link: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NeuralPointProcessSpec {
    patterns: Vec<NeuralPattern>,
    mark_labels: Vec<String>,
    architecture: NeuralPointArchitecture,
    quadrature_grid: usize,
    quadrature_reference_grid: usize,
    parameter_precision: f64,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PointSetPattern {
    pattern_id: String,
    patient_id: String,
    split: String,
    window: [f64; 4],
    context: Vec<f64>,
    points: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PointSetFlowSpec {
    family: String,
    boundary_epsilon: f64,
    variance_floor: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PointSetDiffusionSpec {
    schedule: String,
    steps: usize,
    beta_min: f64,
    beta_max: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointSetGeneratorsSpec {
    patterns: Vec<PointSetPattern>,
    flow: PointSetFlowSpec,
    diffusion: PointSetDiffusionSpec,
    generated_patterns_per_context: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NeuralSbiPrior {
    family: String,
    lower: f64,
    upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NeuralSbiSimulator {
    family: String,
    noise_standard_deviation: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NeuralSbiTraining {
    density_estimator: String,
    ratio_classifier: String,
    batch_size: usize,
    maximum_epochs: usize,
    stop_after_epochs: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NeuralSbiSpec {
    prior: NeuralSbiPrior,
    simulator: NeuralSbiSimulator,
    observed_summary: f64,
    simulations_per_estimator: usize,
    sequential_rounds: usize,
    simulations_per_round: usize,
    training: NeuralSbiTraining,
    posterior_grid_points: usize,
    seed: u64,
    timeout_seconds: u64,
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match NeuralCli::parse().command {
        NeuralTopLevel::Neural {
            command: NeuralCommand::PointProcess { input, out },
        } => run_point_process(input, out),
        NeuralTopLevel::Neural {
            command: NeuralCommand::PointSetGenerators { input, out },
        } => run_point_set_generators(input, out),
        NeuralTopLevel::Neural {
            command: NeuralCommand::Sbi { input, out },
        } => run_neural_sbi(input, out),
        NeuralTopLevel::Neural {
            command:
                NeuralCommand::ValidateGenerative {
                    data,
                    model,
                    model_repeat,
                    timeout_seconds,
                    out,
                },
        } => run_validate_generative(data, model, model_repeat, timeout_seconds, out),
    }
}

fn run_validate_generative(
    data: PathBuf,
    model: PathBuf,
    model_repeat: PathBuf,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    if !(1..=3_600).contains(&timeout_seconds) {
        return Err(TopologyCliError::Input(
            "generative validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let data_bytes = read_input(&data)?;
    let model_bytes = read_input(&model)?;
    let repeat_bytes = read_input(&model_repeat)?;
    let data_value: serde_json::Value = serde_json::from_slice(&data_bytes)?;
    let model_value: serde_json::Value = serde_json::from_slice(&model_bytes)?;
    let repeat_value: serde_json::Value = serde_json::from_slice(&repeat_bytes)?;
    if !data_value["patterns"].is_array()
        || model_value["format"] != "marklab.point_set_flow_and_diffusion"
        || repeat_value["format"] != "marklab.point_set_flow_and_diffusion"
    {
        return Err(TopologyCliError::Input(
            "generative validation requires the original point-set data and two point-set generator artifacts"
                .into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_generative_validation_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.generative_validation_request",
        "version": 1,
        "backend": {
            "name": "numpy_scipy_generative_validation",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "data": data_value,
        "data_sha256": sha256_hex(&data_bytes),
        "model": model_value,
        "model_sha256": sha256_hex(&model_bytes),
        "model_repeat": repeat_value,
        "model_repeat_sha256": sha256_hex(&repeat_bytes)
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, timeout_seconds)?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.generative_tissue_model_card"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "synthetic_generative_model_validation_no_real_promotion"
    {
        return Err(TopologyCliError::Backend(
            "generative validation result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn run_neural_sbi(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: NeuralSbiSpec = serde_json::from_slice(&bytes)?;
    validate_neural_sbi(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_sbi_neural_estimators_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.sbi_neural_estimators_request",
        "version": 1,
        "backend": {
            "name": "sbi_torch",
            "version": "sbi-0.26.1",
            "torch_version": "2.13.0",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "device_policy": "deterministic_cpu_only",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "prior": spec.prior,
        "simulator": spec.simulator,
        "observed_summary": spec.observed_summary,
        "simulations_per_estimator": spec.simulations_per_estimator,
        "sequential_rounds": spec.sequential_rounds,
        "simulations_per_round": spec.simulations_per_round,
        "training": spec.training,
        "posterior_grid_points": spec.posterior_grid_points,
        "seed": spec.seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.neural_simulation_based_inference"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_amortized_and_sequential_sbi"
    {
        return Err(TopologyCliError::Backend(
            "neural SBI result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_neural_sbi(spec: &NeuralSbiSpec) -> Result<(), TopologyCliError> {
    if spec.prior.family != "uniform"
        || !spec.prior.lower.is_finite()
        || !spec.prior.upper.is_finite()
        || spec.prior.lower >= spec.prior.upper
        || spec.simulator.family != "gaussian_location"
        || !spec.simulator.noise_standard_deviation.is_finite()
        || spec.simulator.noise_standard_deviation <= 0.0
        || !spec.observed_summary.is_finite()
        || !(spec.prior.lower..=spec.prior.upper).contains(&spec.observed_summary)
        || !(512..=100_000).contains(&spec.simulations_per_estimator)
        || spec.sequential_rounds != 2
        || !(256..=100_000).contains(&spec.simulations_per_round)
        || spec.training.density_estimator != "mdn"
        || spec.training.ratio_classifier != "mlp"
        || !(32..=2_048).contains(&spec.training.batch_size)
        || !(10..=500).contains(&spec.training.maximum_epochs)
        || !(2..=50).contains(&spec.training.stop_after_epochs)
        || spec.training.stop_after_epochs >= spec.training.maximum_epochs
        || !(401..=10_001).contains(&spec.posterior_grid_points)
        || spec.posterior_grid_points.is_multiple_of(2)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "neural SBI requires a bounded uniform/Gaussian-location oracle, distinct NPE/NLE/NRE architectures, two isolated proposal rounds, bounded training, and an odd posterior grid"
                .into(),
        ));
    }
    Ok(())
}

fn run_point_set_generators(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: PointSetGeneratorsSpec = serde_json::from_slice(&bytes)?;
    spec.patterns
        .sort_by(|left, right| left.pattern_id.cmp(&right.pattern_id));
    validate_point_set_generators(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_scipy_point_set_generators_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_point_set_generators_request",
        "version": 1,
        "backend": {
            "name": "numpy_scipy_equivariant_flow_and_diffusion",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "device_policy": "deterministic_cpu_only",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "patterns": spec.patterns,
        "flow": spec.flow,
        "diffusion": spec.diffusion,
        "generated_patterns_per_context": spec.generated_patterns_per_context,
        "seed": spec.seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.point_set_flow_and_diffusion"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_point_set_generators"
    {
        return Err(TopologyCliError::Backend(
            "point-set generator result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_point_set_generators(spec: &PointSetGeneratorsSpec) -> Result<(), TopologyCliError> {
    let context_dimension = spec
        .patterns
        .first()
        .map(|pattern| pattern.context.len())
        .unwrap_or(0);
    let window = spec.patterns.first().map(|pattern| pattern.window);
    let mut pattern_ids = HashSet::new();
    let mut patient_ids = HashSet::new();
    let valid_patterns = spec.patterns.iter().all(|pattern| {
        !pattern.pattern_id.trim().is_empty()
            && pattern.pattern_id.trim() == pattern.pattern_id
            && pattern_ids.insert(pattern.pattern_id.as_str())
            && !pattern.patient_id.trim().is_empty()
            && pattern.patient_id.trim() == pattern.patient_id
            && patient_ids.insert(pattern.patient_id.as_str())
            && matches!(pattern.split.as_str(), "train" | "test")
            && Some(pattern.window) == window
            && pattern.window.iter().all(|value| value.is_finite())
            && pattern.window[0] < pattern.window[1]
            && pattern.window[2] < pattern.window[3]
            && pattern.context.len() == context_dimension
            && pattern.context.iter().all(|value| value.is_finite())
            && (1..=64).contains(&pattern.points.len())
            && pattern.points.iter().all(|point| {
                point.iter().all(|value| value.is_finite())
                    && (pattern.window[0]..=pattern.window[1]).contains(&point[0])
                    && (pattern.window[2]..=pattern.window[3]).contains(&point[1])
            })
    });
    let train_count = spec
        .patterns
        .iter()
        .filter(|pattern| pattern.split == "train")
        .count();
    let test_count = spec
        .patterns
        .iter()
        .filter(|pattern| pattern.split == "test")
        .count();
    if !(1..=8).contains(&context_dimension)
        || !valid_patterns
        || train_count < 12
        || test_count < 4
        || spec.flow.family != "conditional_logistic_normal_iid_equivariant"
        || !spec.flow.boundary_epsilon.is_finite()
        || !(0.0..0.01).contains(&spec.flow.boundary_epsilon)
        || !spec.flow.variance_floor.is_finite()
        || spec.flow.variance_floor <= 0.0
        || spec.diffusion.schedule != "variance_preserving_linear_beta"
        || !(16..=256).contains(&spec.diffusion.steps)
        || !spec.diffusion.beta_min.is_finite()
        || !spec.diffusion.beta_max.is_finite()
        || spec.diffusion.beta_min <= 0.0
        || spec.diffusion.beta_max <= spec.diffusion.beta_min
        || !(4..=128).contains(&spec.generated_patterns_per_context)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "point-set generators require independent train/test patient sets in one exact window/context contract, a bounded equivariant logistic-normal flow, VP diffusion schedule, and bounded sampling"
                .into(),
        ));
    }
    Ok(())
}

fn run_point_process(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: NeuralPointProcessSpec = serde_json::from_slice(&bytes)?;
    spec.patterns
        .sort_by(|left, right| left.pattern_id.cmp(&right.pattern_id));
    validate_point_process(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_neural_point_process_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_neural_point_process_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_neural_point_process",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "device_policy": "deterministic_cpu_only",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "patterns": spec.patterns,
        "mark_labels": spec.mark_labels,
        "architecture": spec.architecture,
        "quadrature_grid": spec.quadrature_grid,
        "quadrature_reference_grid": spec.quadrature_reference_grid,
        "parameter_precision": spec.parameter_precision,
        "maximum_iterations": spec.maximum_iterations,
        "seed": spec.seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.neural_marked_cox_process"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_neural_marked_cox_process"
    {
        return Err(TopologyCliError::Backend(
            "neural point-process result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_point_process(spec: &NeuralPointProcessSpec) -> Result<(), TopologyCliError> {
    let mut labels = HashSet::new();
    let valid_labels = spec.mark_labels.iter().all(|label| {
        !label.trim().is_empty() && label.trim() == label && labels.insert(label.as_str())
    });
    let context_dimension = spec
        .patterns
        .first()
        .map(|pattern| pattern.context.len())
        .unwrap_or(0);
    let mut pattern_ids = HashSet::new();
    let mut patient_ids = HashSet::new();
    let valid_patterns = spec.patterns.iter().all(|pattern| {
        !pattern.pattern_id.trim().is_empty()
            && pattern.pattern_id.trim() == pattern.pattern_id
            && pattern_ids.insert(pattern.pattern_id.as_str())
            && !pattern.patient_id.trim().is_empty()
            && pattern.patient_id.trim() == pattern.patient_id
            && patient_ids.insert(pattern.patient_id.as_str())
            && matches!(pattern.split.as_str(), "train" | "test")
            && pattern.window.iter().all(|value| value.is_finite())
            && pattern.window[0] < pattern.window[1]
            && pattern.window[2] < pattern.window[3]
            && pattern.context.len() == context_dimension
            && pattern.context.iter().all(|value| value.is_finite())
            && (5..=1_000).contains(&pattern.points.len())
            && pattern.points.iter().all(|point| {
                point.coordinates.iter().all(|value| value.is_finite())
                    && (pattern.window[0]..=pattern.window[1]).contains(&point.coordinates[0])
                    && (pattern.window[2]..=pattern.window[3]).contains(&point.coordinates[1])
                    && labels.contains(point.mark.as_str())
            })
    });
    let train_count = spec
        .patterns
        .iter()
        .filter(|pattern| pattern.split == "train")
        .count();
    let test_count = spec
        .patterns
        .iter()
        .filter(|pattern| pattern.split == "test")
        .count();
    if !(2..=16).contains(&spec.mark_labels.len())
        || !valid_labels
        || !(1..=8).contains(&context_dimension)
        || !valid_patterns
        || train_count < 8
        || test_count < 4
        || !(2..=32).contains(&spec.architecture.hidden_units)
        || spec.architecture.activation != "tanh"
        || spec.architecture.intensity_link != "softplus"
        || spec.architecture.mark_link != "softmax"
        || !(8..=64).contains(&spec.quadrature_grid)
        || !(spec.quadrature_grid..=128).contains(&spec.quadrature_reference_grid)
        || spec.quadrature_reference_grid <= spec.quadrature_grid
        || !spec.parameter_precision.is_finite()
        || spec.parameter_precision <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "neural point-process training requires independent train/test patient patterns, exact windows/contexts/marks, a bounded tanh-softplus/softmax network, fixed/reference quadrature, and positive controls"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<NeuralCli>(|| run_cli().map_err(into_marklab_error))
}
