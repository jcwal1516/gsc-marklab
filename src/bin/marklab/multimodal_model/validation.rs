use std::path::PathBuf;

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, run_worker, TopologyCliError};
use super::worker_assets;

pub(super) fn run_validation_suite(
    seed: u64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    if !(1..=3_600).contains(&timeout_seconds) {
        return Err(TopologyCliError::Input(
            "multimodal validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let assets = worker_assets::load("marklab_multimodal_validation_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.multimodal_validation_request",
        "version": 1,
        "backend": {
            "name": "numpy_scipy_exact_synthetic_controls",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "seed": seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &assets.repository,
        &assets.worker_path,
        &request_bytes,
        timeout_seconds,
    )?;
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
