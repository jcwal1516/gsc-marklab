use std::{collections::BTreeMap, fs};

use marklab_bayes::{
    json_sha256, sha256_hex, BetaBinomialGroupGenderPatientData,
    BetaBinomialGroupGenderRegressionInputIdentity, BetaBinomialGroupGenderRegressionSpec,
    BetaBinomialGroupGenderRegressionWorkerRequest, BetaBinomialGroupGenderRegressionWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::{
    BetaBinomialGroupGenderRegressionArgs, BetaBinomialGroupGenderRegressionModelArgs,
};
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    group: String,
    gender: String,
    successes: u64,
    trials: u64,
}

pub(super) fn run(args: BetaBinomialGroupGenderRegressionArgs) -> Result<(), CliError> {
    let BetaBinomialGroupGenderRegressionArgs { model, out } = args;
    let output_path = out;
    let prepared = prepare(model)?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedBetaBinomialGroupGenderRegression {
    pub(crate) request: BetaBinomialGroupGenderRegressionWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: BetaBinomialGroupGenderRegressionInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    model: BetaBinomialGroupGenderRegressionModelArgs,
) -> Result<PreparedBetaBinomialGroupGenderRegression, CliError> {
    let BetaBinomialGroupGenderRegressionModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
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
            "beta-binomial group/gender input exceeds 16 MiB".into(),
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
            row.map(|row| BetaBinomialGroupGenderPatientData {
                patient_id: row.patient_id,
                group: row.group,
                gender: row.gender,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CliError::Input(format!(
                "invalid beta-binomial group/gender patient CSV: {error}"
            ))
        })?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(
        repository,
        "marklab_pymc_beta_binomial_group_gender_regression_worker.py",
    )?;
    let request = BetaBinomialGroupGenderRegressionWorkerRequest::new(
        BetaBinomialGroupGenderRegressionSpec {
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            concentration_prior_sd,
            patients,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let mut design_cell_counts = BTreeMap::new();
    for patient in &request.patients {
        *design_cell_counts
            .entry(format!("{}:{}", patient.group, patient.gender))
            .or_insert(0) += 1;
    }
    let input_identity = BetaBinomialGroupGenderRegressionInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: json_sha256(&request.patients)?,
        design_cell_counts,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedBetaBinomialGroupGenderRegression {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedBetaBinomialGroupGenderRegression,
) -> Result<BetaBinomialGroupGenderRegressionWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_beta_binomial_group_gender_regression_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: BetaBinomialGroupGenderRegressionWorkerResult =
        serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
