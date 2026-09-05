//! CSV admission and source-bound application results for the four transport workflows.
//! Scientific kernels consume typed data; only the partial-transport CLI uses a killable child.
use marklab_bayes::{
    EntropicSoftAssignmentSpec, PartialTransportSpec, SinkhornSpec, UnbalancedSinkhornSpec,
};
use serde::Serialize;
use thiserror::Error;
mod csv;

/// Input, scientific, or native execution failure; no result is published on error.
#[derive(Debug, Error)]
pub enum TransportCsvError {
    #[error("transport input: {0}")]
    Input(String),
    #[error("transport CSV: {0}")]
    Csv(#[from] ::csv::Error),
    #[error(transparent)]
    Transport(#[from] marklab_bayes::TransportError),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("transport JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("transport exact input: {0}")]
    Wire(#[from] std::io::Error),
    #[error(transparent)]
    Runtime(#[from] crate::NativeBackendError),
}

/// Existing file identities alongside the unchanged scientific result fields.
#[derive(Debug, Serialize)]
pub struct TransportCsvResult<T> {
    pub source_sha256: String,
    pub target_sha256: String,
    pub cost_sha256: String,
    #[serde(flatten)]
    pub result: T,
}
fn bind<T>(source: &[u8], target: &[u8], cost: &[u8], result: T) -> TransportCsvResult<T> {
    TransportCsvResult {
        source_sha256: marklab_bayes::sha256_hex(source),
        target_sha256: marklab_bayes::sha256_hex(target),
        cost_sha256: marklab_bayes::sha256_hex(cost),
        result,
    }
}

/// Fit equal-total Sinkhorn from strict source, target and complete cost CSV files.
/// Each file is bounded to 16 MiB; existing scientific controls and errors are preserved.
pub fn fit_sinkhorn_csv(
    source: &[u8],
    target: &[u8],
    cost: &[u8],
    epsilon: f64,
    tolerance: f64,
    maximum_iterations: u32,
) -> Result<TransportCsvResult<marklab_bayes::SinkhornResult>, TransportCsvError> {
    let source_rows = csv::read_source(source, 2000)?;
    let target_rows = csv::read_target(target, 2000)?;
    let costs_row_major = csv::read_costs(cost, &source_rows, &target_rows)?;
    let result = marklab_bayes::sinkhorn_ot(SinkhornSpec {
        source: source_rows,
        target: target_rows,
        costs_row_major,
        epsilon,
        tolerance,
        maximum_iterations,
    })?;
    Ok(bind(source, target, cost, result))
}

/// Fit KL-relaxed transport with the same bounded CSV and identity rules as Sinkhorn.
#[allow(clippy::too_many_arguments)]
pub fn fit_unbalanced_csv(
    source: &[u8],
    target: &[u8],
    cost: &[u8],
    epsilon: f64,
    tau_source: f64,
    tau_target: f64,
    tolerance: f64,
    maximum_iterations: u32,
) -> Result<TransportCsvResult<marklab_bayes::UnbalancedSinkhornResult>, TransportCsvError> {
    let source_rows = csv::read_source(source, 2000)?;
    let target_rows = csv::read_target(target, 2000)?;
    let costs_row_major = csv::read_costs(cost, &source_rows, &target_rows)?;
    let result = marklab_bayes::unbalanced_sinkhorn(UnbalancedSinkhornSpec {
        source: source_rows,
        target: target_rows,
        costs_row_major,
        epsilon,
        tau_source,
        tau_target,
        tolerance,
        maximum_iterations,
    })?;
    Ok(bind(source, target, cost, result))
}

/// Fit explicit-dustbin soft assignment, retaining its separate scientific objective.
pub fn fit_soft_assignment_csv(
    source: &[u8],
    target: &[u8],
    cost: &[u8],
    epsilon: f64,
    dustbin_cost: f64,
    tolerance: f64,
    maximum_iterations: u32,
) -> Result<TransportCsvResult<marklab_bayes::EntropicSoftAssignmentResult>, TransportCsvError> {
    let source_rows = csv::read_source(source, 2000)?;
    let target_rows = csv::read_target(target, 2000)?;
    let costs_row_major = csv::read_costs(cost, &source_rows, &target_rows)?;
    let result = marklab_bayes::entropic_soft_assignment(EntropicSoftAssignmentSpec {
        source: source_rows,
        target: target_rows,
        costs_row_major,
        epsilon,
        dustbin_cost,
        tolerance,
        maximum_iterations,
    })?;
    Ok(bind(source, target, cost, result))
}

fn partial_spec(
    source: &[u8],
    target: &[u8],
    cost: &[u8],
    transported_mass: f64,
    epsilon: f64,
    timeout_seconds: u64,
) -> Result<PartialTransportSpec, TransportCsvError> {
    let source = csv::read_source(source, 64)?;
    let target = csv::read_target(target, 64)?;
    let costs_row_major = csv::read_costs(cost, &source, &target)?;
    let spec = PartialTransportSpec {
        source,
        target,
        costs_row_major,
        transported_mass,
        epsilon,
        timeout_seconds,
    };
    spec.validate()?;
    Ok(spec)
}

/// Fit fixed-mass entropic partial transport with a cooperative scientific deadline.
/// Errors include files over 16 MiB, supports over 64, incomplete/duplicate costs, invalid
/// scientific inputs and failure to reach both feasibility and optimality within the bounds.
pub fn fit_partial_csv(
    source: &[u8],
    target: &[u8],
    cost: &[u8],
    transported_mass: f64,
    epsilon: f64,
    timeout_seconds: u64,
) -> Result<TransportCsvResult<marklab_bayes::PartialTransportFit>, TransportCsvError> {
    let spec = partial_spec(
        source,
        target,
        cost,
        transported_mass,
        epsilon,
        timeout_seconds,
    )?;
    Ok(bind(
        source,
        target,
        cost,
        marklab_bayes::fit_partial_transport(spec)?,
    ))
}

/// Execute the bounded private typed protocol; the runtime owns streams and hard deadlines.
#[doc(hidden)]
pub fn execute_partial_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, TransportCsvError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(TransportCsvError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let spec: PartialTransportSpec = crate::exact_float_json::decode(&bytes)?;
    drop(bytes);
    Ok(serde_json::to_vec(&marklab_bayes::fit_partial_transport(
        spec,
    )?)?)
}

/// Complete source-bound partial transport through the registered killable native child.
#[doc(hidden)]
pub fn run_partial_csv(
    source: &[u8],
    target: &[u8],
    cost: &[u8],
    transported_mass: f64,
    epsilon: f64,
    timeout_seconds: u64,
) -> Result<Vec<u8>, TransportCsvError> {
    let spec = partial_spec(
        source,
        target,
        cost,
        transported_mass,
        epsilon,
        timeout_seconds,
    )?;
    let request = crate::exact_float_json::encode(&spec)?.into_vec();
    drop(spec);
    let bytes = crate::run_native_partial_transport(request, timeout_seconds)?;
    // Preserve the child's f64 spelling. A Value round trip can change input/output bits.
    let result: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_slice(&bytes)?;
    for key in ["source_sha256", "target_sha256", "cost_sha256"] {
        if result.contains_key(key) {
            return Err(TransportCsvError::Input(
                "native result supplied a file identity".into(),
            ));
        }
    }
    let result = serde_json::to_vec(&bind(source, target, cost, result))?;
    if result.len() > 16 * 1024 * 1024 {
        return Err(TransportCsvError::Input(
            "native result exceeds 16 MiB".into(),
        ));
    }
    Ok(result)
}
