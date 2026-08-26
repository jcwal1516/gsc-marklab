use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use marklab_topology::sha256_hex;
use serde::{Deserialize, Serialize};

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct MultimodalModelCli {
    #[command(subcommand)]
    command: MultimodalTopLevel,
}

#[derive(Debug, Subcommand)]
enum MultimodalTopLevel {
    Multimodal {
        #[command(subcommand)]
        command: MultimodalModelCommand,
    },
}

#[derive(Debug, Subcommand)]
enum MultimodalModelCommand {
    Pcca {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    BayesianPcca {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Mofa {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MatrixFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpatialMatrixFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    TensorFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpatialLatentFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MultiresolutionFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    DropoutRobust {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    JointPathology {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CompareModels {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        outer_folds: u32,
        #[arg(long)]
        inner_folds: u32,
        #[arg(long)]
        ridge_alphas: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    Validate {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ModalityDesign {
    id: String,
    measurement_status: String,
    likelihood: String,
    feature_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PairedMultimodalDesign {
    entity_level: String,
    modality_x: ModalityDesign,
    modality_y: ModalityDesign,
    missingness_assumption: String,
    coordinate_frame: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PairedMultimodalRow {
    entity_id: String,
    split: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbabilisticCcaSpec {
    design: PairedMultimodalDesign,
    rows: Vec<PairedMultimodalRow>,
    latent_dimensions: usize,
    regularization: f64,
    noise_floor: f64,
    maximum_iterations: usize,
    convergence_tolerance: f64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BayesianPccaSpec {
    design: PairedMultimodalDesign,
    rows: Vec<PairedMultimodalRow>,
    latent_dimensions: usize,
    priors: String,
    warmup: usize,
    samples: usize,
    target_accept: f64,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MultiviewDesign {
    entity_level: String,
    modalities: Vec<ModalityDesign>,
    missingness_assumption: String,
    coordinate_frame: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MultiviewValues {
    values: Vec<f64>,
    observed: Vec<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MultiviewRow {
    entity_id: String,
    split: String,
    views: Vec<MultiviewValues>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MofaSpec {
    design: MultiviewDesign,
    rows: Vec<MultiviewRow>,
    maximum_factors: usize,
    iterations: usize,
    convergence_mode: String,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MatrixFactorRow {
    entity_id: String,
    values: Vec<f64>,
    observed: Vec<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MatrixFactorSpec {
    matrix_id: String,
    entity_level: String,
    likelihood: String,
    feature_names: Vec<String>,
    rows: Vec<MatrixFactorRow>,
    factors: usize,
    iterations: usize,
    convergence_mode: String,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HierarchicalEntity {
    entity_id: String,
    level: String,
    parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HierarchicalModality {
    modality_id: String,
    entity_level: String,
    entity_ids: Vec<String>,
    measurement_status: String,
    likelihood: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HierarchicalFactorSpec {
    model_id: String,
    latent_dimensions: usize,
    entities: Vec<HierarchicalEntity>,
    modalities: Vec<HierarchicalModality>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialGraphEdge {
    left: String,
    right: String,
    weight: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialFactorGraph {
    graph_id: String,
    edges: Vec<SpatialGraphEdge>,
    normalization: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpatialMatrixFactorSpec {
    matrix_id: String,
    entity_level: String,
    likelihood: String,
    feature_names: Vec<String>,
    rows: Vec<MatrixFactorRow>,
    graph: SpatialFactorGraph,
    factors: usize,
    spatial_precision: f64,
    diagonal_epsilon: f64,
    loading_precision: f64,
    noise_standard_deviation: f64,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TensorEntry {
    indices: Vec<usize>,
    value: f64,
    observed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TensorFactorSpec {
    tensor_id: String,
    mode_names: Vec<String>,
    shape: Vec<usize>,
    entries: Vec<TensorEntry>,
    decomposition: String,
    ranks: Vec<usize>,
    prior_precision: f64,
    noise_standard_deviation: f64,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialLatentRow {
    entity_id: String,
    coordinates_um: [f64; 2],
    values: Vec<f64>,
    observed: Vec<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpatialLatentFactorSpec {
    matrix_id: String,
    entity_level: String,
    coordinate_frame: String,
    likelihood: String,
    feature_names: Vec<String>,
    rows: Vec<SpatialLatentRow>,
    factors: usize,
    matern_nu: f64,
    length_scale_prior_um: f64,
    noise_standard_deviation: f64,
    warmup: usize,
    samples: usize,
    target_accept: f64,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MultiresolutionScale {
    scale_id: String,
    physical_scale_um: f64,
    basis_columns: Vec<Vec<f64>>,
    factors: usize,
    coefficient_precision: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiresolutionFactorSpec {
    matrix_id: String,
    entity_level: String,
    likelihood: String,
    feature_names: Vec<String>,
    rows: Vec<MatrixFactorRow>,
    scales: Vec<MultiresolutionScale>,
    loading_precision: f64,
    noise_standard_deviation: f64,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DropoutView {
    values: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DropoutRow {
    entity_id: String,
    split: String,
    views: Vec<DropoutView>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DropoutPattern {
    pattern_id: String,
    retained_modalities: Vec<String>,
    probability: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DropoutRobustSpec {
    design: MultiviewDesign,
    rows: Vec<DropoutRow>,
    latent_dimensions: usize,
    dropout_patterns: Vec<DropoutPattern>,
    required_anchor_modalities: Vec<String>,
    consistency_weight: f64,
    parameter_precision: f64,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointRegion {
    region_id: String,
    patient_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointRegionObservation {
    region_id: String,
    morphology: Vec<f64>,
    ihc: Vec<f64>,
    omics_counts: Vec<u64>,
    library_size: f64,
    clone_label: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointPatientOutcome {
    patient_id: String,
    value: f64,
    observed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointObservationBlock {
    measurement_status: String,
    likelihood: String,
    feature_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointModelSpec {
    morphology: JointObservationBlock,
    ihc: JointObservationBlock,
    omics: JointObservationBlock,
    clone: JointObservationBlock,
    clinical: JointObservationBlock,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JointPathologySpec {
    project_id: String,
    patients: Vec<String>,
    regions: Vec<JointRegion>,
    region_observations: Vec<JointRegionObservation>,
    patient_outcomes: Vec<JointPatientOutcome>,
    model_spec: JointModelSpec,
    inference_plan: String,
    region_latent_standard_deviation: f64,
    gaussian_noise_standard_deviation: f64,
    parameter_precision: f64,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match MultimodalModelCli::parse().command {
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::Pcca { input, out },
        } => run_pcca(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::BayesianPcca { input, out },
        } => run_bayesian_pcca(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::Mofa { input, out },
        } => run_mofa(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::MatrixFactor { input, out },
        } => run_matrix_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::HierarchicalFactor { input, out },
        } => run_hierarchical_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::SpatialMatrixFactor { input, out },
        } => run_spatial_matrix_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::TensorFactor { input, out },
        } => run_tensor_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::SpatialLatentFactor { input, out },
        } => run_spatial_latent_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::MultiresolutionFactor { input, out },
        } => run_multiresolution_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::DropoutRobust { input, out },
        } => run_dropout_robust(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::JointPathology { input, out },
        } => run_joint_pathology(input, out),
        MultimodalTopLevel::Multimodal {
            command:
                MultimodalModelCommand::CompareModels {
                    input,
                    outer_folds,
                    inner_folds,
                    ridge_alphas,
                    permutations,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => super::bayes::complementarity::run_multimodal_comparison(
            input,
            outer_folds,
            inner_folds,
            ridge_alphas,
            permutations,
            seed,
            timeout_seconds,
            out,
        )
        .map_err(|error| TopologyCliError::Backend(error.to_string())),
        MultimodalTopLevel::Multimodal {
            command:
                MultimodalModelCommand::Validate {
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_validation_suite(seed, timeout_seconds, out),
    }
}

fn run_validation_suite(
    seed: u64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    if !(1..=3_600).contains(&timeout_seconds) {
        return Err(TopologyCliError::Input(
            "multimodal validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_multimodal_validation_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.multimodal_validation_request",
        "version": 1,
        "backend": {
            "name": "numpy_scipy_exact_synthetic_controls",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "seed": seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, timeout_seconds)?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.multimodal_bayesian_validation_suite"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || !matches!(
            result["overall_status"].as_str(),
            Some("partial_external_evidence_required" | "failed_synthetic_control")
        )
        || result["claim_status"] != "synthetic_validation_ledger_with_explicit_gaps"
    {
        return Err(TopologyCliError::Backend(
            "multimodal validation result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn run_joint_pathology(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: JointPathologySpec = serde_json::from_slice(&bytes)?;
    spec.patients.sort();
    spec.regions
        .sort_by(|left, right| left.region_id.cmp(&right.region_id));
    spec.region_observations
        .sort_by(|left, right| left.region_id.cmp(&right.region_id));
    spec.patient_outcomes
        .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
    validate_joint_pathology(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_joint_pathology_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_joint_pathology_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_l_bfgs_laplace",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "project_id": spec.project_id,
        "patients": spec.patients,
        "regions": spec.regions,
        "region_observations": spec.region_observations,
        "patient_outcomes": spec.patient_outcomes,
        "model_spec": spec.model_spec,
        "inference_plan": spec.inference_plan,
        "region_latent_standard_deviation": spec.region_latent_standard_deviation,
        "gaussian_noise_standard_deviation": spec.gaussian_noise_standard_deviation,
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
    if result["format"] != "marklab.joint_pathology_model_fit"
        || result["model_ir"]["format"] != "marklab.joint_pathology_model_ir"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_joint_pathology_model"
    {
        return Err(TopologyCliError::Backend(
            "joint pathology result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_joint_pathology(spec: &JointPathologySpec) -> Result<(), TopologyCliError> {
    let mut patients = HashSet::new();
    let valid_patients = spec.patients.iter().all(|patient| {
        !patient.trim().is_empty() && patient.trim() == patient && patients.insert(patient.as_str())
    });
    let mut regions = HashSet::new();
    let valid_regions = spec.regions.iter().all(|region| {
        !region.region_id.trim().is_empty()
            && region.region_id.trim() == region.region_id
            && regions.insert(region.region_id.as_str())
            && patients.contains(region.patient_id.as_str())
    });
    let mut observed_regions = HashSet::new();
    let valid_region_observations = spec.region_observations.iter().all(|observation| {
        regions.contains(observation.region_id.as_str())
            && observed_regions.insert(observation.region_id.as_str())
            && observation.morphology.len() == spec.model_spec.morphology.feature_names.len()
            && observation.ihc.len() == spec.model_spec.ihc.feature_names.len()
            && observation.omics_counts.len() == spec.model_spec.omics.feature_names.len()
            && observation
                .morphology
                .iter()
                .chain(&observation.ihc)
                .all(|value| value.is_finite())
            && observation.library_size.is_finite()
            && observation.library_size > 0.0
            && observation.clone_label <= 1
    });
    let mut outcome_patients = HashSet::new();
    let valid_outcomes = spec.patient_outcomes.iter().all(|outcome| {
        patients.contains(outcome.patient_id.as_str())
            && outcome_patients.insert(outcome.patient_id.as_str())
            && outcome.value.is_finite()
    });
    let blocks = [
        (&spec.model_spec.morphology, "gaussian"),
        (&spec.model_spec.ihc, "gaussian"),
        (&spec.model_spec.omics, "poisson"),
        (&spec.model_spec.clone, "bernoulli"),
        (&spec.model_spec.clinical, "gaussian"),
    ];
    let valid_blocks = blocks.iter().all(|(block, likelihood)| {
        let mut names = HashSet::new();
        block.measurement_status == "measured"
            && block.likelihood == *likelihood
            && !block.feature_names.is_empty()
            && block.feature_names.len() <= 32
            && block.feature_names.iter().all(|name| {
                !name.trim().is_empty() && name.trim() == name && names.insert(name.as_str())
            })
    }) && spec.model_spec.clone.feature_names.len() == 1
        && spec.model_spec.clinical.feature_names.len() == 1;
    if spec.project_id.trim().is_empty()
        || spec.project_id.trim() != spec.project_id
        || !(4..=64).contains(&spec.patients.len())
        || !valid_patients
        || !(spec.patients.len()..=256).contains(&spec.regions.len())
        || !valid_regions
        || spec.region_observations.len() != spec.regions.len()
        || !valid_region_observations
        || observed_regions.len() != regions.len()
        || spec.patient_outcomes.len() != spec.patients.len()
        || !valid_outcomes
        || outcome_patients.len() != patients.len()
        || spec
            .patient_outcomes
            .iter()
            .filter(|outcome| outcome.observed)
            .count()
            < 3
        || !spec
            .patient_outcomes
            .iter()
            .any(|outcome| !outcome.observed)
        || !valid_blocks
        || spec.inference_plan != "laplace"
        || !spec.region_latent_standard_deviation.is_finite()
        || spec.region_latent_standard_deviation <= 0.0
        || !spec.gaussian_noise_standard_deviation.is_finite()
        || spec.gaussian_noise_standard_deviation <= 0.0
        || !spec.parameter_precision.is_finite()
        || spec.parameter_precision <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "joint pathology requires complete patient-region ownership, five measured typed likelihood blocks, masked patient outcomes, and bounded Laplace controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_dropout_robust(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: DropoutRobustSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    spec.dropout_patterns
        .sort_by(|left, right| left.pattern_id.cmp(&right.pattern_id));
    spec.required_anchor_modalities.sort();
    validate_dropout_robust(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_modality_dropout_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_modality_dropout_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_predictive_objective",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "design": spec.design,
        "rows": spec.rows,
        "latent_dimensions": spec.latent_dimensions,
        "dropout_patterns": spec.dropout_patterns,
        "required_anchor_modalities": spec.required_anchor_modalities,
        "consistency_weight": spec.consistency_weight,
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
    if result["format"] != "marklab.modality_robust_inference_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_modality_dropout_robustness"
    {
        return Err(TopologyCliError::Backend(
            "modality dropout result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_dropout_robust(spec: &DropoutRobustSpec) -> Result<(), TopologyCliError> {
    let train_count = spec.rows.iter().filter(|row| row.split == "train").count();
    let test_count = spec.rows.iter().filter(|row| row.split == "test").count();
    let mut modality_ids = HashSet::new();
    let valid_modalities = spec.design.modalities.iter().all(|modality| {
        let mut features = HashSet::new();
        !modality.id.trim().is_empty()
            && modality.id.trim() == modality.id
            && modality_ids.insert(modality.id.as_str())
            && modality.measurement_status == "measured"
            && modality.likelihood == "gaussian"
            && (1..=64).contains(&modality.feature_names.len())
            && modality.feature_names.iter().all(|feature| {
                !feature.trim().is_empty()
                    && feature.trim() == feature
                    && features.insert(feature.as_str())
            })
    });
    let mut entities = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && matches!(row.split.as_str(), "train" | "test")
            && row.views.len() == spec.design.modalities.len()
            && row
                .views
                .iter()
                .zip(&spec.design.modalities)
                .all(|(view, modality)| {
                    view.values.len() == modality.feature_names.len()
                        && view.values.iter().all(|value| value.is_finite())
                })
    });
    let anchors = spec
        .required_anchor_modalities
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let valid_anchors = !anchors.is_empty()
        && anchors.len() == spec.required_anchor_modalities.len()
        && anchors.iter().all(|anchor| modality_ids.contains(anchor));
    let mut pattern_ids = HashSet::new();
    let valid_patterns = spec.dropout_patterns.iter().all(|pattern| {
        let retained = pattern
            .retained_modalities
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        !pattern.pattern_id.trim().is_empty()
            && pattern.pattern_id.trim() == pattern.pattern_id
            && pattern_ids.insert(pattern.pattern_id.as_str())
            && !retained.is_empty()
            && retained.len() == pattern.retained_modalities.len()
            && retained.len() < spec.design.modalities.len()
            && retained.iter().all(|id| modality_ids.contains(id))
            && retained.iter().any(|id| anchors.contains(id))
            && pattern.probability.is_finite()
            && pattern.probability > 0.0
    });
    let probability_sum = spec
        .dropout_patterns
        .iter()
        .map(|pattern| pattern.probability)
        .sum::<f64>();
    let maximum_latent = spec
        .design
        .modalities
        .iter()
        .map(|modality| modality.feature_names.len())
        .min()
        .unwrap_or(0);
    if spec.design.entity_level != "patient"
        || spec.design.missingness_assumption != "structurally_absent_or_mar"
        || spec.design.coordinate_frame.is_some()
        || !(2..=8).contains(&spec.design.modalities.len())
        || !valid_modalities
        || !valid_rows
        || train_count < 12
        || test_count < 4
        || !(1..=maximum_latent).contains(&spec.latent_dimensions)
        || !(1..=32).contains(&spec.dropout_patterns.len())
        || !valid_anchors
        || !valid_patterns
        || (probability_sum - 1.0).abs() > 1e-12
        || !spec.consistency_weight.is_finite()
        || spec.consistency_weight < 0.0
        || !spec.parameter_precision.is_finite()
        || spec.parameter_precision <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "modality dropout requires measured patient Gaussian views, isolated train/test rows, normalized anchor-preserving missing-view patterns, and bounded optimization controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_multiresolution_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: MultiresolutionFactorSpec = serde_json::from_slice(&bytes)?;
    spec.scales
        .sort_by(|left, right| left.scale_id.cmp(&right.scale_id));
    validate_multiresolution_factor(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_jax_multiresolution_factor_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_multiresolution_factor_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_l_bfgs_laplace",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "scales": spec.scales,
        "loading_precision": spec.loading_precision,
        "noise_standard_deviation": spec.noise_standard_deviation,
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
    if result["format"] != "marklab.multiresolution_spatial_factor_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_multiresolution_spatial_factors"
    {
        return Err(TopologyCliError::Backend(
            "multiresolution factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_multiresolution_factor(
    spec: &MultiresolutionFactorSpec,
) -> Result<(), TopologyCliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let mut previous_entity: Option<&str> = None;
    let valid_rows = spec.rows.iter().all(|row| {
        let canonical_order =
            previous_entity.is_none_or(|previous| previous < row.entity_id.as_str());
        previous_entity = Some(row.entity_id.as_str());
        canonical_order
            && !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    let mut scale_ids = HashSet::new();
    let mut physical_scales = HashSet::new();
    let valid_scales = spec.scales.iter().all(|scale| {
        !scale.scale_id.trim().is_empty()
            && scale.scale_id.trim() == scale.scale_id
            && scale_ids.insert(scale.scale_id.as_str())
            && scale.physical_scale_um.is_finite()
            && scale.physical_scale_um > 0.0
            && physical_scales.insert(scale.physical_scale_um.to_bits())
            && (1..=16).contains(&scale.basis_columns.len())
            && scale.basis_columns.iter().all(|column| {
                column.len() == spec.rows.len()
                    && column.iter().all(|value| value.is_finite())
                    && column.iter().any(|value| *value != 0.0)
            })
            && (1..=scale.basis_columns.len().min(feature_count)).contains(&scale.factors)
            && scale.coefficient_precision.is_finite()
            && scale.coefficient_precision > 0.0
    });
    let parameter_count = spec.scales.iter().fold(0usize, |count, scale| {
        count.saturating_add((scale.basis_columns.len() + feature_count) * scale.factors)
    });
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "region"
        || spec.likelihood != "gaussian"
        || !(2..=32).contains(&feature_count)
        || !valid_features
        || !(8..=256).contains(&spec.rows.len())
        || !valid_rows
        || !enough_observed
        || !spec
            .rows
            .iter()
            .any(|row| row.observed.iter().any(|value| !*value))
        || !(2..=8).contains(&spec.scales.len())
        || !valid_scales
        || parameter_count > 512
        || !spec.loading_precision.is_finite()
        || spec.loading_precision <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "multiresolution factors require a bounded region Gaussian matrix, unique declared physical scales with finite bases, masked evaluation entries, and positive Laplace controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_spatial_latent_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: SpatialLatentFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_spatial_latent_factor(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_pymc_spatial_latent_factor_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.pymc_spatial_latent_factor_request",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": "pymc-6.3.0",
            "pytensor_version": "3.2.4",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "coordinate_frame": spec.coordinate_frame,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "factors": spec.factors,
        "matern_nu": spec.matern_nu,
        "length_scale_prior_um": spec.length_scale_prior_um,
        "noise_standard_deviation": spec.noise_standard_deviation,
        "warmup": spec.warmup,
        "samples": spec.samples,
        "target_accept": spec.target_accept,
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
    if result["format"] != "marklab.spatial_latent_factor_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || !matches!(
            result["fit_state"].as_str(),
            Some("complete" | "nonconverged")
        )
    {
        return Err(TopologyCliError::Backend(
            "spatial latent factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_spatial_latent_factor(spec: &SpatialLatentFactorSpec) -> Result<(), TopologyCliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let mut coordinates = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.coordinates_um.iter().all(|value| value.is_finite())
            && coordinates.insert((
                row.coordinates_um[0].to_bits(),
                row.coordinates_um[1].to_bits(),
            ))
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "region"
        || spec.coordinate_frame.trim().is_empty()
        || spec.coordinate_frame.trim() != spec.coordinate_frame
        || spec.likelihood != "gaussian"
        || !(2..=16).contains(&feature_count)
        || !valid_features
        || !(8..=64).contains(&spec.rows.len())
        || !valid_rows
        || !enough_observed
        || !spec
            .rows
            .iter()
            .any(|row| row.observed.iter().any(|value| !*value))
        || spec.factors != 1
        || spec.matern_nu != 1.5
        || !spec.length_scale_prior_um.is_finite()
        || spec.length_scale_prior_um <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(20..=2_000).contains(&spec.warmup)
        || !(20..=2_000).contains(&spec.samples)
        || !spec.target_accept.is_finite()
        || !(0.8..1.0).contains(&spec.target_accept)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "spatial latent factors require unique finite region coordinates, a bounded Gaussian matrix with masked evaluation entries, one Matern-3/2 factor, and bounded NUTS controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_tensor_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: TensorFactorSpec = serde_json::from_slice(&bytes)?;
    spec.entries
        .sort_by(|left, right| left.indices.cmp(&right.indices));
    validate_tensor_factor(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_scipy_tensor_factor_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_scipy_tensor_factor_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_l_bfgs_laplace",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "tensor_id": spec.tensor_id,
        "mode_names": spec.mode_names,
        "shape": spec.shape,
        "entries": spec.entries,
        "decomposition": spec.decomposition,
        "ranks": spec.ranks,
        "prior_precision": spec.prior_precision,
        "noise_standard_deviation": spec.noise_standard_deviation,
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
    let expected_format = match spec.decomposition.as_str() {
        "cp" => "marklab.bayesian_cp_factorization",
        "tucker" => "marklab.bayesian_tucker_factorization",
        _ => unreachable!("validated decomposition"),
    };
    let expected_claim = format!(
        "experimental_synthetic_bayesian_{}_factorization",
        spec.decomposition
    );
    if result["format"] != expected_format
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != expected_claim
    {
        return Err(TopologyCliError::Backend(
            "tensor factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_tensor_factor(spec: &TensorFactorSpec) -> Result<(), TopologyCliError> {
    let mut modes = HashSet::new();
    let valid_modes = spec
        .mode_names
        .iter()
        .all(|mode| !mode.trim().is_empty() && mode.trim() == mode && modes.insert(mode.as_str()));
    let total_entries = spec
        .shape
        .iter()
        .try_fold(1usize, |total, dimension| total.checked_mul(*dimension));
    let mut indices = HashSet::new();
    let valid_entries = spec.entries.iter().all(|entry| {
        entry.indices.len() == 3
            && entry
                .indices
                .iter()
                .zip(&spec.shape)
                .all(|(index, dimension)| index < dimension)
            && entry.value.is_finite()
            && indices.insert(entry.indices.as_slice())
    });
    let valid_ranks = match spec.decomposition.as_str() {
        "cp" => {
            spec.ranks.len() == 1
                && (1..=spec.shape.iter().copied().min().unwrap_or(0)).contains(&spec.ranks[0])
        }
        "tucker" => {
            spec.ranks.len() == 3
                && spec
                    .ranks
                    .iter()
                    .zip(&spec.shape)
                    .all(|(rank, dimension)| (1..=*dimension).contains(rank))
        }
        _ => false,
    };
    let parameter_count = if spec.decomposition == "cp" && spec.ranks.len() == 1 {
        spec.shape.iter().sum::<usize>() * spec.ranks[0]
    } else if spec.ranks.len() == 3 {
        spec.shape
            .iter()
            .zip(&spec.ranks)
            .map(|(dimension, rank)| dimension * rank)
            .sum::<usize>()
            + spec.ranks.iter().product::<usize>()
    } else {
        usize::MAX
    };
    if spec.tensor_id.trim().is_empty()
        || spec.tensor_id.trim() != spec.tensor_id
        || spec.mode_names.len() != 3
        || spec.shape.len() != 3
        || !valid_modes
        || spec
            .shape
            .iter()
            .any(|dimension| !(2..=16).contains(dimension))
        || total_entries != Some(spec.entries.len())
        || !valid_entries
        || !spec.entries.iter().any(|entry| !entry.observed)
        || spec.entries.iter().filter(|entry| entry.observed).count() < 8
        || !valid_ranks
        || parameter_count > 512
        || !spec.prior_precision.is_finite()
        || spec.prior_precision <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "tensor factorization requires one complete bounded three-mode tensor index, held-out entries, valid CP/Tucker ranks, and positive Laplace controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_spatial_matrix_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: SpatialMatrixFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    spec.graph
        .edges
        .sort_by(|left, right| (&left.left, &left.right).cmp(&(&right.left, &right.right)));
    validate_spatial_matrix_factor(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_scipy_spatial_matrix_factor_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_spatial_matrix_factor_request",
        "version": 1,
        "backend": {
            "name": "scipy_l_bfgs_laplace",
            "version": "scipy-1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "graph": spec.graph,
        "factors": spec.factors,
        "spatial_precision": spec.spatial_precision,
        "diagonal_epsilon": spec.diagonal_epsilon,
        "loading_precision": spec.loading_precision,
        "noise_standard_deviation": spec.noise_standard_deviation,
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
    if result["format"] != "marklab.spatial_bayesian_matrix_factorization"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_spatial_matrix_factorization"
    {
        return Err(TopologyCliError::Backend(
            "spatial matrix factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_spatial_matrix_factor(spec: &SpatialMatrixFactorSpec) -> Result<(), TopologyCliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    let mut edge_keys = HashSet::new();
    let mut incident = HashSet::new();
    let valid_edges = spec.graph.edges.iter().all(|edge| {
        let valid = edge.left < edge.right
            && entities.contains(edge.left.as_str())
            && entities.contains(edge.right.as_str())
            && edge.weight.is_finite()
            && edge.weight > 0.0
            && edge_keys.insert((edge.left.as_str(), edge.right.as_str()));
        if valid {
            incident.insert(edge.left.as_str());
            incident.insert(edge.right.as_str());
        }
        valid
    });
    let total_parameters = (spec.rows.len() + feature_count).saturating_mul(spec.factors);
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "region"
        || spec.likelihood != "gaussian"
        || !(2..=64).contains(&feature_count)
        || !valid_features
        || !(8..=256).contains(&spec.rows.len())
        || !valid_rows
        || !spec
            .rows
            .iter()
            .any(|row| row.observed.iter().any(|value| !*value))
        || !enough_observed
        || spec.graph.graph_id.trim().is_empty()
        || spec.graph.normalization != "unnormalized_laplacian"
        || spec.graph.edges.is_empty()
        || !valid_edges
        || incident.len() != spec.rows.len()
        || !(1..=feature_count.min(spec.rows.len() - 1)).contains(&spec.factors)
        || total_parameters > 512
        || !spec.spatial_precision.is_finite()
        || spec.spatial_precision <= 0.0
        || !spec.diagonal_epsilon.is_finite()
        || spec.diagonal_epsilon <= 0.0
        || !spec.loading_precision.is_finite()
        || spec.loading_precision <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "spatial matrix factorization requires a bounded region Gaussian matrix, canonical positive graph edges covering every entity, masked evaluation values, and positive Laplace controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_hierarchical_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: HierarchicalFactorSpec = serde_json::from_slice(&bytes)?;
    validate_hierarchical_factor(&spec)?;
    spec.entities
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    spec.modalities
        .sort_by(|left, right| left.modality_id.cmp(&right.modality_id));
    let latent_nodes = spec
        .entities
        .iter()
        .map(|entity| {
            serde_json::json!({
                "entity_id": entity.entity_id,
                "level": entity.level,
                "dimensions": spec.latent_dimensions,
                "prior": if entity.level == "patient" { "standard_normal" } else { "conditional_gaussian" }
            })
        })
        .collect::<Vec<_>>();
    let conditional_edges = spec
        .entities
        .iter()
        .filter_map(|entity| {
            entity.parent_id.as_ref().map(|parent| {
                serde_json::json!({
                    "parent_id": parent,
                    "child_id": entity.entity_id,
                    "child_level": entity.level,
                    "transition": "linear_gaussian"
                })
            })
        })
        .collect::<Vec<_>>();
    let observation_attachments = spec
        .modalities
        .iter()
        .flat_map(|modality| {
            modality.entity_ids.iter().map(|entity_id| {
                serde_json::json!({
                    "modality_id": modality.modality_id,
                    "entity_id": entity_id,
                    "entity_level": modality.entity_level,
                    "measurement_status": modality.measurement_status,
                    "likelihood": modality.likelihood
                })
            })
        })
        .collect::<Vec<_>>();
    let result = serde_json::json!({
        "format": "marklab.hierarchical_factor_graph",
        "version": 1,
        "model_id": spec.model_id,
        "latent_dimensions": spec.latent_dimensions,
        "replication_unit": "patient",
        "latent_nodes": latent_nodes,
        "conditional_edges": conditional_edges,
        "observation_attachments": observation_attachments,
        "direct_cell_patient_replication": false,
        "claim_status": "compiled_hierarchical_factor_graph"
    });
    publish_json(&out, &result)
}

fn validate_hierarchical_factor(spec: &HierarchicalFactorSpec) -> Result<(), TopologyCliError> {
    let mut entity_ids = HashSet::new();
    let entity_map = spec
        .entities
        .iter()
        .map(|entity| (entity.entity_id.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let valid_entities = spec.entities.iter().all(|entity| {
        let expected_parent = match entity.level.as_str() {
            "patient" => None,
            "specimen" => Some("patient"),
            "region" => Some("specimen"),
            "cell" => Some("region"),
            _ => return false,
        };
        let parent_level = entity
            .parent_id
            .as_deref()
            .and_then(|parent| entity_map.get(parent))
            .map(|parent| parent.level.as_str());
        !entity.entity_id.trim().is_empty()
            && entity.entity_id.trim() == entity.entity_id
            && entity_ids.insert(entity.entity_id.as_str())
            && parent_level == expected_parent
    });
    let mut modality_ids = HashSet::new();
    let valid_modalities = spec.modalities.iter().all(|modality| {
        let mut attachments = HashSet::new();
        !modality.modality_id.trim().is_empty()
            && modality.modality_id.trim() == modality.modality_id
            && modality_ids.insert(modality.modality_id.as_str())
            && matches!(
                modality.entity_level.as_str(),
                "patient" | "specimen" | "region" | "cell"
            )
            && matches!(
                modality.measurement_status.as_str(),
                "measured" | "imported_prediction" | "morphology_prediction"
            )
            && matches!(
                modality.likelihood.as_str(),
                "gaussian"
                    | "bernoulli"
                    | "binomial"
                    | "poisson"
                    | "negative_binomial"
                    | "ordinal"
                    | "categorical"
            )
            && !modality.entity_ids.is_empty()
            && modality.entity_ids.iter().all(|entity_id| {
                attachments.insert(entity_id.as_str())
                    && entity_map
                        .get(entity_id.as_str())
                        .is_some_and(|entity| entity.level == modality.entity_level)
            })
    });
    if spec.model_id.trim().is_empty()
        || spec.model_id.trim() != spec.model_id
        || !(1..=32).contains(&spec.latent_dimensions)
        || spec.entities.is_empty()
        || spec.entities.len() > 100_000
        || !spec.entities.iter().any(|entity| entity.level == "patient")
        || !valid_entities
        || spec.modalities.is_empty()
        || !valid_modalities
    {
        return Err(TopologyCliError::Input(
            "hierarchical factors require exact patient-specimen-region-cell parentage and level-matched measured or predicted modality attachments"
                .into(),
        ));
    }
    Ok(())
}

fn run_matrix_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: MatrixFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_matrix_factor(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_mofapy2_matrix_factor_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.mofapy2_matrix_factor_request",
        "version": 1,
        "backend": {
            "name": "mofapy2",
            "version": "mofapy2-0.7.4",
            "h5py_version": "3.16.0",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "LGPL-3.0",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "factors": spec.factors,
        "iterations": spec.iterations,
        "convergence_mode": spec.convergence_mode,
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
    if result["format"] != "marklab.bayesian_matrix_factorization"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_bayesian_matrix_factorization"
    {
        return Err(TopologyCliError::Backend(
            "matrix factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_matrix_factor(spec: &MatrixFactorSpec) -> Result<(), TopologyCliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let has_masked = spec
        .rows
        .iter()
        .any(|row| row.observed.iter().any(|value| !*value));
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "patient"
        || spec.likelihood != "gaussian"
        || !(2..=128).contains(&feature_count)
        || !valid_features
        || !(8..=10_000).contains(&spec.rows.len())
        || !valid_rows
        || !has_masked
        || !enough_observed
        || !(1..=feature_count.min(spec.rows.len() - 1)).contains(&spec.factors)
        || !(50..=10_000).contains(&spec.iterations)
        || !matches!(spec.convergence_mode.as_str(), "fast" | "medium" | "slow")
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "matrix factorization requires a bounded patient Gaussian matrix, exact identifiers, observed training values, masked evaluation values, and bounded fit controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_mofa(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: MofaSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_mofa(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_mofapy2_multiview_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.mofapy2_multiview_request",
        "version": 1,
        "backend": {
            "name": "mofapy2",
            "version": "mofapy2-0.7.4",
            "h5py_version": "3.16.0",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "LGPL-3.0",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "design": spec.design,
        "rows": spec.rows,
        "maximum_factors": spec.maximum_factors,
        "iterations": spec.iterations,
        "convergence_mode": spec.convergence_mode,
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
    if result["format"] != "marklab.multiview_factor_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["design"]["validation_status"] != "passed"
        || result["claim_status"] != "experimental_synthetic_multiview_factor_model"
    {
        return Err(TopologyCliError::Backend(
            "MOFA result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_mofa(spec: &MofaSpec) -> Result<(), TopologyCliError> {
    let train_count = spec.rows.iter().filter(|row| row.split == "train").count();
    let test_count = spec.rows.iter().filter(|row| row.split == "test").count();
    let total_features = spec
        .design
        .modalities
        .iter()
        .map(|modality| modality.feature_names.len())
        .sum::<usize>();
    let mut modality_ids = HashSet::new();
    let valid_modalities = spec.design.modalities.iter().all(|modality| {
        let mut features = HashSet::new();
        !modality.id.trim().is_empty()
            && modality.id.trim() == modality.id
            && modality_ids.insert(modality.id.as_str())
            && modality.measurement_status == "measured"
            && modality.likelihood == "gaussian"
            && (1..=128).contains(&modality.feature_names.len())
            && modality.feature_names.iter().all(|feature| {
                !feature.trim().is_empty()
                    && feature.trim() == feature
                    && features.insert(feature.as_str())
            })
    });
    let mut entity_ids = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entity_ids.insert(row.entity_id.as_str())
            && matches!(row.split.as_str(), "train" | "test")
            && row.views.len() == spec.design.modalities.len()
            && row
                .views
                .iter()
                .zip(&spec.design.modalities)
                .all(|(view, modality)| {
                    view.values.len() == modality.feature_names.len()
                        && view.observed.len() == modality.feature_names.len()
                        && view.values.iter().all(|value| value.is_finite())
                })
            && (row.split != "test"
                || (row
                    .views
                    .iter()
                    .any(|view| view.observed.iter().any(|value| *value))
                    && row
                        .views
                        .iter()
                        .any(|view| view.observed.iter().any(|value| !*value))))
    });
    if spec.design.entity_level != "patient"
        || spec.design.missingness_assumption != "structurally_absent_or_mar"
        || spec
            .design
            .coordinate_frame
            .as_ref()
            .is_some_and(|frame| frame.trim().is_empty())
        || !(2..=8).contains(&spec.design.modalities.len())
        || !valid_modalities
        || !valid_rows
        || train_count < 12
        || test_count == 0
        || !(1..=total_features.min(train_count - 1)).contains(&spec.maximum_factors)
        || !(50..=10_000).contains(&spec.iterations)
        || !matches!(spec.convergence_mode.as_str(), "fast" | "medium" | "slow")
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "MOFA requires valid measured patient Gaussian views, structural/MAR masks, train/held-out rows, and bounded fit controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_bayesian_pcca(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: BayesianPccaSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_common_design(&spec.design, &spec.rows, spec.latent_dimensions)?;
    if spec.latent_dimensions != 1
        || spec.priors != "cca_zoo_standard_normal_loadings_log_noise"
        || !(20..=10_000).contains(&spec.warmup)
        || !(20..=10_000).contains(&spec.samples)
        || !spec.target_accept.is_finite()
        || !(0.8..1.0).contains(&spec.target_accept)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "Bayesian pCCA requires pinned priors, one factor, bounded draws, target acceptance, and timeout"
                .into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_ccazoo_bayesian_pcca_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.ccazoo_bayesian_pcca_request",
        "version": 1,
        "backend": {
            "name": "cca_zoo_numpyro_jax",
            "cca_zoo_version": "3.0.0",
            "numpyro_version": "0.21.0",
            "jax_version": "0.11.1",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "MIT_plus_Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "design": spec.design,
        "rows": spec.rows,
        "latent_dimensions": spec.latent_dimensions,
        "priors": spec.priors,
        "warmup": spec.warmup,
        "samples": spec.samples,
        "target_accept": spec.target_accept,
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
    if result["format"] != "marklab.bayesian_pcca"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["design"]["validation_status"] != "passed"
        || !matches!(
            result["claim_status"].as_str(),
            Some("experimental_synthetic_bayesian_pcca")
                | Some("unsupported_for_claim_nonconverged")
        )
    {
        return Err(TopologyCliError::Backend(
            "Bayesian pCCA result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn run_pcca(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: ProbabilisticCcaSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_pcca(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_pcca_em_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_pcca_em_request",
        "version": 1,
        "backend": {
            "name": "numpy_scipy",
            "version": "numpy-2.4.6+scipy-1.18.1",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "design": spec.design,
        "rows": spec.rows,
        "latent_dimensions": spec.latent_dimensions,
        "regularization": spec.regularization,
        "noise_floor": spec.noise_floor,
        "maximum_iterations": spec.maximum_iterations,
        "convergence_tolerance": spec.convergence_tolerance
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.probabilistic_cca"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["design"]["validation_status"] != "passed"
        || result["claim_status"] != "experimental_synthetic_paired_gaussian_pcca"
    {
        return Err(TopologyCliError::Backend(
            "probabilistic CCA result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_pcca(spec: &ProbabilisticCcaSpec) -> Result<(), TopologyCliError> {
    let dx = spec.design.modality_x.feature_names.len();
    let dy = spec.design.modality_y.feature_names.len();
    let valid_design = spec.design.entity_level == "patient"
        && spec.design.modality_x.id != spec.design.modality_y.id
        && spec.design.modality_x.measurement_status == "measured"
        && spec.design.modality_y.measurement_status == "measured"
        && spec.design.modality_x.likelihood == "gaussian"
        && spec.design.modality_y.likelihood == "gaussian"
        && spec.design.missingness_assumption == "complete_paired_rows"
        && spec
            .design
            .coordinate_frame
            .as_ref()
            .is_none_or(|frame| !frame.trim().is_empty());
    let mut entity_ids = HashSet::new();
    let train_count = spec.rows.iter().filter(|row| row.split == "train").count();
    let test_count = spec.rows.iter().filter(|row| row.split == "test").count();
    if !valid_design
        || !(1..=32).contains(&dx)
        || !(1..=32).contains(&dy)
        || spec.rows.len() < 9
        || spec.rows.len() > 10_000
        || train_count < 8
        || test_count == 0
        || !(1..=dx.min(dy)).contains(&spec.latent_dimensions)
        || !spec.regularization.is_finite()
        || spec.regularization < 0.0
        || !spec.noise_floor.is_finite()
        || spec.noise_floor <= 0.0
        || !(2..=10_000).contains(&spec.maximum_iterations)
        || !spec.convergence_tolerance.is_finite()
        || spec.convergence_tolerance <= 0.0
        || !(1..=3_600).contains(&spec.timeout_seconds)
        || spec.rows.iter().any(|row| {
            row.entity_id.trim().is_empty()
                || row.entity_id.trim() != row.entity_id
                || !entity_ids.insert(row.entity_id.as_str())
                || !matches!(row.split.as_str(), "train" | "test")
                || row.x.len() != dx
                || row.y.len() != dy
                || row.x.iter().chain(&row.y).any(|value| !value.is_finite())
        })
    {
        return Err(TopologyCliError::Input(
            "pCCA requires valid paired measured patient Gaussian modalities, train/test rows, dimensions, and numerical controls"
                .into(),
        ));
    }
    for names in [
        &spec.design.modality_x.feature_names,
        &spec.design.modality_y.feature_names,
    ] {
        let mut unique = HashSet::new();
        if names.iter().any(|name| {
            name.trim().is_empty() || name.trim() != name || !unique.insert(name.as_str())
        }) {
            return Err(TopologyCliError::Input(
                "pCCA feature names must be unique exact strings within each modality".into(),
            ));
        }
    }
    Ok(())
}

fn validate_common_design(
    design: &PairedMultimodalDesign,
    rows: &[PairedMultimodalRow],
    latent_dimensions: usize,
) -> Result<(), TopologyCliError> {
    let dx = design.modality_x.feature_names.len();
    let dy = design.modality_y.feature_names.len();
    let valid_design = design.entity_level == "patient"
        && design.modality_x.id != design.modality_y.id
        && design.modality_x.measurement_status == "measured"
        && design.modality_y.measurement_status == "measured"
        && design.modality_x.likelihood == "gaussian"
        && design.modality_y.likelihood == "gaussian"
        && design.missingness_assumption == "complete_paired_rows"
        && design
            .coordinate_frame
            .as_ref()
            .is_none_or(|frame| !frame.trim().is_empty());
    let train_count = rows.iter().filter(|row| row.split == "train").count();
    let test_count = rows.iter().filter(|row| row.split == "test").count();
    let mut entity_ids = HashSet::new();
    if !valid_design
        || !(1..=32).contains(&dx)
        || !(1..=32).contains(&dy)
        || rows.len() < 9
        || rows.len() > 10_000
        || train_count < 8
        || test_count == 0
        || !(1..=dx.min(dy)).contains(&latent_dimensions)
        || rows.iter().any(|row| {
            row.entity_id.trim().is_empty()
                || row.entity_id.trim() != row.entity_id
                || !entity_ids.insert(row.entity_id.as_str())
                || !matches!(row.split.as_str(), "train" | "test")
                || row.x.len() != dx
                || row.y.len() != dy
                || row.x.iter().chain(&row.y).any(|value| !value.is_finite())
        })
    {
        return Err(TopologyCliError::Input(
            "multimodal design requires paired measured patient Gaussian modalities, train/test rows, and valid dimensions"
                .into(),
        ));
    }
    for names in [
        &design.modality_x.feature_names,
        &design.modality_y.feature_names,
    ] {
        let mut unique = HashSet::new();
        if names.iter().any(|name| {
            name.trim().is_empty() || name.trim() != name || !unique.insert(name.as_str())
        }) {
            return Err(TopologyCliError::Input(
                "multimodal feature names must be unique exact strings within each modality".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
