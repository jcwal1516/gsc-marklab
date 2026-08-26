use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GaussianHierarchyInputIdentity, GaussianHierarchySpec,
    GaussianHierarchyWorkerRequest, HierarchicalPatientData, HierarchicalWorkerResult,
    NutsSamplingSpec,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    patient_id: String,
    observation: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let patients = read_patients(&input_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_hierarchical_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = GaussianHierarchyWorkerRequest::new(
        GaussianHierarchySpec {
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let patient_bytes = serde_json::to_vec(&request.patients)?;
    let input_identity = GaussianHierarchyInputIdentity {
        path: input_path.display().to_string(),
        patients: request.patients.len(),
        observations: request.observation_count(),
        patient_data_sha256: sha256_hex(&patient_bytes),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_hierarchical_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let worker_result: HierarchicalWorkerResult = serde_json::from_slice(&result_bytes)?;
    worker_result.validate(&request, &request_sha256)?;
    publish_json(
        &output_path,
        &worker_result.into_fit(request, input_identity),
    )
}

fn read_patients(path: &std::path::Path) -> Result<Vec<HierarchicalPatientData>, BayesCliError> {
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
    if !reader.headers()?.iter().eq(["patient_id", "observation"]) {
        return Err(BayesCliError::Input(
            "hierarchical-normal CSV headers must be exactly: patient_id,observation".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, Vec<f64>>::new();
    let mut count = 0_usize;
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        grouped
            .entry(row.patient_id)
            .or_default()
            .push(row.observation);
        count += 1;
        if count > 100_000 {
            return Err(BayesCliError::Input(
                "observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(grouped
        .into_iter()
        .map(|(patient_id, observations)| HierarchicalPatientData {
            patient_id,
            observations,
        })
        .collect())
}
