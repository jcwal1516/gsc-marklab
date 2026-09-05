//! Bounded JSON application and private native transport for paired Gaussian pCCA EM.

use marklab_bayes::{fit_pcca_em, PccaEmFit, PccaEmSpec};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const INPUT_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PccaJsonError {
    #[error("pCCA input: {0}")]
    Input(String),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("pCCA JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parse and fit one bounded pCCA JSON specification.
pub fn fit_json(bytes: &[u8]) -> Result<PccaEmFit, PccaJsonError> {
    if bytes.len() > INPUT_LIMIT {
        return Err(PccaJsonError::Input("JSON exceeds 16 MiB".into()));
    }
    let spec: PccaEmSpec = serde_json::from_slice(bytes)?;
    Ok(fit_pcca_em(spec)?)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRequest {
    json: String,
}

/// Preserve original JSON bytes and source identity across the killable native child boundary.
#[doc(hidden)]
pub fn native_request(json: Vec<u8>) -> Result<(Vec<u8>, u64), PccaJsonError> {
    if json.len() > INPUT_LIMIT {
        return Err(PccaJsonError::Input("JSON exceeds 16 MiB".into()));
    }
    let timeout_seconds = serde_json::from_slice::<PccaEmSpec>(&json)?.timeout_seconds;
    let json = String::from_utf8(json)
        .map_err(|error| PccaJsonError::Input(format!("JSON is not UTF-8: {error}")))?;
    Ok((
        serde_json::to_vec(&NativeRequest { json })?,
        timeout_seconds,
    ))
}

/// Decode and execute the private protocol; the shared runtime owns streams and process lifetime.
#[doc(hidden)]
pub fn execute_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, PccaJsonError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(PccaJsonError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let request: NativeRequest = serde_json::from_slice(&bytes)?;
    drop(bytes);
    let result = serde_json::to_vec(&fit_json(request.json.as_bytes())?)?;
    if result.len() > INPUT_LIMIT {
        return Err(PccaJsonError::Input("native result exceeds 16 MiB".into()));
    }
    Ok(result)
}
