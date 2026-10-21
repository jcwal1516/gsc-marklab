use std::{collections::BTreeMap, fs};

use marklab_bayes::{
    json_sha256, sha256_hex, DirichletMultinomialGroupInputIdentity, DirichletMultinomialGroupSpec,
    DirichletMultinomialGroupWorkerRequest, DirichletMultinomialGroupWorkerResult,
    DirichletMultinomialPatientData,
};
use serde::Deserialize;

use super::cli_schema::{DirichletMultinomialGroupArgs, DirichletMultinomialGroupModelArgs};
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountRow {
    patient_id: String,
    group: String,
    class_id: String,
    count: u64,
}

pub(super) fn run(args: DirichletMultinomialGroupArgs) -> Result<(), CliError> {
    let DirichletMultinomialGroupArgs {
        model,
        out: output_path,
    } = args;
    let prepared = prepare(model)?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedDirichletMultinomialGroup {
    pub(crate) request: DirichletMultinomialGroupWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: DirichletMultinomialGroupInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: DirichletMultinomialGroupModelArgs,
) -> Result<PreparedDirichletMultinomialGroup, CliError> {
    let DirichletMultinomialGroupModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        logit_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        nuts,
        timeout_seconds,
    } = model;
    let sampling = nuts.spec();
    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "Dirichlet-multinomial input exceeds 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let rows = reader
        .deserialize::<CountRow>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CliError::Input(format!("invalid Dirichlet-multinomial count CSV: {error}"))
        })?;
    let mut class_ids = Vec::new();
    let mut patients = BTreeMap::<String, (String, BTreeMap<String, u64>)>::new();
    for row in rows {
        if !class_ids.contains(&row.class_id) {
            class_ids.push(row.class_id.clone());
        }
        let patient = patients
            .entry(row.patient_id)
            .or_insert_with(|| (row.group.clone(), BTreeMap::new()));
        if patient.0 != row.group || patient.1.insert(row.class_id, row.count).is_some() {
            return Err(CliError::Input(
                "Dirichlet-multinomial patient groups or class rows conflict".into(),
            ));
        }
    }
    let patients = patients
        .into_iter()
        .map(|(patient_id, (group, counts))| {
            if counts.len() != class_ids.len() {
                return Err(CliError::Input(
                    "every patient must contain every Dirichlet-multinomial class exactly once"
                        .into(),
                ));
            }
            Ok(DirichletMultinomialPatientData {
                patient_id,
                group,
                counts: class_ids.iter().map(|class| counts[class]).collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(
        repository,
        "marklab_pymc_dirichlet_multinomial_group_worker.py",
    )?;
    let request = DirichletMultinomialGroupWorkerRequest::new(
        DirichletMultinomialGroupSpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            class_ids: class_ids.clone(),
            logit_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            patients,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input_identity = DirichletMultinomialGroupInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: json_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        class_count: class_ids.len(),
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
    Ok(PreparedDirichletMultinomialGroup {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedDirichletMultinomialGroup,
) -> Result<DirichletMultinomialGroupWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_dirichlet_multinomial_group_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: DirichletMultinomialGroupWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
