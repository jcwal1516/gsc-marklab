use std::{fs, path::PathBuf};

use marklab_bayes::{
    beta_binomial_group_data_sha256, sha256_hex, BetaBinomialGroupPatientData,
    BetaBinomialGroupRegressionInputIdentity, BetaBinomialGroupRegressionSpec,
    BetaBinomialGroupRegressionWorkerRequest, BetaBinomialGroupRegressionWorkerResult,
    NutsSamplingSpec,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    group: String,
    successes: u64,
    trials: u64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = prepare(
        input_path,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedBetaBinomialGroupRegression {
    pub(crate) request: BetaBinomialGroupRegressionWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: BetaBinomialGroupRegressionInputIdentity,
    pub(crate) timeout_seconds: u64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedBetaBinomialGroupRegression, BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "beta-binomial group input exceeds 16 MiB".into(),
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
            row.map(|row| BetaBinomialGroupPatientData {
                patient_id: row.patient_id,
                group: row.group,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            BayesCliError::Input(format!("invalid beta-binomial group patient CSV: {error}"))
        })?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path =
        worker_directory.join("marklab_pymc_beta_binomial_group_regression_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = BetaBinomialGroupRegressionWorkerRequest::new(
        BetaBinomialGroupRegressionSpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input_identity = BetaBinomialGroupRegressionInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: beta_binomial_group_data_sha256(&request.patients)?,
        reference_patients: request
            .patients
            .iter()
            .filter(|patient| patient.group == reference_group)
            .count(),
        comparison_patients: request
            .patients
            .iter()
            .filter(|patient| patient.group == comparison_group)
            .count(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedBetaBinomialGroupRegression {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedBetaBinomialGroupRegression,
) -> Result<BetaBinomialGroupRegressionWorkerResult, BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_beta_binomial_group_regression_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: BetaBinomialGroupRegressionWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
