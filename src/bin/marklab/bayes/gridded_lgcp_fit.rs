use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, GriddedLgcpFitInputIdentity, GriddedLgcpFitSpec, GriddedLgcpFitWorkerRequest,
    GriddedLgcpFitWorkerResult, GriddedLgcpSpec, NutsSamplingSpec, RectangularWindow,
};

use super::{
    inhomogeneous_poisson::{read_events, read_quadrature},
    publish_json, run_worker, BayesCliError,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let (request, input, result) = execute(
        events_path,
        grid_path,
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
        sampling,
        timeout_seconds,
        None,
    )?;
    publish_json(&output_path, &result.into_fit(request, input))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_predictive(
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    replicates: u32,
    prediction_seed: u64,
    maximum_total_points: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let (request, input, result) = execute(
        events_path,
        grid_path,
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
        sampling,
        timeout_seconds,
        Some((replicates, prediction_seed, maximum_total_points)),
    )?;
    publish_json(&output_path, &result.into_prediction(request, input)?)
}

#[allow(clippy::too_many_arguments)]
fn execute(
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    prediction: Option<(u32, u64, u64)>,
) -> Result<
    (
        GriddedLgcpFitWorkerRequest,
        GriddedLgcpFitInputIdentity,
        GriddedLgcpFitWorkerResult,
    ),
    BayesCliError,
> {
    let events = read_events(&events_path)?;
    let grid = read_quadrature(&grid_path)?;
    let events_sha256 = sha256_hex(&serde_json::to_vec(&events)?);
    let grid_sha256 = sha256_hex(&serde_json::to_vec(&grid)?);
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_gridded_lgcp_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
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
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
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
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_gridded_lgcp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: GriddedLgcpFitWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    Ok((request, input, result))
}
