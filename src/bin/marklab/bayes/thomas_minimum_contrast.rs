use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, ThomasKCurveRow, ThomasMinimumContrastInputIdentity, ThomasMinimumContrastSpec,
    ThomasMinimumContrastWorkerRequest, ThomasMinimumContrastWorkerResult,
};

use super::{publish_json, run_worker, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    observed_intensity_per_um2: f64,
    kappa_min_per_um2: f64,
    kappa_max_per_um2: f64,
    sigma_min_um: f64,
    sigma_max_um: f64,
    maximum_iterations: u32,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if input_bytes.len() > 16 * 1_048_576 {
        return Err(BayesCliError::Input(
            "Thomas minimum-contrast input exceeds 16 MiB".into(),
        ));
    }
    let curve = read_curve(&input_bytes)?;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_scipy_thomas_minimum_contrast_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
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
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
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

fn read_curve(bytes: &[u8]) -> Result<Vec<ThomasKCurveRow>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["radius_um", "observed_k_um2", "weight"] {
        return Err(BayesCliError::Input(
            "Thomas minimum-contrast headers must be exactly: radius_um,observed_k_um2,weight"
                .into(),
        ));
    }
    let mut curve = Vec::new();
    for row in reader.deserialize() {
        curve.push(row?);
        if curve.len() > 1_000 {
            return Err(BayesCliError::Input(
                "Thomas minimum-contrast curve exceeds 1000 rows".into(),
            ));
        }
    }
    Ok(curve)
}
