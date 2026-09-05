//! Bounded patient-held-out stacking CSV application and native transport.
use marklab_bayes::{
    fit_predictive_stacking, PredictiveStackingFit, PredictiveStackingPatient,
    PredictiveStackingSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
const INPUT_LIMIT: usize = 16 * 1024 * 1024;
/// CSV admission, scientific or private protocol failure.
#[derive(Debug, Error)]
pub enum StackingCsvError {
    #[error("predictive-stacking input: {0}")]
    Input(String),
    #[error("predictive-stacking CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("predictive-stacking JSON: {0}")]
    Json(#[from] serde_json::Error),
}
/// Source identity and the complete native scientific output.
#[derive(Debug, Serialize)]
pub struct StackingCsvResult {
    pub input_sha256: String,
    #[serde(flatten)]
    pub fit: PredictiveStackingFit,
}
/// Fit all patient-held-out rows and every leave-one-patient subset from strict CSV.
/// Rejects files over 16 MiB, more than 500 rows/16 models, invalid headers/declarations and
/// scientific nonconvergence. Model columns retain order; scientific patient order is canonical.
pub fn fit_csv(bytes: &[u8], timeout_seconds: u64) -> Result<StackingCsvResult, StackingCsvError> {
    if bytes.len() > INPUT_LIMIT {
        return Err(StackingCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = reader.headers()?.clone();
    if !(4..=18).contains(&headers.len())
        || !headers.iter().take(2).eq(["patient_id", "held_out_unit"])
    {
        return Err(StackingCsvError::Input(
            "predictive-stacking CSV header differs".into(),
        ));
    }
    let model_names = headers.iter().skip(2).map(str::to_owned).collect();
    let mut patients = Vec::new();
    for row in reader.records() {
        if patients.len() >= 500 {
            return Err(StackingCsvError::Input("CSV exceeds 500 patients".into()));
        }
        let row = row?;
        patients.push(PredictiveStackingPatient {
            patient_id: row[0].into(),
            held_out_unit: row[1].into(),
            log_predictive_densities: row
                .iter()
                .skip(2)
                .map(|v| {
                    v.parse().map_err(|_| {
                        StackingCsvError::Input("predictive-stacking log density is invalid".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok(StackingCsvResult {
        input_sha256: marklab_bayes::sha256_hex(bytes),
        fit: fit_predictive_stacking(PredictiveStackingSpec {
            patients,
            model_names,
            timeout_seconds,
        })?,
    })
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRequest {
    csv: String,
    timeout_seconds: u64,
}
/// Encode original CSV bytes, preserving all decimal input spelling across the native boundary.
#[doc(hidden)]
pub fn native_request(csv: Vec<u8>, timeout_seconds: u64) -> Result<Vec<u8>, StackingCsvError> {
    if csv.len() > INPUT_LIMIT {
        return Err(StackingCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let csv = String::from_utf8(csv)
        .map_err(|e| StackingCsvError::Input(format!("CSV is not UTF-8: {e}")))?;
    Ok(serde_json::to_vec(&NativeRequest {
        csv,
        timeout_seconds,
    })?)
}
/// Execute the bounded native protocol; the runtime owns streams and process lifetime.
#[doc(hidden)]
pub fn execute_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, StackingCsvError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(StackingCsvError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let request: NativeRequest = serde_json::from_slice(&bytes)?;
    drop(bytes);
    let result = serde_json::to_vec(&fit_csv(request.csv.as_bytes(), request.timeout_seconds)?)?;
    if result.len() > INPUT_LIMIT {
        return Err(StackingCsvError::Input(
            "native result exceeds 16 MiB".into(),
        ));
    }
    Ok(result)
}
