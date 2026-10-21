use marklab_bayes::{
    json_sha256, sha256_hex, GaussianHierarchyInputIdentity, StudentTHierarchySpec,
    StudentTHierarchyWorkerRequest, StudentTHierarchyWorkerResult,
};

use super::cli_schema::{StudentTHierarchyArgs, StudentTHierarchyModelArgs};
use super::worker_identity;
use super::{hierarchical, publish_json, run_worker, CliError};

pub(super) fn run(args: StudentTHierarchyArgs) -> Result<(), CliError> {
    let StudentTHierarchyArgs { model, out } = args;
    let output_path = out;
    let prepared = prepare(model)?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedStudentTHierarchy {
    pub(crate) request: StudentTHierarchyWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: GaussianHierarchyInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: StudentTHierarchyModelArgs,
) -> Result<PreparedStudentTHierarchy, CliError> {
    let StudentTHierarchyModelArgs {
        input: input_path,
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        observation_sd_prior,
        degrees_of_freedom_excess_rate,
        nuts,
        timeout_seconds,
    } = model;
    let sampling = nuts.spec();
    let patients = hierarchical::read_patients(&input_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_student_t_hierarchy_worker.py")?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input_identity = GaussianHierarchyInputIdentity {
        path: input_path.display().to_string(),
        patients: request.patients.len(),
        observations: request.observation_count(),
        patient_data_sha256: json_sha256(&request.patients)?,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedStudentTHierarchy {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedStudentTHierarchy,
) -> Result<StudentTHierarchyWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_student_t_hierarchy_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: StudentTHierarchyWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
