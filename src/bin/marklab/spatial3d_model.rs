use std::{collections::HashSet, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_topology::sha256_hex;
use serde::{Deserialize, Serialize};

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Spatial3dModelCli {
    #[command(subcommand)]
    command: Spatial3dTopLevel,
}

#[derive(Debug, Subcommand)]
enum Spatial3dTopLevel {
    Spatial3d {
        #[command(subcommand)]
        command: Spatial3dModelCommand,
    },
}

#[derive(Debug, Subcommand)]
enum Spatial3dModelCommand {
    SerialStack {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AlphaComplex {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    DeformationBiology {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CloneModels {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ValidateAdvanced {
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
struct SerialSection {
    section_id: String,
    z_um: f64,
    observed_landmarks: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerialStackSpec {
    stack_id: String,
    coordinate_unit: String,
    sections: Vec<SerialSection>,
    reference_section_id: String,
    landmark_noise_standard_deviation_um: f64,
    posterior_draws: usize,
    seed: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AlphaPoint3d {
    point_id: String,
    coordinates_um: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlphaComplex3dSpec {
    coordinate_frame: String,
    points: Vec<AlphaPoint3d>,
    maximum_squared_alpha_um2: f64,
    maximum_homology_dimension: usize,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeformationBiologyPoint {
    point_id: String,
    pre_coordinates: [f64; 2],
    pre_value: f64,
    post_coordinates: [f64; 2],
    post_value: f64,
    negative_control_post_value: f64,
    domain_indicator: f64,
    independent_change_measurement: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeformationBiologySpec {
    coordinate_frame: String,
    points: Vec<DeformationBiologyPoint>,
    deformation_translation_draws: Vec<[f64; 2]>,
    change_model: String,
    interpolation: String,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CloneBranch {
    parent: String,
    child: String,
    length: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CloneTree {
    tree_id: String,
    root_id: String,
    nodes: Vec<String>,
    branches: Vec<CloneBranch>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CloneLocation {
    clone_id: String,
    mean_xy_um: [f64; 2],
    covariance_um2: [[f64; 2]; 2],
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CloneCell {
    cell_id: String,
    patient_id: String,
    coordinates_um: [f64; 2],
    clone_probabilities: Vec<f64>,
    neighborhood_features: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiffusionPrior {
    inverse_gamma_shape: f64,
    inverse_gamma_scale: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CloneModelsSpec {
    tree: CloneTree,
    clone_labels: Vec<String>,
    clone_locations: Vec<CloneLocation>,
    cells: Vec<CloneCell>,
    diffusion_prior: DiffusionPrior,
    posterior_draws: usize,
    seed: u64,
    timeout_seconds: u64,
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match Spatial3dModelCli::parse().command {
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dModelCommand::SerialStack { input, out },
        } => run_serial_stack(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dModelCommand::AlphaComplex { input, out },
        } => run_alpha_complex(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dModelCommand::DeformationBiology { input, out },
        } => run_deformation_biology(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dModelCommand::CloneModels { input, out },
        } => run_clone_models(input, out),
        Spatial3dTopLevel::Spatial3d {
            command:
                Spatial3dModelCommand::ValidateAdvanced {
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_validate_advanced(seed, timeout_seconds, out),
    }
}

fn run_validate_advanced(
    seed: u64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    if !(1..=3_600).contains(&timeout_seconds) {
        return Err(TopologyCliError::Input(
            "advanced 3-D validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_advanced3d_validation_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.advanced3d_validation_request",
        "version": 1,
        "backend": {
            "name": "gudhi_numpy_scipy_advanced3d_validation",
            "gudhi_version": "3.13.0",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "GPLv3_effective_via_CGAL_plus_BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "seed": seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, timeout_seconds)?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.advanced_3d_longitudinal_validation_suite"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "synthetic_3d_longitudinal_validation_with_explicit_real_gap"
    {
        return Err(TopologyCliError::Backend(
            "advanced 3-D validation result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn run_clone_models(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: CloneModelsSpec = serde_json::from_slice(&bytes)?;
    spec.cells
        .sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    spec.clone_locations
        .sort_by(|left, right| left.clone_id.cmp(&right.clone_id));
    validate_clone_models(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_clone_models_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_clone_models_request",
        "version": 1,
        "backend": {
            "name": "scipy_uncertain_clone_models",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "tree": spec.tree,
        "clone_labels": spec.clone_labels,
        "clone_locations": spec.clone_locations,
        "cells": spec.cells,
        "diffusion_prior": spec.diffusion_prior,
        "posterior_draws": spec.posterior_draws,
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
    if result["format"] != "marklab.clone_phylogeography_and_niche_models"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_clone_models_no_identified_history"
    {
        return Err(TopologyCliError::Backend(
            "clone model result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_clone_models(spec: &CloneModelsSpec) -> Result<(), TopologyCliError> {
    let mut nodes = HashSet::new();
    let valid_nodes =
        spec.tree.nodes.iter().all(|node| {
            !node.trim().is_empty() && node.trim() == node && nodes.insert(node.as_str())
        });
    let mut children = HashSet::new();
    let valid_branches = spec.tree.branches.iter().all(|branch| {
        nodes.contains(branch.parent.as_str())
            && nodes.contains(branch.child.as_str())
            && branch.parent != branch.child
            && children.insert(branch.child.as_str())
            && branch.length.is_finite()
            && branch.length > 0.0
    });
    let mut labels = HashSet::new();
    let valid_labels = spec.clone_labels.iter().all(|label| {
        !label.trim().is_empty() && label.trim() == label && labels.insert(label.as_str())
    });
    let mut location_ids = HashSet::new();
    let valid_locations = spec.clone_locations.iter().all(|location| {
        let covariance = location.covariance_um2;
        let determinant = covariance[0][0] * covariance[1][1] - covariance[0][1] * covariance[1][0];
        nodes.contains(location.clone_id.as_str())
            && location_ids.insert(location.clone_id.as_str())
            && location.mean_xy_um.iter().all(|value| value.is_finite())
            && covariance.iter().flatten().all(|value| value.is_finite())
            && covariance[0][1] == covariance[1][0]
            && covariance[0][0] > 0.0
            && determinant > 0.0
    });
    let feature_count = spec
        .cells
        .first()
        .map(|cell| cell.neighborhood_features.len())
        .unwrap_or(0);
    let mut cell_ids = HashSet::new();
    let mut patients = HashSet::new();
    let valid_cells = spec.cells.iter().all(|cell| {
        patients.insert(cell.patient_id.as_str());
        !cell.cell_id.trim().is_empty()
            && cell.cell_id.trim() == cell.cell_id
            && cell_ids.insert(cell.cell_id.as_str())
            && !cell.patient_id.trim().is_empty()
            && cell.coordinates_um.iter().all(|value| value.is_finite())
            && cell.clone_probabilities.len() == spec.clone_labels.len()
            && cell
                .clone_probabilities
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
            && (cell.clone_probabilities.iter().sum::<f64>() - 1.0).abs() <= 1e-12
            && cell.neighborhood_features.len() == feature_count
            && cell
                .neighborhood_features
                .iter()
                .all(|value| value.is_finite())
    });
    if spec.tree.tree_id.trim().is_empty()
        || !(3..=128).contains(&spec.tree.nodes.len())
        || !valid_nodes
        || !nodes.contains(spec.tree.root_id.as_str())
        || spec.tree.branches.len() != spec.tree.nodes.len() - 1
        || !valid_branches
        || children.contains(spec.tree.root_id.as_str())
        || children.len() != spec.tree.nodes.len() - 1
        || spec.clone_labels.len() != 2
        || !valid_labels
        || !spec
            .clone_labels
            .iter()
            .all(|label| nodes.contains(label.as_str()))
        || spec.clone_locations.len() != spec.tree.nodes.len()
        || !valid_locations
        || location_ids.len() != nodes.len()
        || !(40..=100_000).contains(&spec.cells.len())
        || !(1..=32).contains(&feature_count)
        || !valid_cells
        || patients.len() < 4
        || !spec.diffusion_prior.inverse_gamma_shape.is_finite()
        || spec.diffusion_prior.inverse_gamma_shape <= 1.0
        || !spec.diffusion_prior.inverse_gamma_scale.is_finite()
        || spec.diffusion_prior.inverse_gamma_scale <= 0.0
        || !(100..=2_000).contains(&spec.posterior_draws)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "clone models require one rooted imported tree, SPD location uncertainty for every node, two clone labels, normalized uncertain assignments, replicated patients/features, and bounded posterior draws".into(),
        ));
    }
    Ok(())
}

fn run_deformation_biology(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: DeformationBiologySpec = serde_json::from_slice(&bytes)?;
    spec.points
        .sort_by(|left, right| left.point_id.cmp(&right.point_id));
    validate_deformation_biology(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_deformation_biology_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_deformation_biology_request",
        "version": 1,
        "backend": {
            "name": "scipy_rbf_deformation_biology",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "coordinate_frame": spec.coordinate_frame,
        "points": spec.points,
        "deformation_translation_draws": spec.deformation_translation_draws,
        "change_model": spec.change_model,
        "interpolation": spec.interpolation
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.deformation_biology_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_deformation_biology_separation"
    {
        return Err(TopologyCliError::Backend(
            "deformation-biology result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_deformation_biology(spec: &DeformationBiologySpec) -> Result<(), TopologyCliError> {
    let mut point_ids = HashSet::new();
    let mut pre_coordinates = HashSet::new();
    let mut post_coordinates = HashSet::new();
    let valid_points = spec.points.iter().all(|point| {
        !point.point_id.trim().is_empty()
            && point.point_id.trim() == point.point_id
            && point_ids.insert(point.point_id.as_str())
            && point
                .pre_coordinates
                .iter()
                .chain(&point.post_coordinates)
                .all(|value| value.is_finite())
            && pre_coordinates.insert((
                point.pre_coordinates[0].to_bits(),
                point.pre_coordinates[1].to_bits(),
            ))
            && post_coordinates.insert((
                point.post_coordinates[0].to_bits(),
                point.post_coordinates[1].to_bits(),
            ))
            && [
                point.pre_value,
                point.post_value,
                point.negative_control_post_value,
                point.domain_indicator,
                point.independent_change_measurement,
            ]
            .iter()
            .all(|value| value.is_finite())
            && matches!(point.domain_indicator, 0.0 | 1.0)
    });
    if spec.coordinate_frame.trim().is_empty()
        || spec.coordinate_frame.trim() != spec.coordinate_frame
        || !(16..=10_000).contains(&spec.points.len())
        || !valid_points
        || !spec
            .points
            .iter()
            .any(|point| point.domain_indicator == 0.0)
        || !spec
            .points
            .iter()
            .any(|point| point.domain_indicator == 1.0)
        || !(100..=2_000).contains(&spec.deformation_translation_draws.len())
        || spec
            .deformation_translation_draws
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        || spec.change_model != "intercept_plus_prespecified_domain"
        || spec.interpolation != "thin_plate_spline"
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "deformation-biology separation requires paired finite fields, prespecified binary domain, deformation posterior draws, negative control, independent measurement, and bounded interpolation work".into(),
        ));
    }
    Ok(())
}

fn run_alpha_complex(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: AlphaComplex3dSpec = serde_json::from_slice(&bytes)?;
    spec.points
        .sort_by(|left, right| left.point_id.cmp(&right.point_id));
    validate_alpha_complex(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_alpha3d_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.gudhi_alpha3d_request",
        "version": 1,
        "backend": {
            "name": "gudhi_exact_alpha_complex_3d",
            "version": "gudhi-3.13.0",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "GPLv3_effective_via_CGAL",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "coordinate_frame": spec.coordinate_frame,
        "points": spec.points,
        "maximum_squared_alpha_um2": spec.maximum_squared_alpha_um2,
        "maximum_homology_dimension": spec.maximum_homology_dimension
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.alpha_complex_3d"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_exact_3d_alpha_complex"
    {
        return Err(TopologyCliError::Backend(
            "3-D alpha result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_alpha_complex(spec: &AlphaComplex3dSpec) -> Result<(), TopologyCliError> {
    let mut point_ids = HashSet::new();
    let mut coordinates = HashSet::new();
    let valid_points = spec.points.iter().all(|point| {
        !point.point_id.trim().is_empty()
            && point.point_id.trim() == point.point_id
            && point_ids.insert(point.point_id.as_str())
            && point.coordinates_um.iter().all(|value| value.is_finite())
            && coordinates.insert((
                point.coordinates_um[0].to_bits(),
                point.coordinates_um[1].to_bits(),
                point.coordinates_um[2].to_bits(),
            ))
    });
    if spec.coordinate_frame.trim().is_empty()
        || spec.coordinate_frame.trim() != spec.coordinate_frame
        || !(4..=128).contains(&spec.points.len())
        || !valid_points
        || !spec.maximum_squared_alpha_um2.is_finite()
        || spec.maximum_squared_alpha_um2 <= 0.0
        || spec.maximum_homology_dimension > 3
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "3-D alpha complexes require unique finite physical points/frame, positive squared alpha, bounded homology dimension, and bounded exact work".into(),
        ));
    }
    Ok(())
}

fn run_serial_stack(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: SerialStackSpec = serde_json::from_slice(&bytes)?;
    spec.sections
        .sort_by(|left, right| left.z_um.total_cmp(&right.z_um));
    validate_serial_stack(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_serial_stack_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_serial_stack_request",
        "version": 1,
        "backend": {
            "name": "scipy_gaussian_landmark_stack",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "stack_id": spec.stack_id,
        "coordinate_unit": spec.coordinate_unit,
        "sections": spec.sections,
        "reference_section_id": spec.reference_section_id,
        "landmark_noise_standard_deviation_um": spec.landmark_noise_standard_deviation_um,
        "posterior_draws": spec.posterior_draws,
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
    if result["format"] != "marklab.bayesian_serial_section_stack"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_serial_stack_reconstruction"
    {
        return Err(TopologyCliError::Backend(
            "serial stack result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_serial_stack(spec: &SerialStackSpec) -> Result<(), TopologyCliError> {
    let landmark_count = spec
        .sections
        .first()
        .map(|section| section.observed_landmarks.len())
        .unwrap_or(0);
    let mut section_ids = HashSet::new();
    let valid_sections = spec.sections.iter().all(|section| {
        let mut landmarks = HashSet::new();
        !section.section_id.trim().is_empty()
            && section.section_id.trim() == section.section_id
            && section_ids.insert(section.section_id.as_str())
            && section.z_um.is_finite()
            && section.observed_landmarks.len() == landmark_count
            && section.observed_landmarks.iter().all(|point| {
                point.iter().all(|value| value.is_finite())
                    && landmarks.insert((point[0].to_bits(), point[1].to_bits()))
            })
    });
    if spec.stack_id.trim().is_empty()
        || spec.stack_id.trim() != spec.stack_id
        || spec.coordinate_unit != "um"
        || !(3..=128).contains(&spec.sections.len())
        || !(4..=128).contains(&landmark_count)
        || !valid_sections
        || spec
            .sections
            .windows(2)
            .any(|window| window[0].z_um >= window[1].z_um)
        || !section_ids.contains(spec.reference_section_id.as_str())
        || !spec.landmark_noise_standard_deviation_um.is_finite()
        || spec.landmark_noise_standard_deviation_um <= 0.0
        || !(100..=2_000).contains(&spec.posterior_draws)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "serial stacks require ordered unique physical sections, paired unique landmarks, a reference section, positive landmark noise, and bounded posterior draws"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Spatial3dModelCli>(|| run_cli().map_err(into_marklab_error))
}
