//! Bounded late-fusion CSV application and exact native transport.
use marklab_bayes::{
    fit_late_fusion, sha256_hex, LateFusionFit, LateFusionPatient, LateFusionSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
const INPUT_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum FusionCsvError {
    #[error("late-fusion input: {0}")]
    Input(String),
    #[error("late-fusion CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("late-fusion JSON: {0}")]
    Json(#[from] serde_json::Error),
}
/// Complete source-bound native late-fusion result.
#[derive(Debug, Serialize)]
pub struct FusionCsvResult {
    pub input_sha256: String,
    #[serde(flatten)]
    pub fit: LateFusionFit,
}

/// Parse explicit OOF rows and modality_* probabilities, preserving empty-field missingness.
/// Inputs over 16 MiB/100000 rows, invalid headers/values and scientific fit failures are errors.
pub fn fit_csv(
    bytes: &[u8],
    l2_penalty: f64,
    timeout_seconds: u64,
) -> Result<FusionCsvResult, FusionCsvError> {
    if bytes.len() > INPUT_LIMIT {
        return Err(FusionCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = reader.headers()?.clone();
    if !(6..=20).contains(&headers.len())
        || !headers
            .iter()
            .take(4)
            .eq(["patient_id", "split", "base_prediction_source", "label"])
    {
        return Err(FusionCsvError::Input(
            "late-fusion CSV header differs".into(),
        ));
    }
    let modalities = headers.iter().skip(4).map(str::to_owned).collect();
    let mut patients = Vec::new();
    for row in reader.records() {
        if patients.len() >= 100000 {
            return Err(FusionCsvError::Input("CSV exceeds 100000 patients".into()));
        }
        let row = row?;
        patients.push(LateFusionPatient {
            patient_id: row[0].into(),
            split: row[1].into(),
            base_prediction_source: row[2].into(),
            label: row[3]
                .parse()
                .map_err(|_| FusionCsvError::Input("late-fusion label is invalid".into()))?,
            modality_probabilities: row
                .iter()
                .skip(4)
                .map(|v| {
                    if v.is_empty() {
                        Ok(None)
                    } else {
                        v.parse().map(Some).map_err(|_| {
                            FusionCsvError::Input("late-fusion probability is invalid".into())
                        })
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok(FusionCsvResult {
        input_sha256: sha256_hex(bytes),
        fit: fit_late_fusion(LateFusionSpec {
            patients,
            modalities,
            l2_penalty,
            timeout_seconds,
        })?,
    })
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRequest {
    csv: String,
    l2_penalty_bits: u64,
    timeout_seconds: u64,
}
/// Encode actual CSV with exact f64 penalty bits for the private native child.
#[doc(hidden)]
pub fn native_request(
    csv: Vec<u8>,
    l2_penalty: f64,
    timeout_seconds: u64,
) -> Result<Vec<u8>, FusionCsvError> {
    if csv.len() > INPUT_LIMIT {
        return Err(FusionCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let csv = String::from_utf8(csv)
        .map_err(|e| FusionCsvError::Input(format!("CSV is not UTF-8: {e}")))?;
    Ok(serde_json::to_vec(&NativeRequest {
        csv,
        l2_penalty_bits: l2_penalty.to_bits(),
        timeout_seconds,
    })?)
}
/// Execute the bounded private native protocol; the runtime owns I/O and process lifetime.
#[doc(hidden)]
pub fn execute_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, FusionCsvError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(FusionCsvError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let request: NativeRequest = serde_json::from_slice(&bytes)?;
    drop(bytes);
    let result = serde_json::to_vec(&fit_csv(
        request.csv.as_bytes(),
        f64::from_bits(request.l2_penalty_bits),
        request.timeout_seconds,
    )?)?;
    if result.len() > INPUT_LIMIT {
        return Err(FusionCsvError::Input("native result exceeds 16 MiB".into()));
    }
    Ok(result)
}
