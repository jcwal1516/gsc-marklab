use super::cli_schema::FitStraussPseudolikelihoodArgs;
use super::{publish_json, run_worker, CliError};
use marklab_bayes::{
    sha256_hex, RectangularWindow, StraussPoint, StraussPseudolikelihoodInputIdentity,
    StraussPseudolikelihoodSpec, StraussPseudolikelihoodWorkerRequest,
    StraussPseudolikelihoodWorkerResult,
};
use std::fs;

pub(super) fn run(args: FitStraussPseudolikelihoodArgs) -> Result<(), CliError> {
    let FitStraussPseudolikelihoodArgs {
        input,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        interaction_radius_um,
        coarse_grid_x,
        coarse_grid_y,
        fine_grid_x,
        fine_grid_y,
        beta_min_per_um2,
        beta_max_per_um2,
        gamma_min,
        gamma_max,
        maximum_iterations,
        maximum_neighbor_visits,
        timeout_seconds,
        out,
    } = args;
    let xmin = xmin_um;
    let ymin = ymin_um;
    let xmax = xmax_um;
    let ymax = ymax_um;
    let radius = interaction_radius_um;
    let cgx = coarse_grid_x;
    let cgy = coarse_grid_y;
    let fgx = fine_grid_x;
    let fgy = fine_grid_y;
    let bmin = beta_min_per_um2;
    let bmax = beta_max_per_um2;
    let gmin = gamma_min;
    let gmax = gamma_max;
    let iterations = maximum_iterations;
    let visits = maximum_neighbor_visits;
    let timeout = timeout_seconds;
    let bytes = fs::read(&input).map_err(|source| CliError::Io {
        path: input.clone(),
        source,
    })?;
    if bytes.len() > 16 * 1_048_576 {
        return Err(CliError::Input(
            "Strauss pseudolikelihood input exceeds 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes.as_slice());
    if reader.headers()?.iter().collect::<Vec<_>>() != ["point_id", "x_um", "y_um"] {
        return Err(CliError::Input(
            "Strauss point headers must be exactly: point_id,x_um,y_um".into(),
        ));
    }
    let mut points = Vec::<StraussPoint>::new();
    for row in reader.deserialize() {
        points.push(row?);
    }
    let point_count = points.len() as u32;
    let repo = &marklab::backend::python_backend_assets_root()?;
    let dir = repo.join("workers/python");
    let lock = fs::read(dir.join("uv.lock")).map_err(|source| CliError::Io {
        path: dir.join("uv.lock"),
        source,
    })?;
    let worker_path = dir.join("marklab_scipy_strauss_pseudolikelihood_worker.py");
    let worker = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = StraussPseudolikelihoodWorkerRequest::new(
        StraussPseudolikelihoodSpec {
            points,
            window: RectangularWindow {
                xmin_um: xmin,
                ymin_um: ymin,
                xmax_um: xmax,
                ymax_um: ymax,
            },
            interaction_radius_um: radius,
            coarse_grid_x: cgx,
            coarse_grid_y: cgy,
            fine_grid_x: fgx,
            fine_grid_y: fgy,
            beta_min_per_um2: bmin,
            beta_max_per_um2: bmax,
            gamma_min: gmin,
            gamma_max: gmax,
            maximum_iterations: iterations,
            maximum_neighbor_visits: visits,
        },
        sha256_hex(&lock),
        sha256_hex(&worker),
        timeout,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repo,
        "marklab_scipy_strauss_pseudolikelihood_worker.py",
        &request_bytes,
        timeout,
    )?;
    let result: StraussPseudolikelihoodWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha)?;
    publish_json(
        &out,
        &result.into_fit(
            request,
            StraussPseudolikelihoodInputIdentity {
                path: input.display().to_string(),
                sha256: sha256_hex(&bytes),
                point_count,
            },
        ),
    )
}
