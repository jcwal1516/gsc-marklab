use marklab_bayes::{
    json_sha256, sha256_hex, MetaAnalysisInputIdentity, MetaAnalysisSpec,
    MetaAnalysisWorkerRequest, MetaAnalysisWorkerResult, SiteEstimate,
};
use serde::Deserialize;

use super::cli_schema::MetaAnalysisArgs;
use super::worker_identity;
use super::{publish_json, run_worker, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteRow {
    site_id: String,
    effect: f64,
    standard_error: f64,
    covariate: f64,
}

pub(super) fn run(args: MetaAnalysisArgs) -> Result<(), CliError> {
    let MetaAnalysisArgs {
        input,
        covariate_name,
        new_site_covariate,
        global_prior_mean,
        global_prior_sd,
        covariate_prior_sd,
        heterogeneity_prior_sd,
        timeout_seconds,
        out,
        nuts,
    } = args;
    let input_path = input;
    let sampling = nuts.spec();
    let output_path = out;
    let sites = read_sites(&input_path)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_pymc_meta_analysis_worker.py")?;
    let request = MetaAnalysisWorkerRequest::new(
        MetaAnalysisSpec {
            covariate_name,
            new_site_covariate,
            global_prior_mean,
            global_prior_sd,
            covariate_prior_sd,
            heterogeneity_prior_sd,
            sites,
        },
        sampling,
        identity.environment_lock_sha256,
        identity.worker_sha256,
        timeout_seconds,
    )?;
    let input = MetaAnalysisInputIdentity {
        path: input_path.display().to_string(),
        sites: request.sites.len(),
        site_data_sha256: json_sha256(&request.sites)?,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_meta_analysis_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: MetaAnalysisWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, input))
}

fn read_sites(path: &std::path::Path) -> Result<Vec<SiteEstimate>, CliError> {
    crate::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "meta-analysis input must be a regular file within the 16 MiB limit",
    )?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["site_id", "effect", "standard_error", "covariate"])
    {
        return Err(CliError::Input(
            "meta-analysis CSV headers must be exactly: site_id,effect,standard_error,covariate"
                .into(),
        ));
    }
    let mut sites = Vec::new();
    for row in reader.deserialize::<SiteRow>() {
        let row = row?;
        sites.push(SiteEstimate {
            site_id: row.site_id,
            effect: row.effect,
            standard_error: row.standard_error,
            covariate: row.covariate,
        });
        if sites.len() > 10_000 {
            return Err(CliError::Input("site count exceeds 10000".into()));
        }
    }
    Ok(sites)
}
