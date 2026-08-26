use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, MetaAnalysisInputIdentity, MetaAnalysisSpec, MetaAnalysisWorkerRequest,
    MetaAnalysisWorkerResult, NutsSamplingSpec, SiteEstimate,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteRow {
    site_id: String,
    effect: f64,
    standard_error: f64,
    covariate: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    covariate_name: String,
    new_site_covariate: f64,
    global_prior_mean: f64,
    global_prior_sd: f64,
    covariate_prior_sd: f64,
    heterogeneity_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let sites = read_sites(&input_path)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_meta_analysis_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
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
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let input = MetaAnalysisInputIdentity {
        path: input_path.display().to_string(),
        sites: request.sites.len(),
        site_data_sha256: sha256_hex(&serde_json::to_vec(&request.sites)?),
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

fn read_sites(path: &std::path::Path) -> Result<Vec<SiteEstimate>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "meta-analysis input must be a regular file within the 16 MiB limit".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["site_id", "effect", "standard_error", "covariate"])
    {
        return Err(BayesCliError::Input(
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
            return Err(BayesCliError::Input("site count exceeds 10000".into()));
        }
    }
    Ok(sites)
}
