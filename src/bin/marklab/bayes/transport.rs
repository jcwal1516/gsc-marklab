use super::{embedding_spatial, publish_json, BayesCliError};
use std::path::PathBuf;

pub(super) fn run_sinkhorn(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    epsilon: f64,
    tolerance: f64,
    maximum_iterations: u32,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let result = marklab::transport::fit_sinkhorn_csv(
        &source_bytes,
        &target_bytes,
        &cost_bytes,
        epsilon,
        tolerance,
        maximum_iterations,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    publish_json(&out, &result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_unbalanced(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    epsilon: f64,
    tau_source: f64,
    tau_target: f64,
    tolerance: f64,
    maximum_iterations: u32,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let result = marklab::transport::fit_unbalanced_csv(
        &source_bytes,
        &target_bytes,
        &cost_bytes,
        epsilon,
        tau_source,
        tau_target,
        tolerance,
        maximum_iterations,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    publish_json(&out, &result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_soft_assignment(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    epsilon: f64,
    dustbin_cost: f64,
    tolerance: f64,
    maximum_iterations: u32,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let result = marklab::transport::fit_soft_assignment_csv(
        &source_bytes,
        &target_bytes,
        &cost_bytes,
        epsilon,
        dustbin_cost,
        tolerance,
        maximum_iterations,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    publish_json(&out, &result)
}

pub(super) fn run_partial(
    source: PathBuf,
    target: PathBuf,
    cost: PathBuf,
    transported_mass: f64,
    epsilon: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let source_bytes = embedding_spatial::read(&source)?;
    let target_bytes = embedding_spatial::read(&target)?;
    let cost_bytes = embedding_spatial::read(&cost)?;
    let result = marklab::transport::run_partial_csv(
        &source_bytes,
        &target_bytes,
        &cost_bytes,
        transported_mass,
        epsilon,
        timeout_seconds,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let result: &serde_json::value::RawValue = serde_json::from_slice(&result)?;
    publish_json(&out, &result)
}
