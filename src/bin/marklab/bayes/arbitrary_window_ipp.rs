use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::Args;
use marklab::{
    spatial::{
        arbitrary_window_ipp_log_likelihood, ArbitraryWindowIppError, ArbitraryWindowIppEvent,
        ArbitraryWindowIppLikelihoodResult, ArbitraryWindowIppLimits,
        ArbitraryWindowIppQuadratureNode, ArbitraryWindowIppSpec,
    },
    ObservationWindow2D, ObservationWindowLimits,
};

use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Args)]
pub(crate) struct ArbitraryWindowIppLikelihoodArgs {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    events: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    window: PathBuf,
    /// Intercept on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    intercept: f64,
    /// Covariate coefficient on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    coefficient: f64,
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on work.
    #[arg(long)]
    maximum_work: usize,
    /// Upper bound on retained bytes.
    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    maximum_retained_bytes: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) struct PreparedArbitraryWindowIpp {
    pub(crate) window: ObservationWindow2D,
    pub(crate) spec: ArbitraryWindowIppSpec,
}

pub(super) fn run(args: ArbitraryWindowIppLikelihoodArgs) -> Result<(), CliError> {
    let ArbitraryWindowIppLikelihoodArgs {
        events: events_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
        out: output_path,
    } = args;
    let prepared = prepare(
        events_path,
        quadrature_path,
        window_path,
        intercept,
        coefficient,
        marklab::spatial::ArbitraryWindowIppLimits {
            maximum_events,
            maximum_quadrature_nodes,
            maximum_work,
            maximum_retained_bytes,
        },
    )?;
    let result = execute(&prepared)?;
    publish_json(&output_path, &result)
}

pub(crate) fn prepare(
    events_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept: f64,
    coefficient: f64,
    limits: ArbitraryWindowIppLimits,
) -> Result<PreparedArbitraryWindowIpp, CliError> {
    let events = read_csv::<ArbitraryWindowIppEvent>(&events_path, "events")?;
    let quadrature = read_csv::<ArbitraryWindowIppQuadratureNode>(&quadrature_path, "quadrature")?;
    validate_file(&window_path, "window")?;
    let window_text = fs::read_to_string(&window_path).map_err(|source| CliError::Io {
        path: window_path,
        source,
    })?;
    let window =
        ObservationWindow2D::from_geojson_str(&window_text, ObservationWindowLimits::default())
            .map_err(|error| CliError::Input(error.to_string()))?;
    Ok(PreparedArbitraryWindowIpp {
        window,
        spec: ArbitraryWindowIppSpec {
            events,
            quadrature,
            intercept,
            coefficient,
            limits,
        },
    })
}

pub(crate) fn execute(
    prepared: &PreparedArbitraryWindowIpp,
) -> Result<ArbitraryWindowIppLikelihoodResult, CliError> {
    arbitrary_window_ipp_log_likelihood(&prepared.window, prepared.spec.clone()).map_err(map_error)
}

fn read_csv<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> Result<Vec<T>, CliError> {
    validate_file(path, label)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    reader
        .deserialize()
        .collect::<Result<Vec<_>, _>>()
        .map_err(CliError::Csv)
}

fn validate_file(path: &Path, label: &str) -> Result<(), CliError> {
    crate::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        &format!("arbitrary-window IPP {label} must be a regular file within 16 MiB"),
    )
}

fn map_error(error: ArbitraryWindowIppError) -> CliError {
    match error {
        ArbitraryWindowIppError::Invalid(message) => CliError::Input(message),
        ArbitraryWindowIppError::Numerical(message) => CliError::Backend(message),
    }
}
