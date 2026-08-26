use std::{collections::HashSet, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_topology::sha256_hex;
use serde::{Deserialize, Serialize};

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct RegistrationCli {
    #[command(subcommand)]
    command: RegistrationTopLevel,
}

#[derive(Debug, Subcommand)]
enum RegistrationTopLevel {
    Registration {
        #[command(subcommand)]
        command: RegistrationCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RegistrationCommand {
    Nonrigid {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Svf {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    LddmmLandmarks {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ProbabilisticSvf {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    LandmarkUncertainty {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Atlas {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistrationImage {
    frame: String,
    spacing_um: [f64; 2],
    pixels: Vec<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NonrigidSpec {
    fixed: RegistrationImage,
    moving: RegistrationImage,
    fixed_mask: Vec<Vec<bool>>,
    moving_mask: Vec<Vec<bool>>,
    metric: String,
    mesh_size: [u32; 2],
    shrink_factors: Vec<u32>,
    smoothing_sigmas_um: Vec<f64>,
    maximum_iterations: usize,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SvfSpec {
    fixed: RegistrationImage,
    moving: RegistrationImage,
    metric: String,
    regularization_weight: f64,
    squaring_steps: usize,
    maximum_iterations: usize,
    jacobian_tolerance: f64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LddmmLandmarkSpec {
    source_frame: String,
    target_frame: String,
    source_landmarks: Vec<[f64; 2]>,
    target_landmarks: Vec<[f64; 2]>,
    kernel: String,
    kernel_scale_um: f64,
    data_weight: f64,
    time_steps: usize,
    maximum_iterations: usize,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbabilisticSvfSpec {
    fixed: RegistrationImage,
    moving: RegistrationImage,
    velocity_family: String,
    prior_standard_deviation_pixels: f64,
    likelihood_noise_standard_deviation: f64,
    variational_samples: usize,
    posterior_draws: usize,
    maximum_iterations: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistrationCell {
    cell_id: String,
    coordinates_um: [f64; 2],
    features: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeformationPrior {
    kernel: String,
    amplitude_um: f64,
    length_scale_um: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LandmarkUncertaintySpec {
    source_frame: String,
    target_frame: String,
    source_landmarks: Vec<[f64; 2]>,
    target_landmarks: Vec<[f64; 2]>,
    source_cells: Vec<RegistrationCell>,
    target_cells: Vec<RegistrationCell>,
    deformation_prior: DeformationPrior,
    landmark_noise_standard_deviation_um: f64,
    localization_standard_deviation_um: f64,
    posterior_draws: usize,
    spatial_cost_scale_um: f64,
    feature_cost_scale: f64,
    dustbin_cost: f64,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasRepresentation {
    modality: String,
    model_version: String,
    stain: String,
    feature_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasAlignment {
    method: String,
    distance: String,
    ood_distance_threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasReferenceSample {
    sample_id: String,
    patient_id: String,
    site_id: String,
    domain: String,
    features: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasQuery {
    query_id: String,
    features: Vec<f64>,
    expected_domain: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasPerturbation {
    perturbation_id: String,
    feature_shift: Vec<f64>,
    subsample_fraction: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AtlasSpec {
    atlas_id: String,
    representation: AtlasRepresentation,
    alignment: AtlasAlignment,
    reference_samples: Vec<AtlasReferenceSample>,
    queries: Vec<AtlasQuery>,
    perturbations: Vec<AtlasPerturbation>,
    covariance_ridge: f64,
    seed: u64,
    timeout_seconds: u64,
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match RegistrationCli::parse().command {
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::Nonrigid { input, out },
        } => run_nonrigid(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::Svf { input, out },
        } => run_svf(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::LddmmLandmarks { input, out },
        } => run_lddmm_landmarks(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::ProbabilisticSvf { input, out },
        } => run_probabilistic_svf(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::LandmarkUncertainty { input, out },
        } => run_landmark_uncertainty(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::Atlas { input, out },
        } => run_atlas(input, out),
    }
}

fn run_atlas(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: AtlasSpec = serde_json::from_slice(&bytes)?;
    spec.reference_samples
        .sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    spec.perturbations
        .sort_by(|left, right| left.perturbation_id.cmp(&right.perturbation_id));
    validate_atlas(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_atlas_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_atlas_request",
        "version": 1,
        "backend": {
            "name": "scipy_biological_similarity_atlas",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "atlas_id": spec.atlas_id,
        "representation": spec.representation,
        "alignment": spec.alignment,
        "reference_samples": spec.reference_samples,
        "queries": spec.queries,
        "perturbations": spec.perturbations,
        "covariance_ridge": spec.covariance_ridge,
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
    if result["format"] != "marklab.spatial_atlas_mapping_and_validation"
        || result["atlas"]["format"] != "marklab.spatial_atlas"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_biological_similarity_atlas"
    {
        return Err(TopologyCliError::Backend(
            "atlas result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_atlas(spec: &AtlasSpec) -> Result<(), TopologyCliError> {
    let feature_count = spec.representation.feature_names.len();
    let mut feature_names = HashSet::new();
    let valid_feature_names = spec.representation.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && feature_names.insert(name.as_str())
    });
    let mut sample_ids = HashSet::new();
    let mut patients = HashSet::new();
    let mut sites = HashSet::new();
    let mut domains = HashSet::new();
    let valid_samples = spec.reference_samples.iter().all(|sample| {
        patients.insert(sample.patient_id.as_str());
        sites.insert(sample.site_id.as_str());
        domains.insert(sample.domain.as_str());
        !sample.sample_id.trim().is_empty()
            && sample.sample_id.trim() == sample.sample_id
            && sample_ids.insert(sample.sample_id.as_str())
            && !sample.patient_id.trim().is_empty()
            && sample.patient_id.trim() == sample.patient_id
            && !sample.site_id.trim().is_empty()
            && sample.site_id.trim() == sample.site_id
            && !sample.domain.trim().is_empty()
            && sample.domain.trim() == sample.domain
            && sample.features.len() == feature_count
            && sample.features.iter().all(|value| value.is_finite())
    });
    let replicated_domains = domains.iter().all(|domain| {
        let domain_patients = spec
            .reference_samples
            .iter()
            .filter(|sample| sample.domain == *domain)
            .map(|sample| sample.patient_id.as_str())
            .collect::<HashSet<_>>();
        domain_patients.len() >= 4
    });
    let mut query_ids = HashSet::new();
    let valid_queries = spec.queries.iter().all(|query| {
        !query.query_id.trim().is_empty()
            && query.query_id.trim() == query.query_id
            && query_ids.insert(query.query_id.as_str())
            && query.features.len() == feature_count
            && query.features.iter().all(|value| value.is_finite())
            && query
                .expected_domain
                .as_deref()
                .is_none_or(|domain| domains.contains(domain))
    });
    let mut perturbation_ids = HashSet::new();
    let valid_perturbations = spec.perturbations.iter().all(|perturbation| {
        !perturbation.perturbation_id.trim().is_empty()
            && perturbation.perturbation_id.trim() == perturbation.perturbation_id
            && perturbation_ids.insert(perturbation.perturbation_id.as_str())
            && perturbation.feature_shift.len() == feature_count
            && perturbation
                .feature_shift
                .iter()
                .all(|value| value.is_finite())
            && perturbation.subsample_fraction.is_finite()
            && (0.5..=1.0).contains(&perturbation.subsample_fraction)
    });
    if spec.atlas_id.trim().is_empty()
        || spec.atlas_id.trim() != spec.atlas_id
        || spec.representation.modality != "measured_region_features"
        || spec.representation.model_version.trim().is_empty()
        || spec.representation.stain.trim().is_empty()
        || !(1..=128).contains(&feature_count)
        || !valid_feature_names
        || spec.alignment.method != "biological_similarity_no_physical_registration"
        || spec.alignment.distance != "shrinkage_mahalanobis"
        || !spec.alignment.ood_distance_threshold.is_finite()
        || spec.alignment.ood_distance_threshold <= 0.0
        || !(8..=10_000).contains(&spec.reference_samples.len())
        || !valid_samples
        || patients.len() < 4
        || sites.is_empty()
        || !(2..=32).contains(&domains.len())
        || !replicated_domains
        || spec.queries.is_empty()
        || !valid_queries
        || spec.perturbations.is_empty()
        || !valid_perturbations
        || !spec.covariance_ridge.is_finite()
        || spec.covariance_ridge <= 0.0
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "atlas mapping requires replicated measured region domains/patients, frozen representation identity, shrinkage-Mahalanobis biological alignment, bounded queries, OOD threshold, and declared perturbations"
                .into(),
        ));
    }
    Ok(())
}

fn run_landmark_uncertainty(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: LandmarkUncertaintySpec = serde_json::from_slice(&bytes)?;
    spec.source_cells
        .sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    spec.target_cells
        .sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    validate_landmark_uncertainty(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_scipy_landmark_uncertainty_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_landmark_uncertainty_request",
        "version": 1,
        "backend": {
            "name": "scipy_gaussian_process_landmark_posterior",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "source_frame": spec.source_frame,
        "target_frame": spec.target_frame,
        "source_landmarks": spec.source_landmarks,
        "target_landmarks": spec.target_landmarks,
        "source_cells": spec.source_cells,
        "target_cells": spec.target_cells,
        "deformation_prior": spec.deformation_prior,
        "landmark_noise_standard_deviation_um": spec.landmark_noise_standard_deviation_um,
        "localization_standard_deviation_um": spec.localization_standard_deviation_um,
        "posterior_draws": spec.posterior_draws,
        "spatial_cost_scale_um": spec.spatial_cost_scale_um,
        "feature_cost_scale": spec.feature_cost_scale,
        "dustbin_cost": spec.dustbin_cost,
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
    if result["format"] != "marklab.landmark_uncertainty_and_correspondence"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "probabilistic_compatibility_not_cell_identity"
    {
        return Err(TopologyCliError::Backend(
            "landmark uncertainty result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_landmark_uncertainty(spec: &LandmarkUncertaintySpec) -> Result<(), TopologyCliError> {
    let unique_points = |points: &[[f64; 2]]| {
        let mut unique = HashSet::new();
        points.iter().all(|point| {
            point.iter().all(|value| value.is_finite())
                && unique.insert((point[0].to_bits(), point[1].to_bits()))
        })
    };
    let feature_count = spec
        .source_cells
        .first()
        .map(|cell| cell.features.len())
        .unwrap_or(0);
    let valid_cells = |cells: &[RegistrationCell]| {
        let mut ids = HashSet::new();
        cells.iter().all(|cell| {
            !cell.cell_id.trim().is_empty()
                && cell.cell_id.trim() == cell.cell_id
                && ids.insert(cell.cell_id.as_str())
                && cell.coordinates_um.iter().all(|value| value.is_finite())
                && cell.features.len() == feature_count
                && cell.features.iter().all(|value| value.is_finite())
        })
    };
    if spec.source_frame.trim().is_empty()
        || spec.source_frame.trim() != spec.source_frame
        || spec.target_frame.trim().is_empty()
        || spec.target_frame.trim() != spec.target_frame
        || spec.source_frame == spec.target_frame
        || !(4..=128).contains(&spec.source_landmarks.len())
        || spec.source_landmarks.len() != spec.target_landmarks.len()
        || !unique_points(&spec.source_landmarks)
        || !unique_points(&spec.target_landmarks)
        || !(2..=256).contains(&spec.source_cells.len())
        || !(2..=256).contains(&spec.target_cells.len())
        || !(1..=32).contains(&feature_count)
        || !valid_cells(&spec.source_cells)
        || !valid_cells(&spec.target_cells)
        || spec.deformation_prior.kernel != "squared_exponential"
        || !spec.deformation_prior.amplitude_um.is_finite()
        || spec.deformation_prior.amplitude_um <= 0.0
        || !spec.deformation_prior.length_scale_um.is_finite()
        || spec.deformation_prior.length_scale_um <= 0.0
        || !spec.landmark_noise_standard_deviation_um.is_finite()
        || spec.landmark_noise_standard_deviation_um <= 0.0
        || !spec.localization_standard_deviation_um.is_finite()
        || spec.localization_standard_deviation_um < 0.0
        || !(100..=2_000).contains(&spec.posterior_draws)
        || !spec.spatial_cost_scale_um.is_finite()
        || spec.spatial_cost_scale_um <= 0.0
        || !spec.feature_cost_scale.is_finite()
        || spec.feature_cost_scale <= 0.0
        || !spec.dustbin_cost.is_finite()
        || spec.dustbin_cost <= 0.0
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "landmark uncertainty requires unique paired landmarks, exact-frame finite cell features, a positive GP/noise model, bounded draws, and positive spatial/feature/dustbin costs"
                .into(),
        ));
    }
    Ok(())
}

fn run_probabilistic_svf(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: ProbabilisticSvfSpec = serde_json::from_slice(&bytes)?;
    validate_probabilistic_svf(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_probabilistic_svf_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_probabilistic_svf_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_variational_svf",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "fixed": spec.fixed,
        "moving": spec.moving,
        "velocity_family": spec.velocity_family,
        "prior_standard_deviation_pixels": spec.prior_standard_deviation_pixels,
        "likelihood_noise_standard_deviation": spec.likelihood_noise_standard_deviation,
        "variational_samples": spec.variational_samples,
        "posterior_draws": spec.posterior_draws,
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
    if result["format"] != "marklab.probabilistic_diffeomorphic_registration"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_probabilistic_diffeomorphism"
    {
        return Err(TopologyCliError::Backend(
            "probabilistic SVF result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_probabilistic_svf(spec: &ProbabilisticSvfSpec) -> Result<(), TopologyCliError> {
    let height = spec.fixed.pixels.len();
    let width = spec.fixed.pixels.first().map(Vec::len).unwrap_or(0);
    let valid_image = |image: &RegistrationImage| {
        !image.frame.trim().is_empty()
            && image.frame.trim() == image.frame
            && image.spacing_um == spec.fixed.spacing_um
            && image
                .spacing_um
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
            && image.pixels.len() == height
            && image
                .pixels
                .iter()
                .all(|row| row.len() == width && row.iter().all(|value| value.is_finite()))
    };
    if !(16..=64).contains(&height)
        || !(16..=64).contains(&width)
        || !valid_image(&spec.fixed)
        || !valid_image(&spec.moving)
        || spec.fixed.frame == spec.moving.frame
        || spec.velocity_family != "constant_translation_svf"
        || !spec.prior_standard_deviation_pixels.is_finite()
        || spec.prior_standard_deviation_pixels <= 0.0
        || !spec.likelihood_noise_standard_deviation.is_finite()
        || spec.likelihood_noise_standard_deviation <= 0.0
        || !(8..=128).contains(&spec.variational_samples)
        || !spec.variational_samples.is_multiple_of(2)
        || !(20..=1_000).contains(&spec.posterior_draws)
        || !(10..=2_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "probabilistic SVF requires bounded same-grid images, a constant-translation velocity family, positive prior/noise, antithetic variational samples, and bounded posterior controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_lddmm_landmarks(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: LddmmLandmarkSpec = serde_json::from_slice(&bytes)?;
    validate_lddmm_landmarks(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_lddmm_landmark_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_lddmm_landmark_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_landmark_lddmm",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "source_frame": spec.source_frame,
        "target_frame": spec.target_frame,
        "source_landmarks": spec.source_landmarks,
        "target_landmarks": spec.target_landmarks,
        "kernel": spec.kernel,
        "kernel_scale_um": spec.kernel_scale_um,
        "data_weight": spec.data_weight,
        "time_steps": spec.time_steps,
        "maximum_iterations": spec.maximum_iterations
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.lddmm_landmark_registration"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_landmark_lddmm"
    {
        return Err(TopologyCliError::Backend(
            "LDDMM landmark result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_lddmm_landmarks(spec: &LddmmLandmarkSpec) -> Result<(), TopologyCliError> {
    let unique_points = |points: &[[f64; 2]]| {
        let mut unique = HashSet::new();
        points.iter().all(|point| {
            point.iter().all(|value| value.is_finite())
                && unique.insert((point[0].to_bits(), point[1].to_bits()))
        })
    };
    if spec.source_frame.trim().is_empty()
        || spec.source_frame.trim() != spec.source_frame
        || spec.target_frame.trim().is_empty()
        || spec.target_frame.trim() != spec.target_frame
        || spec.source_frame == spec.target_frame
        || !(3..=64).contains(&spec.source_landmarks.len())
        || spec.source_landmarks.len() != spec.target_landmarks.len()
        || !unique_points(&spec.source_landmarks)
        || !unique_points(&spec.target_landmarks)
        || spec.kernel != "gaussian"
        || !spec.kernel_scale_um.is_finite()
        || spec.kernel_scale_um <= 0.0
        || !spec.data_weight.is_finite()
        || spec.data_weight <= 0.0
        || !(8..=256).contains(&spec.time_steps)
        || !(10..=2_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "landmark LDDMM requires unique paired physical landmarks/frames, a positive Gaussian kernel/data weight, and bounded shooting controls"
                .into(),
        ));
    }
    Ok(())
}

fn run_svf(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: SvfSpec = serde_json::from_slice(&bytes)?;
    validate_svf(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_jax_svf_registration_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.jax_svf_registration_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_svf",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "fixed": spec.fixed,
        "moving": spec.moving,
        "metric": spec.metric,
        "regularization_weight": spec.regularization_weight,
        "squaring_steps": spec.squaring_steps,
        "maximum_iterations": spec.maximum_iterations,
        "jacobian_tolerance": spec.jacobian_tolerance
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.svf_diffeomorphic_registration"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_svf_diffeomorphism"
    {
        return Err(TopologyCliError::Backend(
            "SVF registration result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_svf(spec: &SvfSpec) -> Result<(), TopologyCliError> {
    let height = spec.fixed.pixels.len();
    let width = spec.fixed.pixels.first().map(Vec::len).unwrap_or(0);
    let valid_image = |image: &RegistrationImage| {
        !image.frame.trim().is_empty()
            && image.frame.trim() == image.frame
            && image.spacing_um == spec.fixed.spacing_um
            && image
                .spacing_um
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
            && image.pixels.len() == height
            && image
                .pixels
                .iter()
                .all(|row| row.len() == width && row.iter().all(|value| value.is_finite()))
    };
    if !(16..=64).contains(&height)
        || !(16..=64).contains(&width)
        || !valid_image(&spec.fixed)
        || !valid_image(&spec.moving)
        || spec.fixed.frame == spec.moving.frame
        || spec.metric != "mean_squares_same_stain"
        || !spec.regularization_weight.is_finite()
        || spec.regularization_weight <= 0.0
        || !(1..=8).contains(&spec.squaring_steps)
        || !(10..=1_000).contains(&spec.maximum_iterations)
        || !spec.jacobian_tolerance.is_finite()
        || !(0.0..1.0).contains(&spec.jacobian_tolerance)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "SVF registration requires same-grid bounded finite images, same-stain mean squares, positive regularization, bounded scaling/squaring, and a positive Jacobian tolerance"
                .into(),
        ));
    }
    Ok(())
}

fn run_nonrigid(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: NonrigidSpec = serde_json::from_slice(&bytes)?;
    validate_nonrigid(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_simpleitk_nonrigid_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.simpleitk_nonrigid_request",
        "version": 1,
        "backend": {
            "name": "SimpleITK",
            "version": "SimpleITK-2.5.5",
            "itk_family": "ITK-5.4",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "fixed": spec.fixed,
        "moving": spec.moving,
        "fixed_mask": spec.fixed_mask,
        "moving_mask": spec.moving_mask,
        "metric": spec.metric,
        "mesh_size": spec.mesh_size,
        "shrink_factors": spec.shrink_factors,
        "smoothing_sigmas_um": spec.smoothing_sigmas_um,
        "maximum_iterations": spec.maximum_iterations
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.multiresolution_nonrigid_registration"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_nonrigid_registration"
    {
        return Err(TopologyCliError::Backend(
            "nonrigid registration result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_nonrigid(spec: &NonrigidSpec) -> Result<(), TopologyCliError> {
    let height = spec.fixed.pixels.len();
    let width = spec.fixed.pixels.first().map(Vec::len).unwrap_or(0);
    let valid_image = |image: &RegistrationImage| {
        !image.frame.trim().is_empty()
            && image.frame.trim() == image.frame
            && image
                .spacing_um
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
            && image.pixels.len() == height
            && image
                .pixels
                .iter()
                .all(|row| row.len() == width && row.iter().all(|value| value.is_finite()))
    };
    let valid_mask = |mask: &[Vec<bool>]| {
        mask.len() == height
            && mask.iter().all(|row| row.len() == width)
            && mask.iter().flatten().filter(|value| **value).count() >= 64
    };
    let shrink_unique = spec.shrink_factors.iter().copied().collect::<HashSet<_>>();
    if !(16..=256).contains(&height)
        || !(16..=256).contains(&width)
        || height.saturating_mul(width) > 65_536
        || !valid_image(&spec.fixed)
        || !valid_image(&spec.moving)
        || spec.fixed.frame == spec.moving.frame
        || !valid_mask(&spec.fixed_mask)
        || !valid_mask(&spec.moving_mask)
        || spec.metric != "mean_squares_same_stain"
        || spec.mesh_size.iter().any(|size| !(2..=16).contains(size))
        || !(1..=4).contains(&spec.shrink_factors.len())
        || spec.shrink_factors.len() != spec.smoothing_sigmas_um.len()
        || spec.shrink_factors.last() != Some(&1)
        || shrink_unique.len() != spec.shrink_factors.len()
        || spec
            .shrink_factors
            .windows(2)
            .any(|window| window[0] <= window[1])
        || spec
            .smoothing_sigmas_um
            .iter()
            .any(|sigma| !sigma.is_finite() || *sigma < 0.0)
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "nonrigid registration requires bounded finite physical images, valid masks/frames, same-stain mean squares, and a canonical coarse-to-fine B-spline plan"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
