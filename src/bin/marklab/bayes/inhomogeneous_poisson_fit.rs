use marklab_bayes::{
    json_sha256, sha256_hex, InhomogeneousPoissonFitInputIdentity, InhomogeneousPoissonFitSpec,
    InhomogeneousPoissonFitWorkerRequest, InhomogeneousPoissonFitWorkerResult, RectangularWindow,
};

use super::cli_schema::FitInhomogeneousPoissonArgs;
use super::worker_identity;
use super::{
    inhomogeneous_poisson::{read_events, read_quadrature},
    publish_json, run_worker, CliError,
};

pub(super) fn run(args: FitInhomogeneousPoissonArgs) -> Result<(), CliError> {
    let FitInhomogeneousPoissonArgs {
        events,
        quadrature,
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
        timeout_seconds,
        out,
        nuts,
    } = args;
    let events_path = events;
    let quadrature_path = quadrature;
    let sampling = nuts.spec();
    let output_path = out;
    let events = read_events(&events_path)?;
    let quadrature = read_quadrature(&quadrature_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_inhomogeneous_poisson_worker.py")?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input = InhomogeneousPoissonFitInputIdentity {
        events_path: events_path.display().to_string(),
        quadrature_path: quadrature_path.display().to_string(),
        events_sha256: json_sha256(&request.events)?,
        quadrature_sha256: json_sha256(&request.quadrature)?,
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
