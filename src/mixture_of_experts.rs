//! Bounded context-gated mixture CSV application and native transport.
use marklab_bayes::{
    fit_mixture_of_experts, MixtureOfExpertsFit, MixtureOfExpertsPatient, MixtureOfExpertsSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const INPUT_LIMIT: usize = 16 * 1024 * 1024;
const NATIVE_LIMIT: usize = 128 * 1024 * 1024;

/// CSV admission, scientific fit, or private native protocol failure.
#[derive(Debug, Error)]
pub enum MixtureOfExpertsCsvError {
    #[error("mixture-of-experts input: {0}")]
    Input(String),
    #[error("mixture-of-experts CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Fit(#[from] marklab_bayes::BayesError),
    #[error("mixture-of-experts JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Source identity and complete native scientific result.
#[derive(Debug, Serialize)]
pub struct MixtureOfExpertsCsvResult {
    pub input_sha256: String,
    #[serde(flatten)]
    pub fit: MixtureOfExpertsFit,
}

/// Parse a strict CSV and run the complete context-gated mixture workflow.
///
/// Rejects input above 16 MiB, more than 10,000 rows, header dimensions outside the scientific
/// bounds, invalid decimal fields, and every scientific admission/nonconvergence condition. Input
/// column order is retained while patient order is canonicalized by the scientific owner.
pub fn fit_csv(
    bytes: &[u8],
    l2_penalty: f64,
    entropy_regularization: f64,
    ood_validation_quantile: f64,
    timeout_seconds: u64,
) -> Result<MixtureOfExpertsCsvResult, MixtureOfExpertsCsvError> {
    if bytes.len() > INPUT_LIMIT {
        return Err(MixtureOfExpertsCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    let headers = reader.headers()?.clone();
    if !(7..=28).contains(&headers.len())
        || !headers
            .iter()
            .take(4)
            .eq(["patient_id", "split", "expert_prediction_source", "label"])
    {
        return Err(MixtureOfExpertsCsvError::Input("CSV header differs".into()));
    }
    let context_end = headers
        .iter()
        .enumerate()
        .skip(4)
        .take_while(|(_, name)| name.starts_with("context_"))
        .last()
        .map_or(4, |(index, _)| index + 1);
    let context_names = headers
        .iter()
        .skip(4)
        .take(context_end - 4)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let expert_names = headers
        .iter()
        .skip(context_end)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut patients = Vec::new();
    for row in reader.records() {
        if patients.len() >= 10_000 {
            return Err(MixtureOfExpertsCsvError::Input(
                "CSV exceeds 10,000 patients".into(),
            ));
        }
        let row = row?;
        patients.push(MixtureOfExpertsPatient {
            patient_id: row[0].into(),
            split: row[1].into(),
            expert_prediction_source: row[2].into(),
            label: row[3]
                .parse()
                .map_err(|_| MixtureOfExpertsCsvError::Input("label is invalid".into()))?,
            context: row
                .iter()
                .skip(4)
                .take(context_end - 4)
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| MixtureOfExpertsCsvError::Input("context is invalid".into()))
                })
                .collect::<Result<Vec<_>, _>>()?,
            expert_probabilities: row
                .iter()
                .skip(context_end)
                .map(|value| {
                    if value.is_empty() {
                        Ok(None)
                    } else {
                        value.parse().map(Some).map_err(|_| {
                            MixtureOfExpertsCsvError::Input("expert probability is invalid".into())
                        })
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    let result = MixtureOfExpertsCsvResult {
        input_sha256: marklab_bayes::sha256_hex(bytes),
        fit: fit_mixture_of_experts(MixtureOfExpertsSpec {
            patients,
            context_names,
            expert_names,
            l2_penalty,
            entropy_regularization,
            ood_validation_quantile,
            timeout_seconds,
        })?,
    };
    if serde_json::to_vec(&result)?.len() > INPUT_LIMIT {
        return Err(MixtureOfExpertsCsvError::Input(
            "native result exceeds 16 MiB".into(),
        ));
    }
    Ok(result)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRequest {
    csv: String,
    l2_penalty_bits: u64,
    entropy_regularization_bits: u64,
    ood_validation_quantile_bits: u64,
    timeout_seconds: u64,
}

/// Encode the original CSV and exact binary controls for the native child.
#[doc(hidden)]
pub fn native_request(
    csv: Vec<u8>,
    l2_penalty: f64,
    entropy_regularization: f64,
    ood_validation_quantile: f64,
    timeout_seconds: u64,
) -> Result<Vec<u8>, MixtureOfExpertsCsvError> {
    if csv.len() > INPUT_LIMIT {
        return Err(MixtureOfExpertsCsvError::Input("CSV exceeds 16 MiB".into()));
    }
    let csv = String::from_utf8(csv)
        .map_err(|error| MixtureOfExpertsCsvError::Input(format!("CSV is not UTF-8: {error}")))?;
    Ok(serde_json::to_vec(&NativeRequest {
        csv,
        l2_penalty_bits: l2_penalty.to_bits(),
        entropy_regularization_bits: entropy_regularization.to_bits(),
        ood_validation_quantile_bits: ood_validation_quantile.to_bits(),
        timeout_seconds,
    })?)
}

/// Execute the private native request; runtime owns streams and process lifetime.
#[doc(hidden)]
pub fn execute_native_request(bytes: Vec<u8>) -> Result<Vec<u8>, MixtureOfExpertsCsvError> {
    if bytes.len() > NATIVE_LIMIT {
        return Err(MixtureOfExpertsCsvError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let request: NativeRequest = serde_json::from_slice(&bytes)?;
    drop(bytes);
    let result = serde_json::to_vec(&fit_csv(
        request.csv.as_bytes(),
        f64::from_bits(request.l2_penalty_bits),
        f64::from_bits(request.entropy_regularization_bits),
        f64::from_bits(request.ood_validation_quantile_bits),
        request.timeout_seconds,
    )?)?;
    if result.len() > INPUT_LIMIT {
        return Err(MixtureOfExpertsCsvError::Input(
            "native result exceeds 16 MiB".into(),
        ));
    }
    Ok(result)
}
