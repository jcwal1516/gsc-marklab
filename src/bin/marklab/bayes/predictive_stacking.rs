use super::{embedding_spatial, publish_json, BayesCliError};
use std::path::PathBuf;

pub(super) fn run(input: PathBuf, timeout_seconds: u64, out: PathBuf) -> Result<(), BayesCliError> {
    let bytes = embedding_spatial::read(&input)?;
    let request = marklab::predictive_stacking::native_request(bytes, timeout_seconds)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let bytes = marklab::run_native_predictive_stacking(request, timeout_seconds)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let result: &serde_json::value::RawValue = serde_json::from_slice(&bytes)?;
    publish_json(&out, &result)
}
