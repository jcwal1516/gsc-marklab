use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest,
    NumpyroBetaBinomialGroupGenderSlideSbcWorkerResult, NutsSamplingSpec,
};

use super::{beta_binomial_group_gender_slide_hierarchy, publish_json, run_worker, BayesCliError};

const WORKER_IDENTITY_PREFIX: &[u8] =
    b"marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_identity.v1\0";

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
    patient_log_odds_sd_prior_sd: f64,
    slide_concentration_prior_sd: f64,
    replicates: u32,
    sampling: NutsSamplingSpec,
    minimum_rank_uniformity_p_value: f64,
    minimum_coverage_90: f64,
    maximum_coverage_90: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = beta_binomial_group_gender_slide_hierarchy::prepare(
        input_path,
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        patient_log_odds_sd_prior_sd,
        slide_concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory
        .join("marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let model_worker_path = worker_directory
        .join("marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_worker.py");
    let model_worker_bytes = fs::read(&model_worker_path).map_err(|source| BayesCliError::Io {
        path: model_worker_path,
        source,
    })?;
    let mut worker_identity = WORKER_IDENTITY_PREFIX.to_vec();
    worker_identity.extend_from_slice(&worker_bytes);
    worker_identity.push(0);
    worker_identity.extend_from_slice(&model_worker_bytes);
    let request = NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest::new(
        prepared.request,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_identity),
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroBetaBinomialGroupGenderSlideSbcWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request))
}
