use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, BetaBinomialGroupAgreementPolicy, BetaBinomialGroupAgreementResult,
    NumpyroBetaBinomialGroupRegressionWorkerRequest,
    NumpyroBetaBinomialGroupRegressionWorkerResult, NutsSamplingSpec,
};

use super::{beta_binomial_group_regression, publish_json, run_worker, BayesCliError};

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
    maximum_standardized_difference: f64,
    minimum_probability_tolerance: f64,
    minimum_log_odds_tolerance: f64,
    minimum_concentration_tolerance: f64,
    minimum_patient_tolerance: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = beta_binomial_group_regression::prepare(
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
    let pymc = beta_binomial_group_regression::execute(&prepared)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path =
        worker_directory.join("marklab_numpyro_beta_binomial_group_regression_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let numpyro_request = NumpyroBetaBinomialGroupRegressionWorkerRequest::new(
        prepared.request.clone(),
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let request_bytes = serde_json::to_vec(&numpyro_request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_numpyro_beta_binomial_group_regression_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let numpyro: NumpyroBetaBinomialGroupRegressionWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    let numpyro = numpyro.into_validated_payload(&numpyro_request, &request_sha256)?;
    let result = BetaBinomialGroupAgreementResult::new(
        prepared.request,
        prepared.input_identity,
        pymc,
        numpyro,
        BetaBinomialGroupAgreementPolicy {
            maximum_standardized_difference,
            minimum_probability_tolerance,
            minimum_log_odds_tolerance,
            minimum_concentration_tolerance,
            minimum_patient_tolerance,
        },
    )?;
    publish_json(&output_path, &result)
}
