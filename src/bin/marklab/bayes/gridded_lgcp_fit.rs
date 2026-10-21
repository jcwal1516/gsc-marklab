use marklab_bayes::{
    json_sha256, sha256_hex, GriddedLgcpFitInputIdentity, GriddedLgcpFitSpec,
    GriddedLgcpFitWorkerRequest, GriddedLgcpFitWorkerResult, GriddedLgcpSpec, RectangularWindow,
};

use super::cli_schema::SimulateGriddedLgcpPosteriorPredictiveArgs;
use super::cli_schema::{FitGriddedLgcpArgs, GriddedLgcpModelArgs};
use super::worker_identity;
use super::{
    inhomogeneous_poisson::{read_events, read_quadrature},
    publish_json, run_worker, CliError,
};

pub(super) fn run(args: FitGriddedLgcpArgs) -> Result<(), CliError> {
    let FitGriddedLgcpArgs { model, out } = args;
    let output_path = out;
    let prepared = prepare(None, model)?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_fit(prepared.request, prepared.input_identity),
    )
}

pub(super) fn run_predictive(
    args: SimulateGriddedLgcpPosteriorPredictiveArgs,
) -> Result<(), CliError> {
    let SimulateGriddedLgcpPosteriorPredictiveArgs {
        model,
        replicates,
        prediction_seed,
        maximum_total_points,
        out,
    } = args;
    let output_path = out;
    let prepared = prepare(
        Some((replicates, prediction_seed, maximum_total_points)),
        model,
    )?;
    let result = execute(&prepared)?;
    publish_json(
        &output_path,
        &result.into_prediction(prepared.request, prepared.input_identity)?,
    )
}

pub(crate) struct PreparedGriddedLgcpFit {
    pub(crate) request: GriddedLgcpFitWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: GriddedLgcpFitInputIdentity,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(
    prediction: Option<(u32, u64, u64)>,
    model: GriddedLgcpModelArgs,
) -> Result<PreparedGriddedLgcpFit, CliError> {
    let GriddedLgcpModelArgs {
        events: events_path,
        grid: grid_path,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        nuts,
        timeout_seconds,
    } = model;
    let sampling = nuts.spec();
    let events = read_events(&events_path)?;
    let grid = read_quadrature(&grid_path)?;
    let events_sha256 = json_sha256(&events)?;
    let grid_sha256 = json_sha256(&grid)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_gridded_lgcp_worker.py")?;
    let mut request = GriddedLgcpFitWorkerRequest::new(
        GriddedLgcpFitSpec {
            construction: GriddedLgcpSpec {
                window: RectangularWindow {
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                },
                grid_x,
                grid_y,
                events,
                grid,
                intercept_prior_mean,
                intercept_prior_sd,
                coefficient_prior_mean,
                coefficient_prior_sd,
                field_amplitude,
                field_length_scale_um,
                jitter,
            },
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    if let Some((replicates, seed, maximum_total_points)) = prediction {
        request = request.with_prediction(replicates, seed, maximum_total_points)?;
    }
    let input = GriddedLgcpFitInputIdentity {
        events_path: events_path.display().to_string(),
        grid_path: grid_path.display().to_string(),
        events_sha256,
        grid_sha256,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedGriddedLgcpFit {
        request,
        request_bytes,
        request_sha256,
        input_identity: input,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedGriddedLgcpFit,
) -> Result<GriddedLgcpFitWorkerResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_gridded_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: GriddedLgcpFitWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result)
}
