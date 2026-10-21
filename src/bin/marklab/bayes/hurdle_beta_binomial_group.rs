use std::{fs, path::PathBuf};

use clap::Args;
use marklab_bayes::{
    json_sha256, sha256_hex, HurdleBetaBinomialGroupInputIdentity,
    HurdleBetaBinomialGroupPatientData, HurdleBetaBinomialGroupSpec,
    HurdleBetaBinomialGroupWorkerRequest, HurdleBetaBinomialGroupWorkerResult,
};
use serde::Deserialize;

use super::worker_identity;
use super::HurdleBetaBinomialGroupModelArgs;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Args)]
pub(crate) struct HurdleBetaBinomialGroupArgs {
    #[command(flatten)]
    pub(super) model: HurdleBetaBinomialGroupModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    group: String,
    successes: u64,
    trials: u64,
}

pub(super) fn run(args: HurdleBetaBinomialGroupArgs) -> Result<(), CliError> {
    let prepared = prepare(args.model)?;
    let result = execute(&prepared)?;
    publish_json(
        &args.out,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedHurdleBetaBinomialGroup {
    pub(crate) request: HurdleBetaBinomialGroupWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: HurdleBetaBinomialGroupInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: HurdleBetaBinomialGroupModelArgs,
) -> Result<PreparedHurdleBetaBinomialGroup, CliError> {
    let HurdleBetaBinomialGroupModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        presence_intercept_prior_mean,
        presence_intercept_prior_sd,
        presence_group_effect_prior_sd,
        abundance_intercept_prior_mean,
        abundance_intercept_prior_sd,
        abundance_group_effect_prior_sd,
        concentration_prior_sd,
        timeout_seconds,
        nuts,
    } = model;
    let sampling = nuts.spec();

    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "hurdle beta-binomial input exceeds 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(input_bytes.as_slice());
    let patients = reader
        .deserialize::<PatientRow>()
        .map(|row| {
            row.map(|row| HurdleBetaBinomialGroupPatientData {
                patient_id: row.patient_id,
                group: row.group,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(
        repository,
        "marklab_pymc_hurdle_beta_binomial_group_worker.py",
    )?;
    let request = HurdleBetaBinomialGroupWorkerRequest::new(
        HurdleBetaBinomialGroupSpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            presence_intercept_prior_mean,
            presence_intercept_prior_sd,
            presence_group_effect_prior_sd,
            abundance_intercept_prior_mean,
            abundance_intercept_prior_sd,
            abundance_group_effect_prior_sd,
            concentration_prior_sd,
            patients,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input_identity = HurdleBetaBinomialGroupInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: json_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        zero_count: request
            .patients
            .iter()
            .filter(|patient| patient.successes == 0)
            .count(),
        positive_count: request
            .patients
            .iter()
            .filter(|patient| patient.successes > 0)
            .count(),
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
        total_trials: request.patients.iter().map(|patient| patient.trials).sum(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedHurdleBetaBinomialGroup {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedHurdleBetaBinomialGroup,
) -> Result<HurdleBetaBinomialGroupWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_hurdle_beta_binomial_group_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: HurdleBetaBinomialGroupWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
