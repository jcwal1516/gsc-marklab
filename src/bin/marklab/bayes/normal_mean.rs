use std::{
    fs,
    path::{Path, PathBuf},
};

use marklab_bayes::{
    sha256_hex, NormalMeanInputIdentity, NormalMeanSpec, NormalMeanWorkerRequest, NutsSamplingSpec,
    WorkerResult,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn run_normal_mean(
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = prepare_normal_mean(
        input_path,
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let worker_result = execute_normal_mean(&prepared)?;
    let fit = worker_result.into_fit(prepared.request, prepared.input_identity);
    publish_json(&output_path, &fit)
}

pub(crate) struct PreparedNormalMean {
    pub(crate) request: NormalMeanWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: NormalMeanInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare_normal_mean(
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedNormalMean, BayesCliError> {
    let observations = read_observations(&input_path)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path.clone(),
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NormalMeanWorkerRequest::new(
        &NormalMeanSpec {
            prior_mean,
            prior_sd,
            known_sigma,
            observations: observations.clone(),
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedNormalMean {
        request,
        request_bytes,
        request_sha256,
        input_identity: NormalMeanInputIdentity {
            path: input_path.display().to_string(),
            observation_count: observations.len(),
            observations_sha256: observations_digest(&observations),
        },
        timeout_seconds,
    })
}

pub(crate) fn execute_normal_mean(
    prepared: &PreparedNormalMean,
) -> Result<WorkerResult, BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let worker_result: WorkerResult = serde_json::from_slice(&result_bytes)?;
    worker_result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(worker_result)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation: f64,
}

pub(super) fn read_observations(path: &Path) -> Result<Vec<f64>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(BayesCliError::Input(format!(
            "input must be a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"
        )));
    }
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["observation"]) {
        return Err(BayesCliError::Input(
            "normal-mean CSV headers must be exactly: observation".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        observations.push(row?.observation);
        if observations.len() > 100_000 {
            return Err(BayesCliError::Input(
                "observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(observations)
}

pub(super) fn observations_digest(observations: &[f64]) -> String {
    let mut canonical = b"marklab-normal-mean-observations-v1\0".to_vec();
    canonical.extend_from_slice(&(observations.len() as u64).to_be_bytes());
    for observation in observations {
        canonical.extend_from_slice(&observation.to_bits().to_be_bytes());
    }
    sha256_hex(&canonical)
}
