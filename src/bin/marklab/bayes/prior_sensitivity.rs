use std::{io::Cursor, path::PathBuf};

use marklab_bayes::{
    evaluate_normal_mean_prior_sensitivity, sha256_hex, NormalMeanPriorSensitivityResult,
    NormalMeanPriorSensitivitySpec, NormalPriorAlternative, PriorSensitivityInputIdentity,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation: f64,
}

#[derive(Serialize)]
struct PriorSensitivityOutput {
    #[serde(flatten)]
    analysis: NormalMeanPriorSensitivityResult,
    input: PriorSensitivityInputIdentity,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    priors_path: PathBuf,
    base_prior: String,
    known_sigma: f64,
    decision_threshold: f64,
    decision_probability_threshold: f64,
    material_mean_shift: f64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_observations(&input_path)?;
    let mut priors = read_priors(&priors_path)?;
    priors.sort_by(|left, right| left.prior_name.cmp(&right.prior_name));
    let input = PriorSensitivityInputIdentity {
        observations_path: input_path.display().to_string(),
        priors_path: priors_path.display().to_string(),
        observations_sha256: sha256_hex(&serde_json::to_vec(&observations)?),
        priors_sha256: sha256_hex(&serde_json::to_vec(&priors)?),
    };
    let analysis = evaluate_normal_mean_prior_sensitivity(
        NormalMeanPriorSensitivitySpec {
            observations,
            priors,
            base_prior,
            known_sigma,
            decision_threshold,
            decision_probability_threshold,
            material_mean_shift,
        },
        timeout_seconds,
    )?;
    let output = PriorSensitivityOutput { analysis, input };
    if serde_json::to_vec_pretty(&output)?.len() + 1 > 1_048_576 {
        return Err(BayesCliError::Input(
            "prior-sensitivity output exceeds the 1 MiB limit".into(),
        ));
    }
    publish_json(&output_path, &output)
}

fn reader(path: &std::path::Path) -> Result<csv::Reader<Cursor<Vec<u8>>>, BayesCliError> {
    let bytes = super::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "prior-sensitivity inputs must be regular files within the 16 MiB limit",
    )?;
    Ok(csv::ReaderBuilder::new().from_reader(Cursor::new(bytes)))
}

fn read_observations(path: &std::path::Path) -> Result<Vec<f64>, BayesCliError> {
    let mut reader = reader(path)?;
    if !reader.headers()?.iter().eq(["observation"]) {
        return Err(BayesCliError::Input(
            "prior-sensitivity observation header must be exactly: observation".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        observations.push(row?.observation);
        if observations.len() > 100_000 {
            return Err(BayesCliError::Input(
                "prior-sensitivity observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(observations)
}

fn read_priors(path: &std::path::Path) -> Result<Vec<NormalPriorAlternative>, BayesCliError> {
    let mut reader = reader(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["prior_name", "prior_mean", "prior_sd"])
    {
        return Err(BayesCliError::Input(
            "prior-sensitivity prior headers must be exactly: prior_name,prior_mean,prior_sd"
                .into(),
        ));
    }
    let mut priors = Vec::new();
    for row in reader.deserialize::<NormalPriorAlternative>() {
        priors.push(row?);
        if priors.len() > 32 {
            return Err(BayesCliError::Input(
                "prior-sensitivity prior count exceeds 32".into(),
            ));
        }
    }
    Ok(priors)
}
