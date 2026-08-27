use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_bayes::{
    beta_binomial_group_gender_data_sha256, sha256_hex, BetaBinomialGroupGenderPatientData,
    BetaBinomialGroupGenderRegressionInputIdentity, BetaBinomialGroupGenderRegressionSpec,
    BetaBinomialGroupGenderRegressionWorkerRequest, BetaBinomialGroupGenderRegressionWorkerResult,
    NutsSamplingSpec,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    group: String,
    gender: String,
    successes: u64,
    trials: u64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    reference_gender: String,
    comparison_gender: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    gender_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "beta-binomial group/gender input exceeds 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let patients = reader
        .deserialize::<PatientRow>()
        .map(|row| {
            row.map(|row| BetaBinomialGroupGenderPatientData {
                patient_id: row.patient_id,
                group: row.group,
                gender: row.gender,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            BayesCliError::Input(format!(
                "invalid beta-binomial group/gender patient CSV: {error}"
            ))
        })?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path =
        worker_directory.join("marklab_pymc_beta_binomial_group_gender_regression_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = BetaBinomialGroupGenderRegressionWorkerRequest::new(
        BetaBinomialGroupGenderRegressionSpec {
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            concentration_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let mut design_cell_counts = BTreeMap::new();
    for patient in &request.patients {
        *design_cell_counts
            .entry(format!("{}:{}", patient.group, patient.gender))
            .or_insert(0) += 1;
    }
    let input_identity = BetaBinomialGroupGenderRegressionInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: beta_binomial_group_gender_data_sha256(&request.patients)?,
        design_cell_counts,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_beta_binomial_group_gender_regression_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: BetaBinomialGroupGenderRegressionWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request, input_identity))
}
