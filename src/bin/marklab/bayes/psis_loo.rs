use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, PointwiseLogLikelihoodDraw, PsisLooInputIdentity, PsisLooSpec,
    PsisLooWorkerRequest, PsisLooWorkerResult,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogLikelihoodRow {
    chain: u32,
    draw: u32,
    unit_id: String,
    log_likelihood: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    model_name: String,
    likelihood_target: String,
    data_identity_sha256: String,
    preprocessing_identity_sha256: String,
    heldout_unit: String,
    relative_efficiency: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let draws = read_log_likelihood(&input_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_arviz_psis_loo_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = PsisLooWorkerRequest::new(
        PsisLooSpec {
            model_name,
            likelihood_target,
            data_identity_sha256,
            preprocessing_identity_sha256,
            heldout_unit,
            relative_efficiency,
            draws,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input = PsisLooInputIdentity {
        path: input_path.display().to_string(),
        log_likelihood_value_count: request.log_likelihood.len(),
        log_likelihood_sha256: sha256_hex(&serde_json::to_vec(&request.log_likelihood)?),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_arviz_psis_loo_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: PsisLooWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_result(request, input))
}

fn read_log_likelihood(
    path: &std::path::Path,
) -> Result<Vec<PointwiseLogLikelihoodDraw>, BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "PSIS-LOO input must be a regular file within the 16 MiB limit",
    )?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["chain", "draw", "unit_id", "log_likelihood"])
    {
        return Err(BayesCliError::Input(
            "PSIS-LOO headers must be exactly: chain,draw,unit_id,log_likelihood".into(),
        ));
    }
    let mut draws = Vec::new();
    for row in reader.deserialize::<LogLikelihoodRow>() {
        let row = row?;
        draws.push(PointwiseLogLikelihoodDraw {
            chain: row.chain,
            draw: row.draw,
            unit_id: row.unit_id,
            log_likelihood: row.log_likelihood,
        });
        if draws.len() > 500_000 {
            return Err(BayesCliError::Input(
                "PSIS-LOO log-likelihood matrix exceeds 500000 values".into(),
            ));
        }
    }
    Ok(draws)
}
