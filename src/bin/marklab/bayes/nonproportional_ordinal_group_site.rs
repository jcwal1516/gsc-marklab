use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};
use clap::{Args, Parser, Subcommand};
use marklab_bayes::{
    nonproportional_ordinal_group_site_data_sha256, sha256_hex,
    NonproportionalOrdinalGroupSiteInputIdentity, NonproportionalOrdinalGroupSiteSpec,
    NonproportionalOrdinalGroupSiteWorkerRequest, NonproportionalOrdinalGroupSiteWorkerResult,
    NutsSamplingSpec, OrdinalGroupSitePatientData,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: Top,
}
#[derive(Debug, Subcommand)]
enum Top {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}
#[derive(Debug, Subcommand)]
enum Command {
    NonproportionalOrdinalGroupSite(Box<Options>),
}
#[derive(Debug, Args)]
struct Options {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    ordered_levels: String,
    #[arg(long)]
    cutpoint_prior_sd: f64,
    #[arg(long)]
    site_intercept_sd_prior_sd: f64,
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
struct Row {
    patient_id: String,
    site_id: String,
    group: String,
    outcome: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Cli {
        command:
            Top::Bayes {
                command: Command::NonproportionalOrdinalGroupSite(a),
            },
    } = Cli::parse();
    let prepared = prepare(
        a.input,
        a.reference_group,
        a.comparison_group,
        a.ordered_levels.split(',').map(str::to_owned).collect(),
        a.cutpoint_prior_sd,
        a.site_intercept_sd_prior_sd,
        NutsSamplingSpec {
            chains: a.chains,
            tune_per_chain: a.tune,
            draws_per_chain: a.draws,
            target_accept: a.target_accept,
            seed: a.seed,
        },
        a.timeout_seconds,
    )?;
    let result = execute(&prepared)?;
    publish_json(
        &a.out,
        &result.into_result(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedNonproportionalOrdinalGroupSite {
    pub(crate) request: NonproportionalOrdinalGroupSiteWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: NonproportionalOrdinalGroupSiteInputIdentity,
    pub(crate) timeout_seconds: u64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    cutpoint_prior_sd: f64,
    site_intercept_sd_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedNonproportionalOrdinalGroupSite, BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "nonproportional ordinal input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let codes = ordered_levels
        .iter()
        .enumerate()
        .map(|(i, x)| (x.as_str(), i as u32))
        .collect::<BTreeMap<_, _>>();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
    let patients = reader
        .deserialize::<Row>()
        .map(|row| -> Result<_, BayesCliError> {
            let row = row?;
            let outcome_code = codes.get(row.outcome.as_str()).copied().ok_or_else(|| {
                BayesCliError::Input(format!("outcome {:?} is not ordered", row.outcome))
            })?;
            Ok(OrdinalGroupSitePatientData {
                patient_id: row.patient_id,
                site_id: row.site_id,
                group: row.group,
                outcome_code,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = root.join("workers/python");
    let lock_path = directory.join("uv.lock");
    let lock = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = directory.join("marklab_pymc_nonproportional_ordinal_group_site_worker.py");
    let worker = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NonproportionalOrdinalGroupSiteWorkerRequest::new(
        NonproportionalOrdinalGroupSiteSpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            ordered_levels: ordered_levels.clone(),
            cutpoint_prior_sd,
            site_intercept_sd_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock),
        sha256_hex(&worker),
        timeout_seconds,
    )?;
    let identity = NonproportionalOrdinalGroupSiteInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: nonproportional_ordinal_group_site_data_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        site_count: request.site_ids.len(),
        level_count: ordered_levels.len(),
        reference_patients: request
            .patients
            .iter()
            .filter(|p| p.group == reference_group)
            .count(),
        comparison_patients: request
            .patients
            .iter()
            .filter(|p| p.group == comparison_group)
            .count(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedNonproportionalOrdinalGroupSite {
        request,
        request_bytes,
        request_sha256,
        input_identity: identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedNonproportionalOrdinalGroupSite,
) -> Result<NonproportionalOrdinalGroupSiteWorkerResult, BayesCliError> {
    let bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_pymc_nonproportional_ordinal_group_site_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: NonproportionalOrdinalGroupSiteWorkerResult = serde_json::from_slice(&bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
