//! Bounded patient-OOF calibration CSV application and native transport.
use marklab_bayes::{
    fit_prediction_calibration, sha256_hex, PredictionCalibrationFit, PredictionCalibrationRow,
    PredictionCalibrationSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
const INPUT_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum CalibrationCsvError {
    #[error("calibration input: {0}")]
    Input(String),
    #[error("calibration CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("calibration JSON: {0}")]
    Json(#[from] serde_json::Error),
}
/// Native result bound to the exact CSV bytes admitted by this application.
#[derive(Debug, Serialize)]
pub struct CalibrationCsvResult {
    pub input_sha256: String,
    #[serde(flatten)]
    pub fit: PredictionCalibrationFit,
}
/// Parse the exact patient_id,split,score,label header and fit within the scientific limits.
/// Malformed UTF-8/CSV, input over 16 MiB, rows over 100000, and scientific failures are errors.
pub fn fit_csv(
    bytes: &[u8],
    bins: u32,
    timeout_seconds: u64,
) -> Result<CalibrationCsvResult, CalibrationCsvError> {
    if bytes.len() > INPUT_LIMIT {
        return Err(CalibrationCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader
        .headers()?
        .iter()
        .eq(["patient_id", "split", "score", "label"])
    {
        return Err(CalibrationCsvError::Input(
            "calibration CSV header differs".into(),
        ));
    }
    let headers = reader.headers()?.clone();
    let mut rows = Vec::new();
    for record in reader.records() {
        if rows.len() >= 100000 {
            return Err(CalibrationCsvError::Input(
                "CSV exceeds 100000 patients".into(),
            ));
        }
        let record = record?;
        rows.push(record.deserialize::<PredictionCalibrationRow>(Some(&headers))?);
    }
    Ok(CalibrationCsvResult {
        input_sha256: sha256_hex(bytes),
        fit: fit_prediction_calibration(PredictionCalibrationSpec {
            rows,
            bins,
            timeout_seconds,
        })?,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRequest {
    csv: String,
    bins: u32,
    timeout_seconds: u64,
}

/// Encode original CSV without an intermediate floating-point JSON conversion.
#[doc(hidden)]
pub fn native_request(
    csv: Vec<u8>,
    bins: u32,
    timeout_seconds: u64,
) -> Result<Vec<u8>, CalibrationCsvError> {
    if csv.len() > INPUT_LIMIT {
        return Err(CalibrationCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let csv = String::from_utf8(csv)
        .map_err(|e| CalibrationCsvError::Input(format!("CSV is not UTF-8: {e}")))?;
    Ok(serde_json::to_vec(&NativeRequest {
        csv,
        bins,
        timeout_seconds,
    })?)
}

/// Decode bounded private transport and return a bounded source-identified result.
#[doc(hidden)]
pub fn execute_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, CalibrationCsvError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(CalibrationCsvError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let request: NativeRequest = serde_json::from_slice(&bytes)?;
    drop(bytes);
    let result = serde_json::to_vec(&fit_csv(
        request.csv.as_bytes(),
        request.bins,
        request.timeout_seconds,
    )?)?;
    if result.len() > INPUT_LIMIT {
        return Err(CalibrationCsvError::Input(
            "native result exceeds 16 MiB".into(),
        ));
    }
    Ok(result)
}
