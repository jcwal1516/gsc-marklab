use std::path::PathBuf;

use marklab_topology::sha256_hex;
use serde::Deserialize;

use super::{
    super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError},
    RegistrationImage,
};

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
pub(super) fn run_probabilistic_svf(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
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
