use super::{publish_json, BayesCliError};
use marklab_bayes::{
    replicated_hierarchical_lgcp, sha256_hex, ReplicatedError, ReplicatedFieldPolicy,
    ReplicatedLgcpPattern, ReplicatedLgcpSpec,
};
use serde::Serialize;
use std::{fs, path::PathBuf};
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input: PathBuf,
    policy: String,
    gps: f64,
    pips: f64,
    amplitude: f64,
    length: f64,
    rep_amp: f64,
    jitter: f64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let bytes = fs::read(&input).map_err(|source| BayesCliError::Io {
        path: input.clone(),
        source,
    })?;
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes.as_slice());
    if r.headers()?.iter().collect::<Vec<_>>()
        != [
            "pattern_id",
            "patient_id",
            "window_sha256",
            "grid_sha256",
            "covariate_sha256",
            "event_count",
            "cell_count",
        ]
    {
        return Err(BayesCliError::Input(
            "replicated LGCP headers differ".into(),
        ));
    }
    let patterns = r
        .deserialize()
        .collect::<Result<Vec<ReplicatedLgcpPattern>, _>>()?;
    let m = replicated_hierarchical_lgcp(ReplicatedLgcpSpec {
        patterns,
        policy: ReplicatedFieldPolicy::parse(&policy).map_err(map_error)?,
        global_prior_sd: gps,
        patient_intercept_prior_sd: pips,
        population_field_amplitude: amplitude,
        population_field_length_scale_um: length,
        replicate_field_amplitude: rep_amp,
        jitter,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.replicated_hierarchical_lgcp_model",
            version: 1,
            input_sha256: sha256_hex(&bytes),
            fit_state: "not_fitted",
            model: m.model,
            patient_count: m.patient_count,
            pattern_count: m.pattern_count,
            total_event_count: m.total_event_count,
            patterns: m.patterns,
        },
    )
}
fn map_error(e: ReplicatedError) -> BayesCliError {
    let ReplicatedError::Invalid(m) = e;
    BayesCliError::Input(m)
}
#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    input_sha256: String,
    fit_state: &'static str,
    model: marklab_bayes::ReplicatedLgcpModelIr,
    patient_count: u32,
    pattern_count: u32,
    total_event_count: u64,
    patterns: Vec<ReplicatedLgcpPattern>,
}
