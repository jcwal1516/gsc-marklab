use std::path::PathBuf;

use marklab_topology::sha256_hex;
use serde::Deserialize;

use super::{
    super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError},
    RegistrationImage,
};

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
pub(super) fn run_svf(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: SvfSpec = serde_json::from_slice(&bytes)?;
    validate_svf(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
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
