use std::{fs, path::PathBuf};

use marklab_bayes::{
    patient_data_sha256, sha256_hex, BetaBinomialHierarchyInputIdentity, BetaBinomialHierarchySpec,
    BetaBinomialHierarchyWorkerRequest, BetaBinomialHierarchyWorkerResult, BetaBinomialPatientData,
    NutsSamplingSpec,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    successes: u64,
    trials: u64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    population_alpha: f64,
    population_beta: f64,
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
            "beta-binomial patient input exceeds 16 MiB".into(),
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
            row.map(|row| BetaBinomialPatientData {
                patient_id: row.patient_id,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            BayesCliError::Input(format!("invalid beta-binomial patient CSV: {error}"))
        })?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_beta_binomial_hierarchy_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = BetaBinomialHierarchyWorkerRequest::new(
        BetaBinomialHierarchySpec {
            population_alpha,
            population_beta,
            concentration_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input_identity = BetaBinomialHierarchyInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: patient_data_sha256(&request.patients)?,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_beta_binomial_hierarchy_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: BetaBinomialHierarchyWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request, input_identity))
}
