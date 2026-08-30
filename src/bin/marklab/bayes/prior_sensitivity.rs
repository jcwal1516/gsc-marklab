use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NormalMeanPriorSensitivitySpec, NormalPriorAlternative,
    PriorSensitivityInputIdentity, PriorSensitivityWorkerRequest, PriorSensitivityWorkerResult,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    priors_path: PathBuf,
    base_prior: String,
    known_sigma: f64,
    decision_threshold: f64,
    decision_probability_threshold: f64,
    material_mean_shift: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&input_path)?;
    let priors = read_priors(&priors_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_scipy_prior_sensitivity_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PriorSensitivityWorkerRequest::new(
        NormalMeanPriorSensitivitySpec {
            observations,
            priors,
            base_prior,
            known_sigma,
            decision_threshold,
            decision_probability_threshold,
            material_mean_shift,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input = PriorSensitivityInputIdentity {
        observations_path: input_path.display().to_string(),
        priors_path: priors_path.display().to_string(),
        observations_sha256: sha256_hex(&serde_json::to_vec(&request.observations)?),
        priors_sha256: sha256_hex(&serde_json::to_vec(&request.priors)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_scipy_prior_sensitivity_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: PriorSensitivityWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request, input))
}

fn reader(path: &std::path::Path) -> Result<csv::Reader<std::fs::File>, BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "prior-sensitivity inputs must be regular files within the 16 MiB limit",
    )?;
    Ok(csv::ReaderBuilder::new().from_path(path)?)
}

fn read_observations(path: &std::path::Path) -> Result<Vec<f64>, BayesCliError> {
    let mut reader = reader(path)?;
    if !reader.headers()?.iter().eq(["observation"]) {
        return Err(BayesCliError::Input(
            "prior-sensitivity observation header must be exactly: observation".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        observations.push(row?.observation);
        if observations.len() > 100_000 {
            return Err(BayesCliError::Input(
                "prior-sensitivity observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(observations)
}

fn read_priors(path: &std::path::Path) -> Result<Vec<NormalPriorAlternative>, BayesCliError> {
    let mut reader = reader(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["prior_name", "prior_mean", "prior_sd"])
    {
        return Err(BayesCliError::Input(
            "prior-sensitivity prior headers must be exactly: prior_name,prior_mean,prior_sd"
                .into(),
        ));
    }
    let mut priors = Vec::new();
    for row in reader.deserialize::<NormalPriorAlternative>() {
        priors.push(row?);
        if priors.len() > 32 {
            return Err(BayesCliError::Input(
                "prior-sensitivity prior count exceeds 32".into(),
            ));
        }
    }
    Ok(priors)
}
