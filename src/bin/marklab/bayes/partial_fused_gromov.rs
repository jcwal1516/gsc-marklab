use std::{fs, path::PathBuf};

use marklab_bayes::{
    FgwPlanEntry, FgwSupport, PartialFusedGromovWassersteinSpec,
    PartialFusedGromovWassersteinWorkerRequest, PartialFusedGromovWassersteinWorkerResult,
};
use serde::{Deserialize, Serialize};

use super::{embedding_spatial, publish_json, run_worker, BayesCliError};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    source: Vec<FgwSupport>,
    target: Vec<FgwSupport>,
    source_structure_row_major: Vec<f64>,
    target_structure_row_major: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input: PathBuf,
    transported_mass: f64,
    alpha: f64,
    epsilon: f64,
    feature_scale: f64,
    structure_scale: f64,
    tolerance: f64,
    maximum_iterations: u32,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = embedding_spatial::read(&input)?;
    let input: Input = serde_json::from_slice(&input_bytes)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_name = "marklab_pot_partial_fgw_worker.py";
    let worker_path = worker_directory.join(worker_name);
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PartialFusedGromovWassersteinWorkerRequest::new(
        PartialFusedGromovWassersteinSpec {
            source: input.source,
            target: input.target,
            source_structure_row_major: input.source_structure_row_major,
            target_structure_row_major: input.target_structure_row_major,
            transported_mass,
            alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            timeout_seconds,
        },
        marklab_bayes::sha256_hex(&lock_bytes),
        marklab_bayes::sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = marklab_bayes::sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, worker_name, &request_bytes, timeout_seconds)?;
    let result: PartialFusedGromovWassersteinWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.partial_fused_gromov_wasserstein",
            version: 1,
            input_sha256: marklab_bayes::sha256_hex(&input_bytes),
            backend: result.backend,
            request_sha256: result.request_sha256,
            fit_state: result.fit_state,
            transported_mass_control: transported_mass,
            alpha,
            feature_weight: alpha,
            structure_weight: 1.0 - alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            best_initialization: result.best_initialization,
            best_plan: result.best_plan,
            source_marginals: result.source_marginals,
            target_marginals: result.target_marginals,
            transported_mass: result.transported_mass,
            unmatched_source_mass: result.unmatched_source_mass,
            unmatched_target_mass: result.unmatched_target_mass,
            initialization_sensitivity: result.initialization_sensitivity,
            parameter_sensitivity: result.parameter_sensitivity,
            unbalanced_fgw_status: "blocked_no_pinned_fgw_backend_with_kl_relaxed_marginals",
            claim_status: "ensemble_descriptive_alignment_not_correspondence",
        },
    )
}

#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    input_sha256: String,
    backend: marklab_bayes::WorkerBackend,
    request_sha256: String,
    fit_state: marklab_bayes::FitState,
    transported_mass_control: f64,
    alpha: f64,
    feature_weight: f64,
    structure_weight: f64,
    epsilon: f64,
    feature_scale: f64,
    structure_scale: f64,
    tolerance: f64,
    maximum_iterations: u32,
    best_initialization: String,
    best_plan: Vec<FgwPlanEntry>,
    source_marginals: Vec<f64>,
    target_marginals: Vec<f64>,
    transported_mass: f64,
    unmatched_source_mass: Vec<f64>,
    unmatched_target_mass: Vec<f64>,
    initialization_sensitivity: Vec<marklab_bayes::PartialFgwFit>,
    parameter_sensitivity: Vec<marklab_bayes::PartialFgwFit>,
    unbalanced_fgw_status: &'static str,
    claim_status: &'static str,
}
