use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::{BayesianPccaSpec, PairedMultimodalDesign, PairedMultimodalRow};
use super::worker_assets;

pub(super) fn run_bayesian_pcca(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: BayesianPccaSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_common_design(&spec.design, &spec.rows, spec.latent_dimensions)?;
    if spec.latent_dimensions != 1
        || spec.priors != "cca_zoo_standard_normal_loadings_log_noise"
        || !(20..=10_000).contains(&spec.warmup)
        || !(20..=10_000).contains(&spec.samples)
        || !spec.target_accept.is_finite()
        || !(0.8..1.0).contains(&spec.target_accept)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "Bayesian pCCA requires pinned priors, one factor, bounded draws, target acceptance, and timeout"
                .into(),
        ));
    }
    let assets = worker_assets::load("marklab_ccazoo_bayesian_pcca_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.ccazoo_bayesian_pcca_request",
        "version": 1,
        "backend": {
            "name": "cca_zoo_numpyro_jax",
            "cca_zoo_version": "3.0.0",
            "numpyro_version": "0.21.0",
            "jax_version": "0.11.1",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "MIT_plus_Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "design": spec.design,
        "rows": spec.rows,
        "latent_dimensions": spec.latent_dimensions,
        "priors": spec.priors,
        "warmup": spec.warmup,
        "samples": spec.samples,
        "target_accept": spec.target_accept,
        "seed": spec.seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &assets.repository,
        &assets.worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.bayesian_pcca"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["design"]["validation_status"] != "passed"
        || !matches!(
            result["claim_status"].as_str(),
            Some("experimental_synthetic_bayesian_pcca")
                | Some("unsupported_for_claim_nonconverged")
        )
    {
        return Err(TopologyCliError::Backend(
            "Bayesian pCCA result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

pub(super) fn run_pcca(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let (request, timeout_seconds) = marklab::pcca_em::native_request(bytes)
        .map_err(|error| TopologyCliError::Input(error.to_string()))?;
    let response = marklab::run_native_pcca_em(request, timeout_seconds)
        .map_err(|error| TopologyCliError::Backend(error.to_string()))?;
    // RawValue validates syntax while preserving the native serializer's exact f64 decimals.
    let result: &serde_json::value::RawValue = serde_json::from_slice(&response)?;
    publish_json(&out, &result)
}

fn validate_common_design(
    design: &PairedMultimodalDesign,
    rows: &[PairedMultimodalRow],
    latent_dimensions: usize,
) -> Result<(), TopologyCliError> {
    let dx = design.modality_x.feature_names.len();
    let dy = design.modality_y.feature_names.len();
    let valid_design = design.entity_level == "patient"
        && design.modality_x.id != design.modality_y.id
        && design.modality_x.measurement_status == "measured"
        && design.modality_y.measurement_status == "measured"
        && design.modality_x.likelihood == "gaussian"
        && design.modality_y.likelihood == "gaussian"
        && design.missingness_assumption == "complete_paired_rows"
        && design
            .coordinate_frame
            .as_ref()
            .is_none_or(|frame| !frame.trim().is_empty());
    let train_count = rows.iter().filter(|row| row.split == "train").count();
    let test_count = rows.iter().filter(|row| row.split == "test").count();
    let mut entity_ids = HashSet::new();
    if !valid_design
        || !(1..=32).contains(&dx)
        || !(1..=32).contains(&dy)
        || rows.len() < 9
        || rows.len() > 10_000
        || train_count < 8
        || test_count == 0
        || !(1..=dx.min(dy)).contains(&latent_dimensions)
        || rows.iter().any(|row| {
            row.entity_id.trim().is_empty()
                || row.entity_id.trim() != row.entity_id
                || !entity_ids.insert(row.entity_id.as_str())
                || !matches!(row.split.as_str(), "train" | "test")
                || row.x.len() != dx
                || row.y.len() != dy
                || row.x.iter().chain(&row.y).any(|value| !value.is_finite())
        })
    {
        return Err(TopologyCliError::Input(
            "multimodal design requires paired measured patient Gaussian modalities, train/test rows, and valid dimensions"
                .into(),
        ));
    }
    for names in [
        &design.modality_x.feature_names,
        &design.modality_y.feature_names,
    ] {
        let mut unique = HashSet::new();
        if names.iter().any(|name| {
            name.trim().is_empty() || name.trim() != name || !unique.insert(name.as_str())
        }) {
            return Err(TopologyCliError::Input(
                "multimodal feature names must be unique exact strings within each modality".into(),
            ));
        }
    }
    Ok(())
}
