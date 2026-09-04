use std::{fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use marklab_bayes::{
    hurdle_beta_binomial_group_data_sha256, sha256_hex, HurdleBetaBinomialGroupInputIdentity,
    HurdleBetaBinomialGroupPatientData, HurdleBetaBinomialGroupSpec,
    HurdleBetaBinomialGroupWorkerRequest, HurdleBetaBinomialGroupWorkerResult, NutsSamplingSpec,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand)]
enum TopLevel {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    HurdleBetaBinomialGroup(Box<HurdleBetaBinomialGroupArgs>),
}

#[derive(Debug, Args)]
struct HurdleBetaBinomialGroupArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    presence_intercept_prior_mean: f64,
    #[arg(long)]
    presence_intercept_prior_sd: f64,
    #[arg(long)]
    presence_group_effect_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    abundance_intercept_prior_mean: f64,
    #[arg(long)]
    abundance_intercept_prior_sd: f64,
    #[arg(long)]
    abundance_group_effect_prior_sd: f64,
    #[arg(long)]
    concentration_prior_sd: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientRow {
    patient_id: String,
    group: String,
    successes: u64,
    trials: u64,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Cli {
        command:
            TopLevel::Bayes {
                command: Command::HurdleBetaBinomialGroup(args),
            },
    } = Cli::parse();
    let prepared = prepare(
        args.input,
        args.reference_group,
        args.comparison_group,
        args.presence_intercept_prior_mean,
        args.presence_intercept_prior_sd,
        args.presence_group_effect_prior_sd,
        args.abundance_intercept_prior_mean,
        args.abundance_intercept_prior_sd,
        args.abundance_group_effect_prior_sd,
        args.concentration_prior_sd,
        NutsSamplingSpec {
            chains: args.chains,
            tune_per_chain: args.tune,
            draws_per_chain: args.draws,
            target_accept: args.target_accept,
            seed: args.seed,
        },
        args.timeout_seconds,
    )?;
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    presence_intercept_prior_mean: f64,
    presence_intercept_prior_sd: f64,
    presence_group_effect_prior_sd: f64,
    abundance_intercept_prior_mean: f64,
    abundance_intercept_prior_sd: f64,
    abundance_group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedHurdleBetaBinomialGroup, BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "hurdle beta-binomial input exceeds 16 MiB".into(),
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
            row.map(|row| HurdleBetaBinomialGroupPatientData {
                patient_id: row.patient_id,
                group: row.group,
                successes: row.successes,
                trials: row.trials,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_hurdle_beta_binomial_group_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
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
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input_identity = HurdleBetaBinomialGroupInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: hurdle_beta_binomial_group_data_sha256(&request.patients)?,
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
) -> Result<HurdleBetaBinomialGroupWorkerResult, BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
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

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
