use std::path::PathBuf;

use super::{embedding_spatial, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input: PathBuf,
    l2_penalty: f64,
    entropy_regularization: f64,
    ood_validation_quantile: f64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let bytes = embedding_spatial::read(&input)?;
    let request = marklab::mixture_of_experts::native_request(
        bytes,
        l2_penalty,
        entropy_regularization,
        ood_validation_quantile,
        timeout_seconds,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let bytes = marklab::run_native_mixture_of_experts(request, timeout_seconds)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let result: &serde_json::value::RawValue = serde_json::from_slice(&bytes)?;
    publish_json(&out, &result)
}
