use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GaussianHierarchyInputIdentity, NutsSamplingSpec, StudentTHierarchySpec,
    StudentTHierarchyWorkerRequest, StudentTHierarchyWorkerResult,
};

use super::{hierarchical, publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    observation_sd_prior: f64,
    degrees_of_freedom_excess_rate: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let patients = hierarchical::read_patients(&input_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_student_t_hierarchy_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = StudentTHierarchyWorkerRequest::new(
        StudentTHierarchySpec {
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input_identity = GaussianHierarchyInputIdentity {
        path: input_path.display().to_string(),
        patients: request.patients.len(),
        observations: request.observation_count(),
        patient_data_sha256: sha256_hex(&serde_json::to_vec(&request.patients)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_student_t_hierarchy_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: StudentTHierarchyWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request, input_identity))
}
