use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab::{
    arbitrary_window_ipp_log_likelihood, ArbitraryWindowIppError, ArbitraryWindowIppEvent,
    ArbitraryWindowIppLikelihoodResult, ArbitraryWindowIppLimits, ArbitraryWindowIppQuadratureNode,
    ArbitraryWindowIppSpec, ObservationWindow2D, ObservationWindowLimits,
};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct ArbitraryWindowIppCli {
    #[command(subcommand)]
    command: ArbitraryWindowIppTopLevel,
}

#[derive(Debug, Subcommand)]
enum ArbitraryWindowIppTopLevel {
    Bayes {
        #[command(subcommand)]
        command: ArbitraryWindowIppCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ArbitraryWindowIppCommand {
    ArbitraryWindowIppLikelihood {
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        quadrature: PathBuf,
        #[arg(long)]
        window: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        intercept: f64,
        #[arg(long, allow_hyphen_values = true)]
        coefficient: f64,
        #[arg(long)]
        maximum_events: usize,
        #[arg(long)]
        maximum_quadrature_nodes: usize,
        #[arg(long)]
        maximum_work: usize,
        #[arg(long, default_value_t = 16 * 1024 * 1024)]
        maximum_retained_bytes: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let ArbitraryWindowIppTopLevel::Bayes { command } =
        ArbitraryWindowIppCli::parse_from(std::env::args_os()).command;
    let ArbitraryWindowIppCommand::ArbitraryWindowIppLikelihood {
        events,
        quadrature,
        window,
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
        out,
    } = command;
    run(
        events,
        quadrature,
        window,
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
        out,
    )
}

pub(crate) struct PreparedArbitraryWindowIpp {
    pub(crate) window: ObservationWindow2D,
    pub(crate) spec: ArbitraryWindowIppSpec,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept: f64,
    coefficient: f64,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_work: usize,
    maximum_retained_bytes: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = prepare(
        events_path,
        quadrature_path,
        window_path,
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
    )?;
    let result = execute(&prepared)?;
    publish_json(&output_path, &result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    events_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept: f64,
    coefficient: f64,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_work: usize,
    maximum_retained_bytes: usize,
) -> Result<PreparedArbitraryWindowIpp, BayesCliError> {
    let events = read_csv::<ArbitraryWindowIppEvent>(&events_path, "events")?;
    let quadrature = read_csv::<ArbitraryWindowIppQuadratureNode>(&quadrature_path, "quadrature")?;
    validate_file(&window_path, "window")?;
    let window_text = fs::read_to_string(&window_path).map_err(|source| BayesCliError::Io {
        path: window_path,
        source,
    })?;
    let window =
        ObservationWindow2D::from_geojson_str(&window_text, ObservationWindowLimits::default())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    Ok(PreparedArbitraryWindowIpp {
        window,
        spec: ArbitraryWindowIppSpec {
            events,
            quadrature,
            intercept,
            coefficient,
            limits: ArbitraryWindowIppLimits {
                maximum_events,
                maximum_quadrature_nodes,
                maximum_work,
                maximum_retained_bytes,
            },
        },
    })
}

pub(crate) fn execute(
    prepared: &PreparedArbitraryWindowIpp,
) -> Result<ArbitraryWindowIppLikelihoodResult, BayesCliError> {
    arbitrary_window_ipp_log_likelihood(&prepared.window, prepared.spec.clone()).map_err(map_error)
}

fn read_csv<T: serde::de::DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<Vec<T>, BayesCliError> {
    validate_file(path, label)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    reader
        .deserialize()
        .collect::<Result<Vec<_>, _>>()
        .map_err(BayesCliError::Csv)
}

fn validate_file(path: &Path, label: &str) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "arbitrary-window IPP {label} must be a regular file within 16 MiB"
        )));
    }
    Ok(())
}

fn map_error(error: ArbitraryWindowIppError) -> BayesCliError {
    match error {
        ArbitraryWindowIppError::Invalid(message) => BayesCliError::Input(message),
        ArbitraryWindowIppError::Numerical(message) => BayesCliError::Backend(message),
    }
}
