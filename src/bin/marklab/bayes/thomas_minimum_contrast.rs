use std::fs;

use marklab_bayes::sha256_hex;
use marklab_simulation::{
    ThomasKCurveRow, ThomasMinimumContrastInputIdentity, ThomasMinimumContrastSpec,
    ThomasMinimumContrastWorkerRequest, ThomasMinimumContrastWorkerResult,
};

use super::cli_schema::FitThomasMinimumContrastArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError};

pub(super) fn run(args: FitThomasMinimumContrastArgs) -> Result<(), CliError> {
    let FitThomasMinimumContrastArgs {
        input,
        observed_intensity_per_um2,
        kappa_min_per_um2,
        kappa_max_per_um2,
        sigma_min_um,
        sigma_max_um,
        maximum_iterations,
        timeout_seconds,
        out,
    } = args;
    let input_path = input;
    let output_path = out;
    let input_bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if input_bytes.len() > 16 * 1_048_576 {
        return Err(CliError::Input(
            "Thomas minimum-contrast input exceeds 16 MiB".into(),
        ));
    }
    let curve = read_curve(&input_bytes)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(
        repository,
        "marklab_scipy_thomas_minimum_contrast_worker.py",
    )?;
    let request = ThomasMinimumContrastWorkerRequest::new(
        ThomasMinimumContrastSpec {
            curve,
            observed_intensity_per_um2,
            kappa_min_per_um2,
            kappa_max_per_um2,
            sigma_min_um,
            sigma_max_um,
            maximum_iterations,
        },
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_scipy_thomas_minimum_contrast_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: ThomasMinimumContrastWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(
        &output_path,
        &result.into_fit(
            request,
            ThomasMinimumContrastInputIdentity {
                path: input_path.display().to_string(),
                sha256: sha256_hex(&input_bytes),
            },
        ),
    )
}

fn read_curve(bytes: &[u8]) -> Result<Vec<ThomasKCurveRow>, CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["radius_um", "observed_k_um2", "weight"] {
        return Err(CliError::Input(
            "Thomas minimum-contrast headers must be exactly: radius_um,observed_k_um2,weight"
                .into(),
        ));
    }
    let mut curve = Vec::new();
    for row in reader.deserialize() {
        curve.push(row?);
        if curve.len() > 1_000 {
            return Err(CliError::Input(
                "Thomas minimum-contrast curve exceeds 1000 rows".into(),
            ));
        }
    }
    Ok(curve)
}
