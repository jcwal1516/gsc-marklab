use marklab_bayes::{
    json_sha256, sha256_hex, PointwiseLogLikelihoodDraw, PsisLooInputIdentity, PsisLooSpec,
    PsisLooWorkerRequest, PsisLooWorkerResult,
};
use serde::Deserialize;

use super::cli_schema::PsisLooArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogLikelihoodRow {
    chain: u32,
    draw: u32,
    unit_id: String,
    log_likelihood: f64,
}

pub(super) fn run(args: PsisLooArgs) -> Result<(), CliError> {
    let PsisLooArgs {
        input,
        model_name,
        likelihood_target,
        data_identity_sha256,
        preprocessing_identity_sha256,
        heldout_unit,
        relative_efficiency,
        timeout_seconds,
        out,
    } = args;
    let input_path = input;
    let output_path = out;
    let draws = read_log_likelihood(&input_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_arviz_psis_loo_worker.py")?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input = PsisLooInputIdentity {
        path: input_path.display().to_string(),
        log_likelihood_value_count: request.log_likelihood.len(),
        log_likelihood_sha256: json_sha256(&request.log_likelihood)?,
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
) -> Result<Vec<PointwiseLogLikelihoodDraw>, CliError> {
    crate::input_file::validate_regular_file(
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
        return Err(CliError::Input(
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
            return Err(CliError::Input(
                "PSIS-LOO log-likelihood matrix exceeds 500000 values".into(),
            ));
        }
    }
    Ok(draws)
}
