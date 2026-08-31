use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, NutsSamplingSpec, SpatialCoefficientInputIdentity, SpatialCoefficientObservation,
    SpatialVaryingCoefficientSpec, SpatialVaryingCoefficientWorkerRequest,
    SpatialVaryingCoefficientWorkerResult,
};

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

pub(crate) struct Prepared {
    request: SpatialVaryingCoefficientWorkerRequest,
    identity: SpatialCoefficientInputIdentity,
    request_bytes: Vec<u8>,
    request_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }

    pub(crate) fn validate_output(&self, output: &serde_json::Value) -> Result<(), BayesCliError> {
        let object = output.as_object().ok_or_else(|| {
            BayesCliError::Backend("spatial coefficient cached result is not an object".into())
        })?;
        let expected_keys = [
            "backend",
            "claim_status",
            "coefficient_field",
            "constraints",
            "diagnostics",
            "fit_state",
            "format",
            "input",
            "model",
            "posterior",
            "posterior_predictive",
            "request_sha256",
            "sampling",
            "seed",
            "version",
        ];
        let mut actual_keys = object.keys().map(String::as_str).collect::<Vec<_>>();
        actual_keys.sort_unstable();
        let expected_draws = u64::from(self.request.sampling.chains)
            * u64::from(self.request.sampling.draws_per_chain);
        let fit_state = output["fit_state"].as_str();
        let claim_status = output["claim_status"].as_str();
        let fields = output["coefficient_field"].as_array();
        if actual_keys != expected_keys
            || output["format"] != "marklab.bayesian_spatial_varying_coefficient_fit"
            || output["version"] != 1
            || output["model"] != serde_json::to_value(&self.request.model)?
            || output["input"] != serde_json::to_value(&self.identity)?
            || output["backend"]["name"] != self.request.backend.name
            || output["backend"]["version"] != self.request.backend.version
            || output["backend"]["python_version"] != self.request.backend.python_version
            || output["backend"]["environment_lock_sha256"]
                != self.request.backend.environment_lock_sha256
            || output["backend"]["worker_sha256"] != self.request.backend.worker_sha256
            || !matches!(fit_state, Some("complete" | "nonconverged"))
            || (fit_state == Some("complete")) != (claim_status == Some("experimental"))
            || (fit_state == Some("nonconverged"))
                != (claim_status == Some("diagnostic_only_nonconverged"))
            || output["sampling"]["chains"] != self.request.sampling.chains
            || output["sampling"]["tune_per_chain"] != self.request.sampling.tune_per_chain
            || output["sampling"]["draws_per_chain"] != self.request.sampling.draws_per_chain
            || output["sampling"]["completed_draws"] != expected_draws
            || output["constraints"] != serde_json::json!(["sum_to_zero:delta"])
            || output["seed"] != self.request.sampling.seed
            || output["request_sha256"] != self.request_sha256
            || fields.is_none_or(|rows| rows.len() != self.request.observations.len())
            || fields.is_some_and(|rows| {
                rows.iter()
                    .zip(&self.request.observations)
                    .any(|(actual, expected)| {
                        actual["coordinate_id"] != expected.coordinate_id
                            || actual["x_um"].as_f64().map(f64::to_bits)
                                != Some(expected.x_um.to_bits())
                    })
            })
        {
            return Err(BayesCliError::Backend(
                "spatial coefficient cached result identity or shape differs".into(),
            ));
        }
        Ok(())
    }
}

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
    let prepared = prepare(
        input_path,
        global_predictor_name,
        spatial_predictor_name,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_sd,
        amplitude_prior_sd,
        length_scale_prior_sd_um,
        known_noise_sd,
        jitter,
        sampling,
        timeout_seconds,
    )?;
    publish_json(&output_path, &execute(&prepared)?)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
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
) -> Result<Prepared, BayesCliError> {
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
    Ok(Prepared {
        request,
        identity,
        request_bytes,
        request_sha256,
        timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &Prepared,
) -> Result<marklab_bayes::SpatialVaryingCoefficientFit, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_spatial_varying_coefficient_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: SpatialVaryingCoefficientWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    let request = prepared.request.clone();
    let identity = prepared.identity.clone();
    Ok(result.into_fit(request, identity))
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
