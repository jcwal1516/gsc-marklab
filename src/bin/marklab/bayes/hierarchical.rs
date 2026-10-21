use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GaussianHierarchyInputIdentity, GaussianHierarchySpec,
    GaussianHierarchyWorkerRequest, HierarchicalPatientData, HierarchicalWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::{HierarchicalNormalArgs, HierarchicalNormalModelArgs};
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    patient_id: String,
    observation: f64,
}

pub(super) fn run(args: HierarchicalNormalArgs) -> Result<(), CliError> {
    let HierarchicalNormalArgs { model, input, out } = args;
    let input_path = input;
    let output_path = out;
    let prepared = prepare(input_path, model)?;
    let worker_result = execute(&prepared)?;
    publish_json(
        &output_path,
        &worker_result.into_fit(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedGaussianHierarchy {
    pub(crate) request: GaussianHierarchyWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: GaussianHierarchyInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    input_path: PathBuf,
    model: HierarchicalNormalModelArgs,
) -> Result<PreparedGaussianHierarchy, CliError> {
    let HierarchicalNormalModelArgs {
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        known_sigma,
        nuts,
        timeout_seconds,
    } = model;
    let sampling = nuts.spec();
    let patients = read_patients(&input_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_hierarchical_worker.py")?;
    let request = GaussianHierarchyWorkerRequest::new(
        GaussianHierarchySpec {
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            patients,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
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
    Ok(PreparedGaussianHierarchy {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedGaussianHierarchy,
) -> Result<HierarchicalWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_hierarchical_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let worker_result: HierarchicalWorkerResult = serde_json::from_slice(&result_bytes)?;
    worker_result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(worker_result)
}

pub(crate) fn read_patients(
    path: &std::path::Path,
) -> Result<Vec<HierarchicalPatientData>, CliError> {
    let metadata = fs::metadata(path).map_err(|source| CliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(CliError::Input(format!(
            "input must be a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(format!(
            "input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"
        )));
    }
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["patient_id", "observation"]) {
        return Err(CliError::Input(
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
            return Err(CliError::Input("observation count exceeds 100000".into()));
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
