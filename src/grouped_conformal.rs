//! Bounded grouped-conformal CSV application and its private native transport.
use marklab_bayes::{
    fit_grouped_conformal, sha256_hex, GroupedConformalFit, GroupedConformalPatient,
    GroupedConformalSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const INPUT_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ConformalCsvError {
    #[error("grouped conformal input: {0}")]
    Input(String),
    #[error("grouped conformal CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("grouped conformal JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Source-bound version-2 native result; scientific fields retain their domain owner.
#[derive(Debug, Serialize)]
pub struct ConformalCsvResult {
    pub input_sha256: String,
    #[serde(flatten)]
    pub fit: GroupedConformalFit,
}

/// Fit a bounded CSV using the existing exact header, patient and scientific contract.
/// Numeric CSV values are parsed directly as f64, without intermediate JSON rounding.
/// Input must be at most 16 MiB; malformed rows and all domain/optimizer failures are errors.
pub fn fit_csv(
    input_bytes: &[u8],
    alpha: f64,
    l2_penalty: f64,
    timeout_seconds: u64,
) -> Result<ConformalCsvResult, ConformalCsvError> {
    if input_bytes.len() > INPUT_LIMIT {
        return Err(ConformalCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 7
        || headers.len() > 133
        || headers.iter().take(5).collect::<Vec<_>>()
            != ["patient_id", "split", "site", "subgroup", "label"]
    {
        return Err(ConformalCsvError::Input(
            "grouped conformal CSV header differs".into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(5)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut patients = Vec::new();
    for row in reader.records() {
        if patients.len() >= 100_000 {
            return Err(ConformalCsvError::Input(
                "CSV exceeds 100000 patients".into(),
            ));
        }
        let row = row?;
        patients.push(GroupedConformalPatient {
            patient_id: row[0].into(),
            split: row[1].into(),
            site: row[2].into(),
            subgroup: row[3].into(),
            label: row[4]
                .parse()
                .map_err(|_| ConformalCsvError::Input("conformal label is invalid".into()))?,
            features: row
                .iter()
                .skip(5)
                .map(|value| {
                    value.parse().map_err(|_| {
                        ConformalCsvError::Input("conformal feature is invalid".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    let fit = fit_grouped_conformal(GroupedConformalSpec {
        patients,
        feature_names,
        alpha,
        l2_penalty,
        timeout_seconds,
    })?;
    Ok(ConformalCsvResult {
        input_sha256: sha256_hex(input_bytes),
        fit,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRequest {
    csv: String,
    alpha_bits: u64,
    l2_penalty_bits: u64,
    timeout_seconds: u64,
}

/// Private native CLI protocol: original CSV plus exact f64 control bits.
#[doc(hidden)]
pub fn native_request(
    csv: Vec<u8>,
    alpha: f64,
    l2_penalty: f64,
    timeout_seconds: u64,
) -> Result<Vec<u8>, ConformalCsvError> {
    if csv.len() > INPUT_LIMIT {
        return Err(ConformalCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let csv = String::from_utf8(csv)
        .map_err(|error| ConformalCsvError::Input(format!("CSV is not UTF-8: {error}")))?;
    Ok(serde_json::to_vec(&NativeRequest {
        csv,
        alpha_bits: alpha.to_bits(),
        l2_penalty_bits: l2_penalty.to_bits(),
        timeout_seconds,
    })?)
}

/// Decode and execute the private native protocol; caller owns streams and process lifetime.
#[doc(hidden)]
pub fn execute_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, ConformalCsvError> {
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(ConformalCsvError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let request: NativeRequest = serde_json::from_slice(&bytes)?;
    // NativeRequest owns its CSV; release the escaped wire buffer before allocating fit storage.
    drop(bytes);
    let result = fit_csv(
        request.csv.as_bytes(),
        f64::from_bits(request.alpha_bits),
        f64::from_bits(request.l2_penalty_bits),
        request.timeout_seconds,
    )?;
    let result = serde_json::to_vec(&result)?;
    if result.len() > 16 * 1024 * 1024 {
        return Err(ConformalCsvError::Input(
            "native result exceeds 16 MiB".into(),
        ));
    }
    Ok(result)
}
