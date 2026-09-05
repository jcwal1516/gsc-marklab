use super::{embedding_spatial, publish_json, BayesCliError};
use std::path::PathBuf;

pub(super) fn run(
    input: PathBuf,
    l2_penalty: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let bytes = embedding_spatial::read(&input)?;
    let request = marklab::late_fusion::native_request(bytes, l2_penalty, timeout_seconds)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let bytes = marklab::run_native_late_fusion(request, timeout_seconds)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let result: &serde_json::value::RawValue = serde_json::from_slice(&bytes)?;
    publish_json(&out, &result)
}
