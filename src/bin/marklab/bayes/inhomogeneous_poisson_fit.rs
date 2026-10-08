use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, InhomogeneousPoissonFitInputIdentity, InhomogeneousPoissonFitSpec,
    InhomogeneousPoissonFitWorkerRequest, InhomogeneousPoissonFitWorkerResult, NutsSamplingSpec,
    RectangularWindow,
};

use super::{
    inhomogeneous_poisson::{read_events, read_quadrature},
    publish_json, run_worker, BayesCliError,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    quadrature_path: PathBuf,
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
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let events = read_events(&events_path)?;
    let quadrature = read_quadrature(&quadrature_path)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_inhomogeneous_poisson_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = InhomogeneousPoissonFitWorkerRequest::new(
        InhomogeneousPoissonFitSpec {
            window: RectangularWindow {
                xmin_um,
                ymin_um,
                xmax_um,
                ymax_um,
            },
            grid_x,
            grid_y,
            events,
            quadrature,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input = InhomogeneousPoissonFitInputIdentity {
        events_path: events_path.display().to_string(),
        quadrature_path: quadrature_path.display().to_string(),
        events_sha256: sha256_hex(&serde_json::to_vec(&request.events)?),
        quadrature_sha256: sha256_hex(&serde_json::to_vec(&request.quadrature)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_inhomogeneous_poisson_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: InhomogeneousPoissonFitWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, input))
}
