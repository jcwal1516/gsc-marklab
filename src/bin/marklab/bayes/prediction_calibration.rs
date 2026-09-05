use super::{embedding_spatial, publish_json, BayesCliError};
use std::path::PathBuf;

pub(super) fn run(
    input: PathBuf,
    method: String,
    bins: u32,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    if method != "platt-logistic" {
        return Err(BayesCliError::Input(
            "calibration method must be platt-logistic".into(),
        ));
    }
    let bytes = embedding_spatial::read(&input)?;
    let request = marklab::prediction_calibration::native_request(bytes, bins, timeout_seconds)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let bytes = marklab::run_native_prediction_calibration(request, timeout_seconds)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let result: &serde_json::value::RawValue = serde_json::from_slice(&bytes)?;
    publish_json(&out, &result)
}
