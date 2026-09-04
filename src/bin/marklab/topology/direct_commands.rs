use std::path::PathBuf;

use marklab_topology::{
    connectivity_transition, sha256_hex, AlphaPersistenceResult, AlphaPersistenceSpec,
    AlphaPersistenceWorkerRequest, ConnectivityTransitionSpec,
};

use super::{publish_json, read_input, read_required, run_worker, TopologyCliError};

pub(super) fn run_validation(out: PathBuf) -> Result<(), TopologyCliError> {
    let repository = marklab::python_backend_assets_root()?;
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

pub(super) fn run_stability(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
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
    let repository = marklab::python_backend_assets_root()?;
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

pub(super) fn run_compare_persistence(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
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
    let repository = marklab::python_backend_assets_root()?;
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

pub(super) fn run_connectivity(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
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

pub(super) fn run_raster_morphology(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
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
    let repository = marklab::python_backend_assets_root()?;
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
pub(super) fn run_alpha_persistence(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: AlphaPersistenceSpec = serde_json::from_slice(&bytes)?;
    let repository = marklab::python_backend_assets_root()?;
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
