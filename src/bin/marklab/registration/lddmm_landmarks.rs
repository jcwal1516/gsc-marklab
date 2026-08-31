use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;
use serde::Deserialize;

use super::super::topology::{
    publish_json, read_input, read_required, run_worker, TopologyCliError,
};

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
pub(super) fn run_lddmm_landmarks(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
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
