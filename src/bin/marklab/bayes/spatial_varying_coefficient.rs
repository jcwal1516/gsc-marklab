use std::path::PathBuf;

use marklab_bayes::{
    json_sha256, sha256_hex, NutsSamplingSpec, SpatialCoefficientInputIdentity,
    SpatialCoefficientObservation, SpatialVaryingCoefficientSpec,
    SpatialVaryingCoefficientWorkerRequest, SpatialVaryingCoefficientWorkerResult,
};

use super::cli_schema::SpatialVaryingCoefficientArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

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

    pub(crate) fn validate_output(&self, output: &serde_json::Value) -> Result<(), CliError> {
        let object = output.as_object().ok_or_else(|| {
            CliError::Backend("spatial coefficient cached result is not an object".into())
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
            return Err(CliError::Backend(
                "spatial coefficient cached result identity or shape differs".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn run(args: SpatialVaryingCoefficientArgs) -> Result<(), CliError> {
    let SpatialVaryingCoefficientArgs {
        input,
        global_predictor_name,
        spatial_predictor_name,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_sd,
        amplitude_prior_sd,
        length_scale_prior_sd_um,
        known_noise_sd,
        jitter,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let input_path = input;
    let sampling = nuts.spec();
    let output_path = out;
    let prepared = prepare(Parameters {
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
    })?;
    publish_json(&output_path, &execute(&prepared)?)
}

pub(crate) struct Parameters {
    pub(crate) input_path: PathBuf,
    pub(crate) global_predictor_name: String,
    pub(crate) spatial_predictor_name: String,
    pub(crate) intercept_prior_mean: f64,
    pub(crate) intercept_prior_sd: f64,
    pub(crate) coefficient_prior_sd: f64,
    pub(crate) amplitude_prior_sd: f64,
    pub(crate) length_scale_prior_sd_um: f64,
    pub(crate) known_noise_sd: f64,
    pub(crate) jitter: f64,
    pub(crate) sampling: NutsSamplingSpec,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(parameters: Parameters) -> Result<Prepared, CliError> {
    let Parameters {
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
    } = parameters;
    let observations =
        read_observations(&input_path, &global_predictor_name, &spatial_predictor_name)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(
        repository,
        "marklab_pymc_spatial_varying_coefficient_worker.py",
    )?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let identity = SpatialCoefficientInputIdentity {
        path: input_path.display().to_string(),
        observations_sha256: json_sha256(&request.observations)?,
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
) -> Result<marklab_bayes::SpatialVaryingCoefficientFit, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
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
) -> Result<Vec<SpatialCoefficientObservation>, CliError> {
    crate::input_file::validate_regular_file(
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
        return Err(CliError::Input(format!(
            "spatial coefficient headers must be exactly: coordinate_id,x_um,y,{global_predictor_name},{spatial_predictor_name}"
        )));
    }
    let mut observations = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize, field: &str| {
            record[index].parse::<f64>().map_err(|_| {
                CliError::Input(format!("spatial coefficient {field} must be numeric"))
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
            return Err(CliError::Input(
                "spatial coefficient observation count exceeds 64".into(),
            ));
        }
    }
    Ok(observations)
}
