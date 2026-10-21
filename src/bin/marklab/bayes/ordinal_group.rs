use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::Args;
use marklab_bayes::{
    json_sha256, sha256_hex, OrdinalGroupInputIdentity, OrdinalGroupPatientData, OrdinalGroupSpec,
    OrdinalGroupWorkerRequest, OrdinalGroupWorkerResult,
};
use serde::Deserialize;

use super::worker_identity;
use super::OrdinalGroupModelArgs;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Args)]
pub(crate) struct OrdinalGroupArgs {
    #[command(flatten)]
    pub(super) model: OrdinalGroupModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    group: String,
    outcome: String,
}

pub(super) fn run(args: OrdinalGroupArgs) -> Result<(), CliError> {
    let prepared = prepare(args.model)?;
    let result = execute(&prepared)?;
    publish_json(
        &args.out,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedOrdinalGroup {
    pub(crate) request: OrdinalGroupWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: OrdinalGroupInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(model: OrdinalGroupModelArgs) -> Result<PreparedOrdinalGroup, CliError> {
    let OrdinalGroupModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        ordered_levels,
        cutpoint_prior_sd,
        group_effect_prior_sd,
        timeout_seconds,
        nuts,
    } = model;
    let sampling = nuts.spec();

    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input("ordinal-group input exceeds 16 MiB".into()));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let level_codes = ordered_levels
        .iter()
        .enumerate()
        .map(|(index, level)| (level.as_str(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let rows = reader
        .deserialize::<PatientRow>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| CliError::Input(format!("invalid ordinal-group CSV: {error}")))?;
    let patients = rows
        .into_iter()
        .map(|row| -> Result<OrdinalGroupPatientData, CliError> {
            let outcome_code = level_codes
                .get(row.outcome.as_str())
                .copied()
                .ok_or_else(|| {
                    CliError::Input(format!(
                        "ordinal-group outcome {:?} is not in --ordered-levels",
                        row.outcome
                    ))
                })?;
            Ok(OrdinalGroupPatientData {
                patient_id: row.patient_id,
                group: row.group,
                outcome_code,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_ordinal_group_worker.py")?;
    let request = OrdinalGroupWorkerRequest::new(
        OrdinalGroupSpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            ordered_levels: ordered_levels.clone(),
            cutpoint_prior_sd,
            group_effect_prior_sd,
            patients,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input_identity = OrdinalGroupInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: json_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        level_count: ordered_levels.len(),
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
    Ok(PreparedOrdinalGroup {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedOrdinalGroup,
) -> Result<OrdinalGroupWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_ordinal_group_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: OrdinalGroupWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
