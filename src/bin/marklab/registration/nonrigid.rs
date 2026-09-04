use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;
use serde::Deserialize;

use super::{
    super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError},
    RegistrationImage,
};

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
pub(super) fn run_nonrigid(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: NonrigidSpec = serde_json::from_slice(&bytes)?;
    validate_nonrigid(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
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
