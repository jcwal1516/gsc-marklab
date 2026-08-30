use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::TensorFactorSpec;
use super::worker_assets;

pub(super) fn run_tensor_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: TensorFactorSpec = serde_json::from_slice(&bytes)?;
    spec.entries
        .sort_by(|left, right| left.indices.cmp(&right.indices));
    validate_tensor_factor(&spec)?;
    let assets = worker_assets::load("marklab_jax_scipy_tensor_factor_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.jax_scipy_tensor_factor_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_l_bfgs_laplace",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "tensor_id": spec.tensor_id,
        "mode_names": spec.mode_names,
        "shape": spec.shape,
        "entries": spec.entries,
        "decomposition": spec.decomposition,
        "ranks": spec.ranks,
        "prior_precision": spec.prior_precision,
        "noise_standard_deviation": spec.noise_standard_deviation,
        "maximum_iterations": spec.maximum_iterations,
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
    let expected_format = match spec.decomposition.as_str() {
        "cp" => "marklab.bayesian_cp_factorization",
        "tucker" => "marklab.bayesian_tucker_factorization",
        _ => unreachable!("validated decomposition"),
    };
    let expected_claim = format!(
        "experimental_synthetic_bayesian_{}_factorization",
        spec.decomposition
    );
    if result["format"] != expected_format
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != expected_claim
    {
        return Err(TopologyCliError::Backend(
            "tensor factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_tensor_factor(spec: &TensorFactorSpec) -> Result<(), TopologyCliError> {
    let mut modes = HashSet::new();
    let valid_modes = spec
        .mode_names
        .iter()
        .all(|mode| !mode.trim().is_empty() && mode.trim() == mode && modes.insert(mode.as_str()));
    let total_entries = spec
        .shape
        .iter()
        .try_fold(1usize, |total, dimension| total.checked_mul(*dimension));
    let mut indices = HashSet::new();
    let valid_entries = spec.entries.iter().all(|entry| {
        entry.indices.len() == 3
            && entry
                .indices
                .iter()
                .zip(&spec.shape)
                .all(|(index, dimension)| index < dimension)
            && entry.value.is_finite()
            && indices.insert(entry.indices.as_slice())
    });
    let valid_ranks = match spec.decomposition.as_str() {
        "cp" => {
            spec.ranks.len() == 1
                && (1..=spec.shape.iter().copied().min().unwrap_or(0)).contains(&spec.ranks[0])
        }
        "tucker" => {
            spec.ranks.len() == 3
                && spec
                    .ranks
                    .iter()
                    .zip(&spec.shape)
                    .all(|(rank, dimension)| (1..=*dimension).contains(rank))
        }
        _ => false,
    };
    let parameter_count = if spec.decomposition == "cp" && spec.ranks.len() == 1 {
        spec.shape.iter().sum::<usize>() * spec.ranks[0]
    } else if spec.ranks.len() == 3 {
        spec.shape
            .iter()
            .zip(&spec.ranks)
            .map(|(dimension, rank)| dimension * rank)
            .sum::<usize>()
            + spec.ranks.iter().product::<usize>()
    } else {
        usize::MAX
    };
    if spec.tensor_id.trim().is_empty()
        || spec.tensor_id.trim() != spec.tensor_id
        || spec.mode_names.len() != 3
        || spec.shape.len() != 3
        || !valid_modes
        || spec
            .shape
            .iter()
            .any(|dimension| !(2..=16).contains(dimension))
        || total_entries != Some(spec.entries.len())
        || !valid_entries
        || !spec.entries.iter().any(|entry| !entry.observed)
        || spec.entries.iter().filter(|entry| entry.observed).count() < 8
        || !valid_ranks
        || parameter_count > 512
        || !spec.prior_precision.is_finite()
        || spec.prior_precision <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "tensor factorization requires one complete bounded three-mode tensor index, held-out entries, valid CP/Tucker ranks, and positive Laplace controls"
                .into(),
        ));
    }
    Ok(())
}
