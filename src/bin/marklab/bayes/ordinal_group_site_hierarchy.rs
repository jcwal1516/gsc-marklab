use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use marklab_bayes::{
    ordinal_group_site_data_sha256, sha256_hex, NutsSamplingSpec,
    OrdinalGroupSiteHierarchyInputIdentity, OrdinalGroupSiteHierarchySpec,
    OrdinalGroupSiteHierarchyWorkerRequest, OrdinalGroupSiteHierarchyWorkerResult,
    OrdinalGroupSitePatientData,
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
    OrdinalGroupSiteHierarchy(Box<ArgsValue>),
}

#[derive(Debug, Args)]
struct ArgsValue {
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
    group_effect_prior_sd: f64,
    #[arg(long)]
    site_intercept_sd_prior_sd: f64,
    #[arg(long)]
    site_group_slope_sd_prior_sd: f64,
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
    site_id: String,
    group: String,
    outcome: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Cli {
        command:
            TopLevel::Bayes {
                command: Command::OrdinalGroupSiteHierarchy(args),
            },
    } = Cli::parse();
    let prepared = prepare(
        args.input,
        args.reference_group,
        args.comparison_group,
        args.ordered_levels.split(',').map(str::to_owned).collect(),
        args.cutpoint_prior_sd,
        args.group_effect_prior_sd,
        args.site_intercept_sd_prior_sd,
        args.site_group_slope_sd_prior_sd,
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

pub(crate) struct PreparedOrdinalGroupSiteHierarchy {
    pub(crate) request: OrdinalGroupSiteHierarchyWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: OrdinalGroupSiteHierarchyInputIdentity,
    pub(crate) timeout_seconds: u64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    cutpoint_prior_sd: f64,
    group_effect_prior_sd: f64,
    site_intercept_sd_prior_sd: f64,
    site_group_slope_sd_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedOrdinalGroupSiteHierarchy, BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "ordinal site hierarchy input exceeds 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
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
    let patients = reader
        .deserialize::<PatientRow>()
        .map(
            |row| -> Result<OrdinalGroupSitePatientData, BayesCliError> {
                let row = row?;
                let outcome_code =
                    level_codes
                        .get(row.outcome.as_str())
                        .copied()
                        .ok_or_else(|| {
                            BayesCliError::Input(format!(
                                "ordinal site hierarchy outcome {:?} is not in --ordered-levels",
                                row.outcome
                            ))
                        })?;
                Ok(OrdinalGroupSitePatientData {
                    patient_id: row.patient_id,
                    site_id: row.site_id,
                    group: row.group,
                    outcome_code,
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_bytes =
        fs::read(worker_directory.join("uv.lock")).map_err(|source| BayesCliError::Io {
            path: worker_directory.join("uv.lock"),
            source,
        })?;
    let worker_path = worker_directory.join("marklab_pymc_ordinal_group_site_hierarchy_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = OrdinalGroupSiteHierarchyWorkerRequest::new(
        OrdinalGroupSiteHierarchySpec {
            reference_group: reference_group.clone(),
            comparison_group: comparison_group.clone(),
            ordered_levels: ordered_levels.clone(),
            cutpoint_prior_sd,
            group_effect_prior_sd,
            site_intercept_sd_prior_sd,
            site_group_slope_sd_prior_sd,
            patients,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input_identity = OrdinalGroupSiteHierarchyInputIdentity {
        path: input_path.display().to_string(),
        patient_data_sha256: ordinal_group_site_data_sha256(&request.patients)?,
        patient_count: request.patients.len(),
        site_count: request.site_ids.len(),
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
    Ok(PreparedOrdinalGroupSiteHierarchy {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedOrdinalGroupSiteHierarchy,
) -> Result<OrdinalGroupSiteHierarchyWorkerResult, BayesCliError> {
    let result_bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_pymc_ordinal_group_site_hierarchy_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: OrdinalGroupSiteHierarchyWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
