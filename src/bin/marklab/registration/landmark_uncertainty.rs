use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;
use serde::{Deserialize, Serialize};

use super::super::topology::{
    publish_json, read_input, read_required, run_worker, TopologyCliError,
};

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
pub(super) fn run_landmark_uncertainty(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
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
