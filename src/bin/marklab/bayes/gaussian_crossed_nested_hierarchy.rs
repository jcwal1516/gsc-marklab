use std::{fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, GaussianCrossedNestedHierarchySpec, GaussianCrossedNestedHierarchyWorkerRequest,
    GaussianCrossedNestedInputIdentity, GaussianCrossedNestedObservation,
    GaussianCrossedNestedWorkerResult, NutsSamplingSpec,
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
    GaussianCrossedNestedHierarchy(Options),
}

#[derive(Debug, Args)]
struct Options {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    slope_prior_sd: f64,
    #[arg(long)]
    component_prior_sd: f64,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    patient_id: String,
    slide_id: String,
    roi_id: String,
    batch_id: String,
    cohort_id: String,
    exposure: f64,
    outcome: f64,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Cli {
        command:
            TopLevel::Bayes {
                command: Command::GaussianCrossedNestedHierarchy(options),
            },
    } = Cli::parse();
    run(
        options.input,
        options.intercept_prior_sd,
        options.slope_prior_sd,
        options.component_prior_sd,
        NutsSamplingSpec {
            chains: options.chains,
            tune_per_chain: options.tune,
            draws_per_chain: options.draws,
            target_accept: options.target_accept,
            seed: options.seed,
        },
        options.timeout_seconds,
        options.out,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    intercept_prior_sd: f64,
    slope_prior_sd: f64,
    component_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = prepare(
        input_path,
        intercept_prior_sd,
        slope_prior_sd,
        component_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_fit(prepared.request, prepared.input_identity),
    )
}

pub(crate) struct PreparedGaussianCrossedNestedHierarchy {
    pub(crate) request: GaussianCrossedNestedHierarchyWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: GaussianCrossedNestedInputIdentity,
    pub(crate) timeout_seconds: u64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input_path: PathBuf,
    intercept_prior_sd: f64,
    slope_prior_sd: f64,
    component_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedGaussianCrossedNestedHierarchy, BayesCliError> {
    let observations = read_observations(&input_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path =
        worker_directory.join("marklab_pymc_gaussian_crossed_nested_hierarchy_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = GaussianCrossedNestedHierarchyWorkerRequest::new(
        GaussianCrossedNestedHierarchySpec {
            intercept_prior_sd,
            slope_prior_sd,
            component_prior_sd,
            observations,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let counts = request.counts();
    let observation_bytes = serde_json::to_vec(&request.observations)?;
    let input_identity = GaussianCrossedNestedInputIdentity {
        path: input_path.display().to_string(),
        patients: counts.patients,
        slides: counts.slides,
        rois: counts.rois,
        batches: counts.batches,
        cohorts: counts.cohorts,
        observations: counts.observations,
        observation_data_sha256: sha256_hex(&observation_bytes),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedGaussianCrossedNestedHierarchy {
        request,
        request_bytes,
        request_sha256,
        input_identity,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedGaussianCrossedNestedHierarchy,
) -> Result<GaussianCrossedNestedWorkerResult, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes = run_worker(
        repository,
        "marklab_pymc_gaussian_crossed_nested_hierarchy_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: GaussianCrossedNestedWorkerResult = serde_json::from_slice(&bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}

fn read_observations(
    path: &std::path::Path,
) -> Result<Vec<GaussianCrossedNestedObservation>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(BayesCliError::Input(format!(
            "input must be a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"
        )));
    }
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq([
        "patient_id",
        "slide_id",
        "roi_id",
        "batch_id",
        "cohort_id",
        "exposure",
        "outcome",
    ]) {
        return Err(BayesCliError::Input(
            "crossed/nested hierarchy CSV headers must be exactly: patient_id,slide_id,roi_id,batch_id,cohort_id,exposure,outcome".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        let row = row?;
        observations.push(GaussianCrossedNestedObservation {
            patient_id: row.patient_id,
            slide_id: row.slide_id,
            roi_id: row.roi_id,
            batch_id: row.batch_id,
            cohort_id: row.cohort_id,
            exposure: row.exposure,
            outcome: row.outcome,
        });
        if observations.len() > 4_096 {
            return Err(BayesCliError::Input(
                "crossed/nested hierarchy input exceeds 4096 observations".into(),
            ));
        }
    }
    Ok(observations)
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
