use std::fs;

use marklab_transport::{
    FgwPlanEntry, FgwSupport, UnbalancedFgwFit, UnbalancedFgwObjective,
    UnbalancedFusedGromovWassersteinSpec, UnbalancedFusedGromovWassersteinWorkerRequest,
    UnbalancedFusedGromovWassersteinWorkerResult,
};
use serde::{Deserialize, Serialize};

use super::cli_schema::UnbalancedFusedGromovWassersteinArgs;
use super::{embedding_spatial, publish_json, run_worker, CliError};

const WORKER: &str = "marklab_pot_unbalanced_fgw_worker.py";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    source: Vec<FgwSupport>,
    target: Vec<FgwSupport>,
    source_structure_row_major: Vec<f64>,
    target_structure_row_major: Vec<f64>,
}

pub(super) fn run(args: UnbalancedFusedGromovWassersteinArgs) -> Result<(), CliError> {
    let UnbalancedFusedGromovWassersteinArgs {
        input,
        linear_weight,
        marginal_relaxation,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        inner_tolerance,
        maximum_inner_iterations,
        timeout_seconds,
        out,
    } = args;
    let input_bytes = embedding_spatial::read(&input)?;
    let input: Input = serde_json::from_slice(&input_bytes)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let read =
        |path: std::path::PathBuf| fs::read(&path).map_err(|source| CliError::Io { path, source });
    let lock_bytes = read(directory.join("uv.lock"))?;
    let worker_bytes = read(directory.join(WORKER))?;
    let request = UnbalancedFusedGromovWassersteinWorkerRequest::new(
        UnbalancedFusedGromovWassersteinSpec {
            source: input.source,
            target: input.target,
            source_structure_row_major: input.source_structure_row_major,
            target_structure_row_major: input.target_structure_row_major,
            linear_weight,
            marginal_relaxation,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            inner_tolerance,
            maximum_iterations,
            maximum_inner_iterations,
            timeout_seconds,
        },
        marklab_bayes::sha256_hex(&lock_bytes),
        marklab_bayes::sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = marklab_bayes::sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, WORKER, &request_bytes, timeout_seconds)?;
    let result: UnbalancedFusedGromovWassersteinWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    let best = result.best().clone();
    publish_json(
        &out,
        &Output {
            format: "marklab.unbalanced_fused_gromov_wasserstein",
            version: 1,
            input_sha256: marklab_bayes::sha256_hex(&input_bytes),
            objective: "pot_fused_unbalanced_gromov_wasserstein_lower_bound_kl_marginals",
            solver: if epsilon == 0.0 {
                "kl_majorization_minimization"
            } else {
                "log_domain_sinkhorn"
            },
            backend: result.backend,
            request_sha256: result.request_sha256,
            fit_state: result.fit_state,
            linear_weight,
            marginal_relaxation,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            inner_tolerance,
            maximum_inner_iterations,
            best_initialization: result.best_initialization,
            best_plan: best.plan,
            source_marginals: best.source_marginals,
            target_marginals: best.target_marginals,
            transported_mass: best.transported_mass,
            source_marginal_minus_mass: best.source_marginal_minus_mass,
            target_marginal_minus_mass: best.target_marginal_minus_mass,
            coupling_difference_maximum: best.coupling_difference_maximum,
            best_objective: best.objective,
            objective_spread_across_starts: result.objective_spread,
            initialization_sensitivity: result.initialization_sensitivity,
            claim_status: "ensemble_descriptive_alignment_not_correspondence",
        },
    )
}

#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    input_sha256: String,
    objective: &'static str,
    solver: &'static str,
    backend: marklab_bayes::WorkerBackend,
    request_sha256: String,
    fit_state: marklab_bayes::FitState,
    linear_weight: f64,
    marginal_relaxation: f64,
    epsilon: f64,
    feature_scale: f64,
    structure_scale: f64,
    tolerance: f64,
    maximum_iterations: u32,
    inner_tolerance: f64,
    maximum_inner_iterations: u32,
    best_initialization: String,
    best_plan: Vec<FgwPlanEntry>,
    source_marginals: Vec<f64>,
    target_marginals: Vec<f64>,
    transported_mass: f64,
    source_marginal_minus_mass: Vec<f64>,
    target_marginal_minus_mass: Vec<f64>,
    coupling_difference_maximum: f64,
    best_objective: UnbalancedFgwObjective,
    objective_spread_across_starts: f64,
    initialization_sensitivity: Vec<UnbalancedFgwFit>,
    claim_status: &'static str,
}
