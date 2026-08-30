use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NutsSamplingSpec, SpatialCoefficientInputIdentity, SpatialCoefficientObservation,
    SpatialVaryingCoefficientSpec, SpatialVaryingCoefficientWorkerRequest,
    SpatialVaryingCoefficientWorkerResult,
};

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    global_predictor_name: String,
    spatial_predictor_name: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_sd: f64,
    amplitude_prior_sd: f64,
    length_scale_prior_sd_um: f64,
    known_noise_sd: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations =
        read_observations(&input_path, &global_predictor_name, &spatial_predictor_name)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_spatial_varying_coefficient_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = SpatialVaryingCoefficientWorkerRequest::new(
        SpatialVaryingCoefficientSpec {
            global_predictor_name,
            spatial_predictor_name,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            known_noise_sd,
            jitter,
            observations,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let identity = SpatialCoefficientInputIdentity {
        path: input_path.display().to_string(),
        observations_sha256: sha256_hex(&serde_json::to_vec(&request.observations)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_spatial_varying_coefficient_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: SpatialVaryingCoefficientWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, identity))
}

fn read_observations(
    path: &std::path::Path,
    global_predictor_name: &str,
    spatial_predictor_name: &str,
) -> Result<Vec<SpatialCoefficientObservation>, BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "spatial coefficient input must be a regular file within the 16 MiB limit",
    )?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq([
        "coordinate_id",
        "x_um",
        "y",
        global_predictor_name,
        spatial_predictor_name,
    ]) {
        return Err(BayesCliError::Input(format!(
            "spatial coefficient headers must be exactly: coordinate_id,x_um,y,{global_predictor_name},{spatial_predictor_name}"
        )));
    }
    let mut observations = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize, field: &str| {
            record[index].parse::<f64>().map_err(|_| {
                BayesCliError::Input(format!("spatial coefficient {field} must be numeric"))
            })
        };
        observations.push(SpatialCoefficientObservation {
            coordinate_id: record[0].to_owned(),
            x_um: parse(1, "x_um")?,
            outcome: parse(2, "y")?,
            global_predictor: parse(3, global_predictor_name)?,
            spatial_predictor: parse(4, spatial_predictor_name)?,
        });
        if observations.len() > 64 {
            return Err(BayesCliError::Input(
                "spatial coefficient observation count exceeds 64".into(),
            ));
        }
    }
    Ok(observations)
}
