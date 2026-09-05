use super::{embedding_spatial, publish_json, BayesCliError};
use std::path::PathBuf;

pub(super) fn run(
    input: PathBuf,
    alpha: f64,
    l2_penalty: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let csv = embedding_spatial::read(&input)?;
    let request =
        marklab::grouped_conformal::native_request(csv, alpha, l2_penalty, timeout_seconds)
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let result = marklab::run_native_grouped_conformal(request, timeout_seconds)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    // RawValue validates syntax while preserving the native serializer's exact f64 decimals.
    let result: &serde_json::value::RawValue = serde_json::from_slice(&result)?;
    publish_json(&out, &result)
}
