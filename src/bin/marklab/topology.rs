use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use clap::{Parser, Subcommand};
use marklab_topology::{
    connectivity_transition, sha256_hex, AlphaPersistenceResult, AlphaPersistenceSpec,
    AlphaPersistenceWorkerRequest, ConnectivityTransitionSpec, TopologyBackendContract,
    WitnessBottleneckDimensionResult, WitnessBottleneckPerturbationResult,
    WitnessDimensionStabilityResult, WitnessPersistenceBottleneckStabilityResult,
    WitnessPersistenceBottleneckStabilitySpec, WitnessPersistenceResult, WitnessPersistenceSpec,
    WitnessPersistenceStabilityResult, WitnessPersistenceStabilitySpec,
    WitnessPersistenceWorkerRequest, WitnessPerturbationStabilityResult,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAXIMUM_WORKER_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct TopologyCli {
    #[command(subcommand)]
    command: TopologyTopLevel,
}

#[derive(Debug, Subcommand)]
enum TopologyTopLevel {
    Topology {
        #[command(subcommand)]
        command: TopologyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum TopologyCommand {
    AlphaPersistence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceStability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceBottleneckStability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    RasterMorphology {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Connectivity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ComparePersistence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Stability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Validate {
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum TopologyCliError {
    #[error("invalid topology input: {0}")]
    Input(String),
    #[error("topology backend failed: {0}")]
    Backend(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid topology JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match TopologyCli::parse().command {
        TopologyTopLevel::Topology {
            command: TopologyCommand::AlphaPersistence { input, out },
        } => run_alpha_persistence(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::WitnessPersistence { input, out },
        } => run_witness_persistence(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::WitnessPersistenceStability { input, out },
        } => run_witness_persistence_stability(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::WitnessPersistenceBottleneckStability { input, out },
        } => run_witness_persistence_bottleneck_stability(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::RasterMorphology { input, out },
        } => run_raster_morphology(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::Connectivity { input, out },
        } => run_connectivity(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::ComparePersistence { input, out },
        } => run_compare_persistence(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::Stability { input, out },
        } => run_stability(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::Validate { out },
        } => run_validation(out),
    }
}

fn run_validation(out: PathBuf) -> Result<(), TopologyCliError> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_topology_validation_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.topology_validation_request",
        "version": 1,
        "backend": {
            "name": "gudhi_scikit_image_scipy",
            "version": "gudhi-3.13.0+scikit-image-0.26.0+scipy-1.18.1",
            "python_version": "3.12",
            "license": "MIT_GPLv3_CGAL_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        }
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, 30)?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.topology_validation"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["exact_fixture_status"] != "passed"
        || result["claim_status"] != "synthetic_exact_topology_validation_only"
    {
        return Err(TopologyCliError::Backend(
            "topology validation result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct TogglePerturbationSpec {
    kind: String,
    candidate_pixels: Vec<[usize; 2]>,
    toggles_per_repetition: usize,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TopologyStabilitySpec {
    base_mask: Vec<Vec<bool>>,
    pixel_size_um: f64,
    perturbation_generator: TogglePerturbationSpec,
    scales_um: Vec<f64>,
    repetitions: usize,
    seed: u64,
    timeout_seconds: u64,
}

fn run_stability(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: TopologyStabilitySpec = serde_json::from_slice(&bytes)?;
    if spec.base_mask.len() < 3
        || spec.base_mask.len() > 2_000
        || spec.base_mask[0].len() < 3
        || spec.base_mask[0].len() > 2_000
        || spec
            .base_mask
            .iter()
            .any(|row| row.len() != spec.base_mask[0].len())
        || !spec.pixel_size_um.is_finite()
        || spec.pixel_size_um <= 0.0
        || spec.perturbation_generator.kind != "toggle_declared_pixels"
        || spec.perturbation_generator.candidate_pixels.is_empty()
        || spec.perturbation_generator.candidate_pixels.len() > 100_000
        || spec.perturbation_generator.toggles_per_repetition == 0
        || spec.perturbation_generator.toggles_per_repetition
            > spec.perturbation_generator.candidate_pixels.len()
        || spec
            .perturbation_generator
            .candidate_pixels
            .iter()
            .any(|pixel| pixel[0] >= spec.base_mask.len() || pixel[1] >= spec.base_mask[0].len())
        || spec.scales_um.len() < 2
        || spec.scales_um.len() > 256
        || spec.scales_um[0] != 0.0
        || spec.scales_um.iter().any(|scale| {
            !scale.is_finite()
                || *scale < 0.0
                || (scale / spec.pixel_size_um - (scale / spec.pixel_size_um).round()).abs() > 1e-12
        })
        || spec.scales_um.windows(2).any(|pair| pair[0] >= pair[1])
        || !(1..=10_000).contains(&spec.repetitions)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "topology stability mask, generator, scales, repetitions, or timeout are invalid"
                .into(),
        ));
    }
    let unique_candidates = spec
        .perturbation_generator
        .candidate_pixels
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_candidates.len() != spec.perturbation_generator.candidate_pixels.len() {
        return Err(TopologyCliError::Input(
            "topology stability candidate pixels must be unique".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_topology_stability_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.topology_stability_request",
        "version": 1,
        "backend": {
            "name": "gudhi_scikit_image_scipy",
            "version": "gudhi-3.13.0+scikit-image-0.26.0+scipy-1.18.1",
            "python_version": "3.12",
            "license": "MIT_GPLv3_CGAL_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "base_mask": spec.base_mask,
        "pixel_size_um": spec.pixel_size_um,
        "perturbation_generator": spec.perturbation_generator,
        "scales_um": spec.scales_um,
        "repetitions": spec.repetitions,
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
    if result["format"] != "marklab.topology_stability"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["results"].as_array().map(Vec::len) != Some(spec.repetitions)
        || result["claim_status"] != "experimental_declared_segmentation_perturbations"
    {
        return Err(TopologyCliError::Backend(
            "topology stability result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct DiagramPairInput {
    birth: f64,
    death: f64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PatientDiagramInput {
    patient_id: String,
    group: String,
    stratum: String,
    finite_pairs: Vec<DiagramPairInput>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistenceComparisonSpec {
    diagrams: Vec<PatientDiagramInput>,
    metric: String,
    coefficient: f64,
    maximum_exact_assignments: usize,
    timeout_seconds: u64,
}

fn run_compare_persistence(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: PersistenceComparisonSpec = serde_json::from_slice(&bytes)?;
    spec.diagrams
        .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
    if spec.diagrams.len() < 4
        || spec.diagrams.len() > 1_000
        || spec.metric != "bottleneck_linf"
        || !spec.coefficient.is_finite()
        || spec.coefficient < 0.0
        || spec.maximum_exact_assignments == 0
        || spec.maximum_exact_assignments > 1_000_000
        || !(1..=3_600).contains(&spec.timeout_seconds)
        || spec.diagrams.iter().enumerate().any(|(index, diagram)| {
            diagram.patient_id.trim().is_empty()
                || diagram.group.trim().is_empty()
                || diagram.stratum.trim().is_empty()
                || (index > 0 && spec.diagrams[index - 1].patient_id == diagram.patient_id)
                || diagram.finite_pairs.iter().any(|pair| {
                    !pair.birth.is_finite() || !pair.death.is_finite() || pair.death < pair.birth
                })
        })
    {
        return Err(TopologyCliError::Input(
            "persistence comparison diagrams, metric, coefficient, assignments, or timeout are invalid"
                .into(),
        ));
    }
    let groups = spec
        .diagrams
        .iter()
        .map(|diagram| diagram.group.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if groups != std::collections::BTreeSet::from(["A", "B"]) {
        return Err(TopologyCliError::Input(
            "persistence comparison requires exact nonempty groups A and B".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_gudhi_persistence_comparison_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.gudhi_persistence_comparison_request",
        "version": 1,
        "backend": {
            "name": "gudhi", "version": "3.13.0", "python_version": "3.12",
            "license": "MIT", "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "diagrams": spec.diagrams, "metric": spec.metric, "coefficient": spec.coefficient,
        "maximum_exact_assignments": spec.maximum_exact_assignments
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.persistence_distribution_comparison"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_whole_patient_topology_comparison"
    {
        return Err(TopologyCliError::Backend(
            "persistence comparison result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn run_connectivity(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: ConnectivityTransitionSpec = serde_json::from_slice(&bytes)?;
    let result = connectivity_transition(spec)
        .map_err(|error| TopologyCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RasterMorphologySpec {
    mask: Vec<Vec<bool>>,
    pixel_size_um: f64,
    connectivity: u8,
    crofton_directions: u8,
    radii_um: Vec<f64>,
    timeout_seconds: u64,
}

fn run_raster_morphology(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: RasterMorphologySpec = serde_json::from_slice(&bytes)?;
    if spec.mask.len() < 3
        || spec.mask.len() > 2_000
        || spec.mask.iter().any(|row| row.len() != spec.mask[0].len())
        || spec.mask[0].len() < 3
        || spec.mask[0].len() > 2_000
        || !spec.pixel_size_um.is_finite()
        || spec.pixel_size_um <= 0.0
        || !matches!(spec.connectivity, 4 | 8)
        || !matches!(spec.crofton_directions, 2 | 4)
        || spec.radii_um.is_empty()
        || spec.radii_um.len() > 256
        || spec.radii_um[0] != 0.0
        || spec.radii_um.iter().any(|radius| {
            !radius.is_finite()
                || *radius < 0.0
                || (radius / spec.pixel_size_um - (radius / spec.pixel_size_um).round()).abs()
                    > 1e-12
        })
        || spec.radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "raster morphology dimensions, conventions, radii, or timeout are invalid".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_skimage_raster_morphology_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.skimage_raster_morphology_request",
        "version": 1,
        "backend": {
            "name": "scikit-image",
            "version": "0.26.0",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker),
        },
        "mask": spec.mask,
        "pixel_size_um": spec.pixel_size_um,
        "connectivity": spec.connectivity,
        "crofton_directions": spec.crofton_directions,
        "radii_um": spec.radii_um,
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.raster_morphology"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_supplied_binary_raster"
    {
        return Err(TopologyCliError::Backend(
            "raster morphology result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

pub(crate) struct PreparedWitnessPersistence {
    pub(crate) request: WitnessPersistenceWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    timeout_seconds: u64,
}

pub(crate) struct PreparedWitnessPersistenceStability {
    pub(crate) spec: WitnessPersistenceStabilitySpec,
    pub(crate) runs: Vec<PreparedWitnessPersistence>,
}

pub(crate) struct PreparedWitnessPersistenceBottleneckStability {
    pub(crate) spec: WitnessPersistenceBottleneckStabilitySpec,
    pub(crate) stability: PreparedWitnessPersistenceStability,
    pub(crate) backend: TopologyBackendContract,
    worker_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WitnessBottleneckWorkerComparison {
    replicate: usize,
    dimension: usize,
    status: String,
    bottleneck_distance_um_squared: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WitnessBottleneckWorkerResult {
    format: String,
    version: u32,
    backend: TopologyBackendContract,
    request_sha256: String,
    metric: String,
    essential_interval_policy: String,
    comparisons: Vec<WitnessBottleneckWorkerComparison>,
    comparison_count: u64,
    interval_count: u64,
    maximum_finite_bottleneck_distance_um_squared: f64,
    has_infinite_essential_mismatch: bool,
    claim_status: String,
}

pub(crate) fn prepare_witness_persistence_stability(
    input: &Path,
) -> Result<PreparedWitnessPersistenceStability, TopologyCliError> {
    let bytes = read_input(input)?;
    let spec: WitnessPersistenceStabilitySpec = serde_json::from_slice(&bytes)?;
    prepare_witness_persistence_stability_spec(spec)
}

fn prepare_witness_persistence_stability_spec(
    spec: WitnessPersistenceStabilitySpec,
) -> Result<PreparedWitnessPersistenceStability, TopologyCliError> {
    validate_witness_stability_controls(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_persistence_worker.py");
    let worker = read_required(&worker_path)?;
    let lock_digest = sha256_hex(&lock);
    let worker_digest = sha256_hex(&worker);
    let mut runs = Vec::with_capacity(spec.perturbation_replicates + 1);
    for run in 0..=spec.perturbation_replicates {
        let points = if run == 0 {
            spec.points.clone()
        } else {
            perturb_witness_points(&spec, run - 1)?.0
        };
        let request = WitnessPersistenceWorkerRequest::new(
            witness_spec(&spec, points),
            lock_digest.clone(),
            worker_digest.clone(),
        )
        .map_err(|error| TopologyCliError::Input(error.to_string()))?;
        let request_bytes = serde_json::to_vec(&request)?;
        runs.push(PreparedWitnessPersistence {
            request,
            request_bytes,
            timeout_seconds: spec.timeout_seconds,
        });
    }
    Ok(PreparedWitnessPersistenceStability { spec, runs })
}

pub(crate) fn prepare_witness_persistence_bottleneck_stability(
    input: &Path,
) -> Result<PreparedWitnessPersistenceBottleneckStability, TopologyCliError> {
    let bytes = read_input(input)?;
    let spec: WitnessPersistenceBottleneckStabilitySpec = serde_json::from_slice(&bytes)?;
    validate_witness_bottleneck_controls(&spec)?;
    let stability = prepare_witness_persistence_stability_spec(spec.stability.clone())?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_bottleneck_worker.py");
    let worker = read_required(&worker_path)?;
    let backend = TopologyBackendContract {
        name: "gudhi".into(),
        version: marklab_topology::GUDHI_VERSION.into(),
        python_version: "3.12".into(),
        license: "MIT_with_GPLv3_CGAL_alpha_complex_dependency".into(),
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    Ok(PreparedWitnessPersistenceBottleneckStability {
        spec,
        stability,
        backend,
        worker_path,
    })
}

pub(crate) fn execute_witness_persistence_stability(
    prepared: &PreparedWitnessPersistenceStability,
) -> Result<WitnessPersistenceStabilityResult, TopologyCliError> {
    let results = execute_witness_persistence_stability_runs(prepared)?;
    summarize_witness_persistence_stability(prepared, &results)
}

fn execute_witness_persistence_stability_runs(
    prepared: &PreparedWitnessPersistenceStability,
) -> Result<Vec<WitnessPersistenceResult>, TopologyCliError> {
    let mut results = Vec::with_capacity(prepared.runs.len());
    for (index, run) in prepared.runs.iter().enumerate() {
        results.push(execute_witness_persistence(run).map_err(|error| {
            TopologyCliError::Backend(format!(
                "witness stability backend execution {index} failed: {error}"
            ))
        })?);
    }
    Ok(results)
}

fn summarize_witness_persistence_stability(
    prepared: &PreparedWitnessPersistenceStability,
    results: &[WitnessPersistenceResult],
) -> Result<WitnessPersistenceStabilityResult, TopologyCliError> {
    if results.len() != prepared.runs.len() {
        return Err(TopologyCliError::Backend(
            "witness stability backend result count differs".into(),
        ));
    }
    let baseline = &results[0];
    let mut perturbations = Vec::with_capacity(results.len());
    for (replicate, result) in results.iter().skip(1).enumerate() {
        let (_, maximum_coordinate_displacement_um) =
            perturb_witness_points(&prepared.spec, replicate)?;
        perturbations.push(summarize_witness_perturbation(
            replicate,
            maximum_coordinate_displacement_um,
            baseline,
            result,
        )?);
    }
    let minimum_landmark_id_match_fraction = perturbations
        .iter()
        .map(|row| row.landmark_id_match_fraction)
        .fold(1.0_f64, f64::min);
    let maximum_coverage_radius_change_um = perturbations
        .iter()
        .map(|row| row.coverage_radius_change_um)
        .fold(0.0_f64, f64::max);
    let maximum_simplex_count_l1_change = perturbations
        .iter()
        .map(|row| row.simplex_count_l1_change)
        .max()
        .unwrap_or(0);
    let maximum_total_persistence_change_um_squared = perturbations
        .iter()
        .map(|row| row.maximum_total_persistence_change_um_squared)
        .fold(0.0_f64, f64::max);
    let backend_executions = prepared.runs.len() as u64;
    let total_point_work = backend_executions * prepared.spec.points.len() as u64;
    let total_simplex_budget = backend_executions * prepared.spec.maximum_simplices as u64;
    let total_timeout_seconds = backend_executions * prepared.spec.timeout_seconds;
    let result = WitnessPersistenceStabilityResult {
        format: "marklab.witness_persistence_stability".into(),
        version: 1,
        statistical_unit: "one_specimen_point_pattern".into(),
        perturbation_rule: "sha256_uniform_independent_axis_jitter".into(),
        finite_result_policy: "reject_non_finite_input_or_output".into(),
        seed: prepared.spec.seed,
        perturbation_replicates: prepared.spec.perturbation_replicates,
        maximum_coordinate_jitter_um: prepared.spec.maximum_coordinate_jitter_um,
        minimum_landmark_id_match_fraction_allowed: prepared
            .spec
            .minimum_landmark_id_match_fraction,
        maximum_coverage_radius_change_um_allowed: prepared.spec.maximum_coverage_radius_change_um,
        maximum_simplex_count_l1_change_allowed: prepared.spec.maximum_simplex_count_l1_change,
        maximum_total_persistence_change_um_squared_allowed: prepared
            .spec
            .maximum_total_persistence_change_um_squared,
        baseline: baseline.clone(),
        perturbations,
        minimum_landmark_id_match_fraction,
        maximum_coverage_radius_change_um,
        maximum_simplex_count_l1_change,
        maximum_total_persistence_change_um_squared,
        backend_executions,
        total_point_work,
        total_simplex_budget,
        total_timeout_seconds,
        stable_under_declared_thresholds: minimum_landmark_id_match_fraction
            >= prepared.spec.minimum_landmark_id_match_fraction
            && maximum_coverage_radius_change_um <= prepared.spec.maximum_coverage_radius_change_um
            && maximum_simplex_count_l1_change <= prepared.spec.maximum_simplex_count_l1_change
            && maximum_total_persistence_change_um_squared
                <= prepared.spec.maximum_total_persistence_change_um_squared,
        claim_status: "witness_coordinate_perturbation_stability_diagnostic".into(),
    };
    validate_witness_persistence_stability_result(prepared, &result)?;
    Ok(result)
}

pub(crate) fn execute_witness_persistence_bottleneck_stability(
    prepared: &PreparedWitnessPersistenceBottleneckStability,
) -> Result<WitnessPersistenceBottleneckStabilityResult, TopologyCliError> {
    let witness_results = execute_witness_persistence_stability_runs(&prepared.stability)?;
    let stability = summarize_witness_persistence_stability(&prepared.stability, &witness_results)?;
    let baseline = &witness_results[0];
    let mut comparisons = Vec::new();
    for (replicate, perturbed) in witness_results.iter().skip(1).enumerate() {
        for dimension in 0..=prepared.spec.stability.maximum_dimension {
            let baseline_dimension = persistence_dimension(baseline, dimension)?;
            let perturbed_dimension = persistence_dimension(perturbed, dimension)?;
            comparisons.push(serde_json::json!({
                "replicate": replicate,
                "dimension": dimension,
                "baseline": {
                    "finite_pairs": baseline_dimension.finite_pairs,
                    "essential_births": baseline_dimension.essential_births,
                },
                "perturbed": {
                    "finite_pairs": perturbed_dimension.finite_pairs,
                    "essential_births": perturbed_dimension.essential_births,
                },
            }));
        }
    }
    let request = serde_json::json!({
        "format": "marklab.gudhi_witness_bottleneck_request",
        "version": 1,
        "backend": prepared.backend,
        "metric": "bottleneck_linf",
        "coefficient": 0.0,
        "comparisons": comparisons,
        "maximum_comparisons": prepared.spec.maximum_bottleneck_comparisons,
        "maximum_interval_budget": prepared.spec.maximum_bottleneck_interval_budget,
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let response = run_worker(
        &repository,
        &prepared.worker_path,
        &request_bytes,
        prepared.spec.bottleneck_timeout_seconds,
    )?;
    let worker: WitnessBottleneckWorkerResult = serde_json::from_slice(&response)?;
    validate_witness_bottleneck_worker_result(prepared, &worker, &request_bytes)?;

    let mut perturbations = Vec::with_capacity(prepared.spec.stability.perturbation_replicates);
    for replicate in 0..prepared.spec.stability.perturbation_replicates {
        let by_dimension = worker
            .comparisons
            .iter()
            .filter(|row| row.replicate == replicate)
            .map(|row| WitnessBottleneckDimensionResult {
                dimension: row.dimension,
                status: row.status.clone(),
                bottleneck_distance_um_squared: row.bottleneck_distance_um_squared,
            })
            .collect::<Vec<_>>();
        let maximum_finite = by_dimension
            .iter()
            .filter_map(|row| row.bottleneck_distance_um_squared)
            .fold(0.0_f64, f64::max);
        let has_infinite = by_dimension
            .iter()
            .any(|row| row.status == "infinite_essential_count_mismatch");
        perturbations.push(WitnessBottleneckPerturbationResult {
            replicate,
            by_dimension,
            maximum_finite_bottleneck_distance_um_squared: maximum_finite,
            has_infinite_essential_mismatch: has_infinite,
        });
    }
    let maximum_finite_bottleneck_distance_um_squared = perturbations
        .iter()
        .map(|row| row.maximum_finite_bottleneck_distance_um_squared)
        .fold(0.0_f64, f64::max);
    let has_infinite_essential_mismatch = perturbations
        .iter()
        .any(|row| row.has_infinite_essential_mismatch);
    let stable_under_bottleneck_threshold = !has_infinite_essential_mismatch
        && maximum_finite_bottleneck_distance_um_squared
            <= prepared.spec.maximum_bottleneck_distance_um_squared;
    let total_backend_executions = stability
        .backend_executions
        .checked_add(1)
        .ok_or_else(|| TopologyCliError::Backend("total backend execution overflow".into()))?;
    let stable_under_all_declared_thresholds =
        stable_under_bottleneck_threshold && stability.stable_under_declared_thresholds;
    let result = WitnessPersistenceBottleneckStabilityResult {
        format: "marklab.witness_persistence_bottleneck_stability".into(),
        version: 1,
        backend: worker.backend,
        bottleneck_request_sha256: worker.request_sha256,
        bottleneck_metric: worker.metric,
        essential_interval_policy: worker.essential_interval_policy,
        stability,
        perturbations,
        maximum_bottleneck_distance_um_squared_allowed: prepared
            .spec
            .maximum_bottleneck_distance_um_squared,
        maximum_finite_bottleneck_distance_um_squared,
        has_infinite_essential_mismatch,
        bottleneck_comparisons: worker.comparison_count,
        bottleneck_interval_count: worker.interval_count,
        bottleneck_backend_executions: 1,
        total_backend_executions,
        stable_under_bottleneck_threshold,
        stable_under_all_declared_thresholds,
        claim_status: "witness_coordinate_bottleneck_stability_diagnostic".into(),
    };
    validate_witness_persistence_bottleneck_stability_result(prepared, &result)?;
    Ok(result)
}

fn persistence_dimension(
    result: &WitnessPersistenceResult,
    dimension: usize,
) -> Result<&marklab_topology::PersistenceDimensionResult, TopologyCliError> {
    result
        .persistence
        .by_dimension
        .iter()
        .find(|row| row.dimension == dimension)
        .ok_or_else(|| TopologyCliError::Backend("witness persistence dimension is missing".into()))
}

fn validate_witness_bottleneck_worker_result(
    prepared: &PreparedWitnessPersistenceBottleneckStability,
    result: &WitnessBottleneckWorkerResult,
    request_bytes: &[u8],
) -> Result<(), TopologyCliError> {
    let expected_comparisons = (prepared.spec.stability.perturbation_replicates as u64)
        * (prepared.spec.stability.maximum_dimension as u64 + 1);
    let maximum_finite = result
        .comparisons
        .iter()
        .filter_map(|row| row.bottleneck_distance_um_squared)
        .fold(0.0_f64, f64::max);
    let has_infinite = result
        .comparisons
        .iter()
        .any(|row| row.status == "infinite_essential_count_mismatch");
    let mut identities = std::collections::BTreeSet::new();
    if result.format != "marklab.gudhi_witness_bottleneck_result"
        || result.version != 1
        || result.metric != "gudhi_exact_linf"
        || result.essential_interval_policy != "infinite_death_equal_counts_else_infinite_mismatch"
        || result.claim_status != "exact_witness_diagram_bottleneck_stability_diagnostic"
        || result.request_sha256 != sha256_hex(request_bytes)
        || !same_topology_backend(&result.backend, &prepared.backend)
        || result.comparison_count != expected_comparisons
        || result.comparisons.len() as u64 != expected_comparisons
        || result.comparison_count > prepared.spec.maximum_bottleneck_comparisons
        || result.interval_count > prepared.spec.maximum_bottleneck_interval_budget
        || maximum_finite.to_bits()
            != result
                .maximum_finite_bottleneck_distance_um_squared
                .to_bits()
        || has_infinite != result.has_infinite_essential_mismatch
        || result.comparisons.iter().any(|row| {
            !identities.insert((row.replicate, row.dimension))
                || row.replicate >= prepared.spec.stability.perturbation_replicates
                || row.dimension > prepared.spec.stability.maximum_dimension
                || match row.status.as_str() {
                    "finite" => row
                        .bottleneck_distance_um_squared
                        .is_none_or(|value| !value.is_finite() || value < 0.0),
                    "infinite_essential_count_mismatch" => {
                        row.bottleneck_distance_um_squared.is_some()
                    }
                    _ => true,
                }
        })
    {
        return Err(TopologyCliError::Backend(
            "witness bottleneck worker result identity or contract mismatch".into(),
        ));
    }
    Ok(())
}

fn same_topology_backend(left: &TopologyBackendContract, right: &TopologyBackendContract) -> bool {
    left.name == right.name
        && left.version == right.version
        && left.python_version == right.python_version
        && left.license == right.license
        && left.environment_lock_sha256 == right.environment_lock_sha256
        && left.worker_sha256 == right.worker_sha256
}

pub(crate) fn validate_witness_persistence_bottleneck_stability_result(
    prepared: &PreparedWitnessPersistenceBottleneckStability,
    result: &WitnessPersistenceBottleneckStabilityResult,
) -> Result<(), TopologyCliError> {
    validate_witness_persistence_stability_result(&prepared.stability, &result.stability)?;
    if result.perturbations.len() != prepared.spec.stability.perturbation_replicates {
        return Err(TopologyCliError::Backend(
            "witness bottleneck perturbation count differs".into(),
        ));
    }
    let expected_dimensions = prepared.spec.stability.maximum_dimension + 1;
    let expected_comparisons = (prepared.spec.stability.perturbation_replicates as u64)
        .checked_mul(expected_dimensions as u64)
        .ok_or_else(|| TopologyCliError::Backend("bottleneck comparison overflow".into()))?;
    let maximum_finite = result
        .perturbations
        .iter()
        .map(|row| row.maximum_finite_bottleneck_distance_um_squared)
        .fold(0.0_f64, f64::max);
    let has_infinite = result
        .perturbations
        .iter()
        .any(|row| row.has_infinite_essential_mismatch);
    let stable_bottleneck =
        !has_infinite && maximum_finite <= prepared.spec.maximum_bottleneck_distance_um_squared;
    let expected_total_executions = result
        .stability
        .backend_executions
        .checked_add(1)
        .ok_or_else(|| TopologyCliError::Backend("total backend execution overflow".into()))?;
    if result.format != "marklab.witness_persistence_bottleneck_stability"
        || result.version != 1
        || result.bottleneck_metric != "gudhi_exact_linf"
        || result.essential_interval_policy != "infinite_death_equal_counts_else_infinite_mismatch"
        || result.claim_status != "witness_coordinate_bottleneck_stability_diagnostic"
        || !same_topology_backend(&result.backend, &prepared.backend)
        || result.bottleneck_request_sha256.len() != 64
        || !result
            .bottleneck_request_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || result
            .maximum_bottleneck_distance_um_squared_allowed
            .to_bits()
            != prepared
                .spec
                .maximum_bottleneck_distance_um_squared
                .to_bits()
        || maximum_finite.to_bits()
            != result
                .maximum_finite_bottleneck_distance_um_squared
                .to_bits()
        || has_infinite != result.has_infinite_essential_mismatch
        || result.bottleneck_comparisons != expected_comparisons
        || result.bottleneck_comparisons > prepared.spec.maximum_bottleneck_comparisons
        || result.bottleneck_interval_count > prepared.spec.maximum_bottleneck_interval_budget
        || result.bottleneck_backend_executions != 1
        || result.bottleneck_backend_executions
            > prepared.spec.maximum_bottleneck_backend_executions
        || result.total_backend_executions != expected_total_executions
        || result.total_backend_executions > prepared.spec.maximum_total_backend_executions
        || result.stable_under_bottleneck_threshold != stable_bottleneck
        || result.stable_under_all_declared_thresholds
            != (stable_bottleneck && result.stability.stable_under_declared_thresholds)
        || result
            .perturbations
            .iter()
            .enumerate()
            .any(|(replicate, row)| {
                let row_maximum = row
                    .by_dimension
                    .iter()
                    .filter_map(|dimension| dimension.bottleneck_distance_um_squared)
                    .fold(0.0_f64, f64::max);
                let row_infinite = row
                    .by_dimension
                    .iter()
                    .any(|dimension| dimension.status == "infinite_essential_count_mismatch");
                let mut dimensions = std::collections::BTreeSet::new();
                row.replicate != replicate
                    || row.by_dimension.len() != expected_dimensions
                    || row_maximum.to_bits()
                        != row.maximum_finite_bottleneck_distance_um_squared.to_bits()
                    || row_infinite != row.has_infinite_essential_mismatch
                    || row.by_dimension.iter().any(|dimension| {
                        !dimensions.insert(dimension.dimension)
                            || dimension.dimension > prepared.spec.stability.maximum_dimension
                            || match dimension.status.as_str() {
                                "finite" => dimension
                                    .bottleneck_distance_um_squared
                                    .is_none_or(|value| !value.is_finite() || value < 0.0),
                                "infinite_essential_count_mismatch" => {
                                    dimension.bottleneck_distance_um_squared.is_some()
                                }
                                _ => true,
                            }
                    })
            })
    {
        return Err(TopologyCliError::Backend(
            "witness bottleneck stability result identity or contract mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_witness_bottleneck_controls(
    spec: &WitnessPersistenceBottleneckStabilitySpec,
) -> Result<(), TopologyCliError> {
    if !spec.maximum_bottleneck_distance_um_squared.is_finite()
        || spec.maximum_bottleneck_distance_um_squared < 0.0
        || spec.maximum_bottleneck_comparisons == 0
        || spec.maximum_bottleneck_interval_budget == 0
        || spec.maximum_bottleneck_backend_executions == 0
        || !(1..=3_600).contains(&spec.bottleneck_timeout_seconds)
        || spec.maximum_total_backend_executions == 0
    {
        return Err(TopologyCliError::Input(
            "witness bottleneck controls or thresholds are invalid".into(),
        ));
    }
    let comparisons = (spec.stability.perturbation_replicates as u64)
        .checked_mul(spec.stability.maximum_dimension as u64 + 1)
        .ok_or_else(|| TopologyCliError::Input("bottleneck comparison overflow".into()))?;
    if comparisons > spec.maximum_bottleneck_comparisons {
        return Err(TopologyCliError::Input(
            "bottleneck comparisons exceed caller maximum".into(),
        ));
    }
    let interval_budget = comparisons
        .checked_mul(2)
        .and_then(|value| value.checked_mul(spec.stability.maximum_simplices as u64))
        .ok_or_else(|| TopologyCliError::Input("bottleneck interval budget overflow".into()))?;
    if interval_budget > spec.maximum_bottleneck_interval_budget {
        return Err(TopologyCliError::Input(
            "bottleneck interval budget exceeds caller maximum".into(),
        ));
    }
    let total_executions = (spec.stability.perturbation_replicates as u64)
        .checked_add(2)
        .ok_or_else(|| TopologyCliError::Input("total backend execution overflow".into()))?;
    if 1 > spec.maximum_bottleneck_backend_executions
        || total_executions > spec.maximum_total_backend_executions
    {
        return Err(TopologyCliError::Input(
            "bottleneck or total backend executions exceed caller maximum".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_witness_persistence_stability_result(
    prepared: &PreparedWitnessPersistenceStability,
    result: &WitnessPersistenceStabilityResult,
) -> Result<(), TopologyCliError> {
    result
        .baseline
        .validate(&prepared.runs[0].request, &prepared.runs[0].request_bytes)
        .map_err(|error| TopologyCliError::Backend(error.to_string()))?;
    let minimum_match = result
        .perturbations
        .iter()
        .map(|row| row.landmark_id_match_fraction)
        .fold(1.0_f64, f64::min);
    let maximum_coverage = result
        .perturbations
        .iter()
        .map(|row| row.coverage_radius_change_um)
        .fold(0.0_f64, f64::max);
    let maximum_simplex = result
        .perturbations
        .iter()
        .map(|row| row.simplex_count_l1_change)
        .max()
        .unwrap_or(0);
    let maximum_persistence = result
        .perturbations
        .iter()
        .map(|row| row.maximum_total_persistence_change_um_squared)
        .fold(0.0_f64, f64::max);
    let executions = prepared.runs.len() as u64;
    let point_work = executions * prepared.spec.points.len() as u64;
    let simplex_budget = executions * prepared.spec.maximum_simplices as u64;
    let timeout = executions * prepared.spec.timeout_seconds;
    if result.format != "marklab.witness_persistence_stability"
        || result.version != 1
        || result.statistical_unit != "one_specimen_point_pattern"
        || result.perturbation_rule != "sha256_uniform_independent_axis_jitter"
        || result.finite_result_policy != "reject_non_finite_input_or_output"
        || result.claim_status != "witness_coordinate_perturbation_stability_diagnostic"
        || result.seed != prepared.spec.seed
        || result.perturbation_replicates != prepared.spec.perturbation_replicates
        || result.perturbations.len() != prepared.spec.perturbation_replicates
        || result.maximum_coordinate_jitter_um.to_bits()
            != prepared.spec.maximum_coordinate_jitter_um.to_bits()
        || result.minimum_landmark_id_match_fraction_allowed.to_bits()
            != prepared.spec.minimum_landmark_id_match_fraction.to_bits()
        || result.maximum_coverage_radius_change_um_allowed.to_bits()
            != prepared.spec.maximum_coverage_radius_change_um.to_bits()
        || result.maximum_simplex_count_l1_change_allowed
            != prepared.spec.maximum_simplex_count_l1_change
        || result
            .maximum_total_persistence_change_um_squared_allowed
            .to_bits()
            != prepared
                .spec
                .maximum_total_persistence_change_um_squared
                .to_bits()
        || result.backend_executions != executions
        || result.total_point_work != point_work
        || result.total_simplex_budget != simplex_budget
        || result.total_timeout_seconds != timeout
        || minimum_match.to_bits() != result.minimum_landmark_id_match_fraction.to_bits()
        || maximum_coverage.to_bits() != result.maximum_coverage_radius_change_um.to_bits()
        || maximum_simplex != result.maximum_simplex_count_l1_change
        || maximum_persistence.to_bits()
            != result.maximum_total_persistence_change_um_squared.to_bits()
        || result.stable_under_declared_thresholds
            != (minimum_match >= prepared.spec.minimum_landmark_id_match_fraction
                && maximum_coverage <= prepared.spec.maximum_coverage_radius_change_um
                && maximum_simplex <= prepared.spec.maximum_simplex_count_l1_change
                && maximum_persistence <= prepared.spec.maximum_total_persistence_change_um_squared)
        || result.perturbations.iter().enumerate().any(|(index, row)| {
            row.replicate != index
                || row.request_sha256 != sha256_hex(&prepared.runs[index + 1].request_bytes)
                || !row.maximum_coordinate_displacement_um.is_finite()
                || !row.landmark_id_match_fraction.is_finite()
                || !(0.0..=1.0).contains(&row.landmark_id_match_fraction)
                || !row.coverage_radius_change_um.is_finite()
                || !row.maximum_total_persistence_change_um_squared.is_finite()
                || row
                    .by_dimension
                    .iter()
                    .any(|dimension| !dimension.total_persistence_change_um_squared.is_finite())
        })
    {
        return Err(TopologyCliError::Backend(
            "witness stability result identity or contract mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_witness_stability_controls(
    spec: &WitnessPersistenceStabilitySpec,
) -> Result<(), TopologyCliError> {
    if !(1..=32).contains(&spec.perturbation_replicates)
        || !spec.maximum_coordinate_jitter_um.is_finite()
        || spec.maximum_coordinate_jitter_um < 0.0
        || !spec.minimum_landmark_id_match_fraction.is_finite()
        || !(0.0..=1.0).contains(&spec.minimum_landmark_id_match_fraction)
        || !spec.maximum_coverage_radius_change_um.is_finite()
        || spec.maximum_coverage_radius_change_um < 0.0
        || !spec.maximum_total_persistence_change_um_squared.is_finite()
        || spec.maximum_total_persistence_change_um_squared < 0.0
    {
        return Err(TopologyCliError::Input(
            "witness stability perturbation controls or thresholds are invalid".into(),
        ));
    }
    let executions = (spec.perturbation_replicates as u64)
        .checked_add(1)
        .ok_or_else(|| TopologyCliError::Input("backend execution count overflow".into()))?;
    if executions > spec.maximum_backend_executions {
        return Err(TopologyCliError::Input(
            "backend executions exceed caller maximum".into(),
        ));
    }
    let point_work = executions
        .checked_mul(spec.points.len() as u64)
        .ok_or_else(|| TopologyCliError::Input("aggregate point work overflow".into()))?;
    if point_work > spec.maximum_total_point_work {
        return Err(TopologyCliError::Input(
            "aggregate point work exceeds caller maximum".into(),
        ));
    }
    let simplex_budget = executions
        .checked_mul(spec.maximum_simplices as u64)
        .ok_or_else(|| TopologyCliError::Input("aggregate simplex budget overflow".into()))?;
    if simplex_budget > spec.maximum_total_simplex_budget {
        return Err(TopologyCliError::Input(
            "aggregate simplex budget exceeds caller maximum".into(),
        ));
    }
    let timeout = executions
        .checked_mul(spec.timeout_seconds)
        .ok_or_else(|| TopologyCliError::Input("aggregate timeout overflow".into()))?;
    if timeout > spec.maximum_total_timeout_seconds {
        return Err(TopologyCliError::Input(
            "aggregate timeout exceeds caller maximum".into(),
        ));
    }
    Ok(())
}

fn witness_spec(
    spec: &WitnessPersistenceStabilitySpec,
    points: Vec<marklab_topology::TopologyPointInput>,
) -> WitnessPersistenceSpec {
    WitnessPersistenceSpec {
        points,
        landmark_method: spec.landmark_method,
        landmark_count: spec.landmark_count,
        maximum_dimension: spec.maximum_dimension,
        nu: spec.nu,
        max_scale_um: spec.max_scale_um,
        coefficient_field: spec.coefficient_field,
        maximum_simplices: spec.maximum_simplices,
        timeout_seconds: spec.timeout_seconds,
    }
}

fn perturb_witness_points(
    spec: &WitnessPersistenceStabilitySpec,
    replicate: usize,
) -> Result<(Vec<marklab_topology::TopologyPointInput>, f64), TopologyCliError> {
    let mut points = spec.points.clone();
    let mut maximum_displacement = 0.0_f64;
    for point in &mut points {
        let mut squared_displacement = 0.0;
        for (axis, coordinate) in point.coordinates_um.iter_mut().enumerate() {
            let jitter = deterministic_witness_jitter(
                spec.seed,
                replicate,
                &point.id,
                axis,
                spec.maximum_coordinate_jitter_um,
            );
            *coordinate += jitter;
            squared_displacement += jitter * jitter;
        }
        if point.coordinates_um.iter().any(|value| !value.is_finite()) {
            return Err(TopologyCliError::Input(
                "witness perturbation produced a non-finite coordinate".into(),
            ));
        }
        maximum_displacement = maximum_displacement.max(squared_displacement.sqrt());
    }
    Ok((points, maximum_displacement))
}

fn deterministic_witness_jitter(
    seed: u64,
    replicate: usize,
    point_id: &str,
    axis: usize,
    maximum: f64,
) -> f64 {
    if maximum == 0.0 {
        return 0.0;
    }
    let mut framed = Vec::with_capacity(96 + point_id.len());
    framed.extend_from_slice(b"marklab-witness-persistence-stability-jitter-v1\0");
    framed.extend_from_slice(&seed.to_be_bytes());
    framed.extend_from_slice(&(replicate as u64).to_be_bytes());
    framed.extend_from_slice(&(point_id.len() as u64).to_be_bytes());
    framed.extend_from_slice(point_id.as_bytes());
    framed.extend_from_slice(&(axis as u64).to_be_bytes());
    let digest = sha256_hex(&framed);
    let bits = u64::from_str_radix(&digest[..16], 16).expect("SHA-256 prefix is hexadecimal");
    let unit = (bits >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64));
    (2.0 * unit - 1.0) * maximum
}

fn summarize_witness_perturbation(
    replicate: usize,
    maximum_coordinate_displacement_um: f64,
    baseline: &WitnessPersistenceResult,
    perturbed: &WitnessPersistenceResult,
) -> Result<WitnessPerturbationStabilityResult, TopologyCliError> {
    let baseline_landmarks = baseline
        .landmark_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let matching_landmarks = perturbed
        .landmark_ids
        .iter()
        .filter(|id| baseline_landmarks.contains(id.as_str()))
        .count();
    let landmark_id_match_fraction = matching_landmarks as f64 / baseline.landmark_ids.len() as f64;
    let coverage_radius_change_um = (perturbed.approximation.coverage_radius_um
        - baseline.approximation.coverage_radius_um)
        .abs();
    let simplex_dimensions = baseline
        .filtration
        .simplex_counts_by_dimension
        .len()
        .max(perturbed.filtration.simplex_counts_by_dimension.len());
    let simplex_count_l1_change = (0..simplex_dimensions).try_fold(0_u64, |total, dimension| {
        let left = baseline
            .filtration
            .simplex_counts_by_dimension
            .get(dimension)
            .copied()
            .unwrap_or(0) as u64;
        let right = perturbed
            .filtration
            .simplex_counts_by_dimension
            .get(dimension)
            .copied()
            .unwrap_or(0) as u64;
        total.checked_add(left.abs_diff(right))
    });
    let simplex_count_l1_change = simplex_count_l1_change
        .ok_or_else(|| TopologyCliError::Backend("witness simplex-count change overflow".into()))?;
    let baseline_dimensions = persistence_dimension_summaries(baseline)?;
    let perturbed_dimensions = persistence_dimension_summaries(perturbed)?;
    let dimensions = baseline_dimensions
        .keys()
        .chain(perturbed_dimensions.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut by_dimension = Vec::with_capacity(dimensions.len());
    for dimension in dimensions {
        let left = baseline_dimensions
            .get(&dimension)
            .copied()
            .unwrap_or((0, 0, 0.0));
        let right = perturbed_dimensions
            .get(&dimension)
            .copied()
            .unwrap_or((0, 0, 0.0));
        by_dimension.push(WitnessDimensionStabilityResult {
            dimension,
            finite_pair_count_change: left.0.abs_diff(right.0),
            essential_count_change: left.1.abs_diff(right.1),
            total_persistence_change_um_squared: (left.2 - right.2).abs(),
        });
    }
    let maximum_total_persistence_change_um_squared = by_dimension
        .iter()
        .map(|row| row.total_persistence_change_um_squared)
        .fold(0.0_f64, f64::max);
    if !landmark_id_match_fraction.is_finite()
        || !coverage_radius_change_um.is_finite()
        || !maximum_total_persistence_change_um_squared.is_finite()
    {
        return Err(TopologyCliError::Backend(
            "witness stability summary is non-finite".into(),
        ));
    }
    Ok(WitnessPerturbationStabilityResult {
        replicate,
        request_sha256: perturbed.request_sha256.clone(),
        maximum_coordinate_displacement_um,
        landmark_id_match_fraction,
        coverage_radius_change_um,
        simplex_count_l1_change,
        by_dimension,
        maximum_total_persistence_change_um_squared,
    })
}

fn persistence_dimension_summaries(
    result: &WitnessPersistenceResult,
) -> Result<std::collections::BTreeMap<usize, (u64, u64, f64)>, TopologyCliError> {
    let mut summaries = std::collections::BTreeMap::new();
    for dimension in &result.persistence.by_dimension {
        let total_persistence = dimension
            .finite_pairs
            .iter()
            .map(|pair| {
                if !pair.birth.is_finite() || !pair.death.is_finite() || pair.death < pair.birth {
                    Err(TopologyCliError::Backend(
                        "witness persistence pair is invalid".into(),
                    ))
                } else {
                    Ok(pair.death - pair.birth)
                }
            })
            .sum::<Result<f64, _>>()?;
        if !total_persistence.is_finite()
            || summaries
                .insert(
                    dimension.dimension,
                    (
                        dimension.finite_pairs.len() as u64,
                        dimension.essential_count as u64,
                        total_persistence,
                    ),
                )
                .is_some()
        {
            return Err(TopologyCliError::Backend(
                "witness persistence dimensions are invalid".into(),
            ));
        }
    }
    Ok(summaries)
}

pub(crate) fn prepare_witness_persistence(
    input: &Path,
) -> Result<PreparedWitnessPersistence, TopologyCliError> {
    let bytes = read_input(input)?;
    let spec: WitnessPersistenceSpec = serde_json::from_slice(&bytes)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_persistence_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let timeout_seconds = spec.timeout_seconds;
    let request =
        WitnessPersistenceWorkerRequest::new(spec, sha256_hex(&lock), sha256_hex(&worker))
            .map_err(|error| TopologyCliError::Input(error.to_string()))?;
    let request_bytes = serde_json::to_vec(&request)?;
    Ok(PreparedWitnessPersistence {
        request,
        request_bytes,
        timeout_seconds,
    })
}

pub(crate) fn execute_witness_persistence(
    prepared: &PreparedWitnessPersistence,
) -> Result<WitnessPersistenceResult, TopologyCliError> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_persistence_worker.py");
    if sha256_hex(&read_required(&lock_path)?) != prepared.request.backend.environment_lock_sha256
        || sha256_hex(&read_required(&worker_path)?) != prepared.request.backend.worker_sha256
    {
        return Err(TopologyCliError::Backend(
            "GUDHI environment or worker changed after request preparation".into(),
        ));
    }
    let response = run_worker(
        &repository,
        &worker_path,
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    WitnessPersistenceResult::parse_and_validate(
        &response,
        &prepared.request,
        &prepared.request_bytes,
    )
    .map_err(|error| TopologyCliError::Backend(error.to_string()))
}

fn run_witness_persistence(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let prepared = prepare_witness_persistence(&input)?;
    let result = execute_witness_persistence(&prepared)?;
    publish_json(&out, &result)
}

fn run_witness_persistence_stability(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let prepared = prepare_witness_persistence_stability(&input)?;
    let result = execute_witness_persistence_stability(&prepared)?;
    publish_json(&out, &result)
}

fn run_witness_persistence_bottleneck_stability(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let prepared = prepare_witness_persistence_bottleneck_stability(&input)?;
    let result = execute_witness_persistence_bottleneck_stability(&prepared)?;
    publish_json(&out, &result)
}

fn run_alpha_persistence(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: AlphaPersistenceSpec = serde_json::from_slice(&bytes)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_alpha_persistence_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let timeout_seconds = spec.timeout_seconds;
    let request = AlphaPersistenceWorkerRequest::new(spec, sha256_hex(&lock), sha256_hex(&worker))
        .map_err(|error| TopologyCliError::Input(error.to_string()))?;
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, timeout_seconds)?;
    let result = AlphaPersistenceResult::parse_and_validate(&response, &request, &request_bytes)
        .map_err(|error| TopologyCliError::Backend(error.to_string()))?;
    publish_json(&out, &result)
}

pub(crate) fn read_input(path: &Path) -> Result<Vec<u8>, TopologyCliError> {
    let metadata = fs::metadata(path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(TopologyCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn read_required(path: &Path) -> Result<Vec<u8>, TopologyCliError> {
    let bytes = fs::read(path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_INPUT_BYTES as usize {
        return Err(TopologyCliError::Backend(format!(
            "required backend artifact is empty or too large: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

pub(crate) fn run_worker(
    repository: &Path,
    worker: &Path,
    request: &[u8],
    timeout_seconds: u64,
) -> Result<Vec<u8>, TopologyCliError> {
    if std::env::var_os("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION").is_some() {
        return Err(TopologyCliError::Backend(
            "external backend execution is disabled by MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"
                .into(),
        ));
    }
    let interpreter = repository.join("target/pymc-venv/bin/python");
    if !interpreter.is_file() {
        return Err(TopologyCliError::Backend(format!(
            "pinned Python environment is missing at {}",
            interpreter.display()
        )));
    }
    let mut child = Command::new(&interpreter)
        .arg("-I")
        .arg(worker)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("OMP_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .env("JAX_PLATFORMS", "cpu")
        .env("XLA_FLAGS", "--xla_cpu_multi_thread_eigen=false")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| TopologyCliError::Io {
            path: interpreter,
            source,
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| TopologyCliError::Backend("worker stdout was not piped".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| TopologyCliError::Backend("worker stderr was not piped".into()))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| TopologyCliError::Backend("worker stdin was not piped".into()))?;
    stdin
        .write_all(request)
        .map_err(|source| TopologyCliError::Io {
            path: worker.to_owned(),
            source,
        })?;
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|source| TopologyCliError::Io {
            path: worker.to_owned(),
            source,
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TopologyCliError::Backend(format!(
                "GUDHI worker exceeded the {timeout_seconds}-second limit"
            )));
        }
        thread::sleep(Duration::from_millis(25));
    };
    let (stdout, stdout_exceeded) = join_reader(stdout_reader, "stdout")?;
    let (stderr, stderr_exceeded) = join_reader(stderr_reader, "stderr")?;
    validate_process_result(status, stdout, stdout_exceeded, stderr, stderr_exceeded)
}

fn read_bounded(mut reader: impl Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut exceeded = false;
    let mut chunk = [0_u8; 8_192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let available = MAXIMUM_WORKER_OUTPUT_BYTES.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..count.min(available)]);
        exceeded |= count > available;
    }
    Ok((retained, exceeded))
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stream: &str,
) -> Result<(Vec<u8>, bool), TopologyCliError> {
    reader
        .join()
        .map_err(|_| TopologyCliError::Backend(format!("worker {stream} reader panicked")))?
        .map_err(|source| TopologyCliError::Io {
            path: PathBuf::from(format!("topology worker {stream}")),
            source,
        })
}

fn validate_process_result(
    status: ExitStatus,
    stdout: Vec<u8>,
    stdout_exceeded: bool,
    stderr: Vec<u8>,
    stderr_exceeded: bool,
) -> Result<Vec<u8>, TopologyCliError> {
    if stdout_exceeded || stderr_exceeded {
        return Err(TopologyCliError::Backend(
            "GUDHI worker exceeded the 16 MiB output limit".into(),
        ));
    }
    if !status.success() {
        return Err(TopologyCliError::Backend(format!(
            "GUDHI worker exited with {status}: {}",
            String::from_utf8_lossy(&stderr).trim()
        )));
    }
    if stdout.is_empty() {
        return Err(TopologyCliError::Backend(
            "GUDHI worker succeeded without a JSON result".into(),
        ));
    }
    Ok(stdout)
}

pub(crate) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), TopologyCliError> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(TopologyCliError::Input(format!(
            "output already exists: {}",
            path.display()
        )));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| TopologyCliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| TopologyCliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-topology-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|source| TopologyCliError::Io {
            path: staging.clone(),
            source,
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|source| TopologyCliError::Io {
            path: staging.clone(),
            source,
        })?;
    fs::rename(&staging, path).map_err(|source| TopologyCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
