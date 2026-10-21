use marklab_bayes::{
    apply_abstention, mahalanobis_ood_score, AbstentionDecision, AbstentionPolicy,
    MahalanobisOodResult, MahalanobisOodSpec, OodRepresentationUnit, PredictionForAbstention,
    PredictionSafetyError,
};
use serde::{Deserialize, Serialize};

use super::cli_schema::ApplyAbstentionArgs;
use super::cli_schema::OodScoreArgs;
use super::{embedding_spatial, publish_json, CliError};

#[derive(Debug, Deserialize)]
struct InputRow {
    prediction_id: String,
    prediction: f64,
    uncertainty: f64,
    ood_score: f64,
}

pub(super) fn run_abstention(args: ApplyAbstentionArgs) -> Result<(), CliError> {
    let ApplyAbstentionArgs {
        input,
        maximum_uncertainty,
        maximum_ood_score,
        out,
    } = args;
    let bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
    if !reader
        .headers()?
        .iter()
        .eq(["prediction_id", "prediction", "uncertainty", "ood_score"])
    {
        return Err(CliError::Input("abstention CSV header differs".into()));
    }
    let policy = AbstentionPolicy {
        maximum_uncertainty,
        maximum_ood_score,
    };
    let decisions = reader
        .deserialize::<InputRow>()
        .map(|row| {
            let row = row?;
            apply_abstention(
                PredictionForAbstention {
                    prediction_id: row.prediction_id,
                    prediction: row.prediction,
                    uncertainty: row.uncertainty,
                    ood_score: row.ood_score,
                },
                policy,
            )
            .map_err(map)
        })
        .collect::<Result<Vec<_>, CliError>>()?;
    if decisions.is_empty() {
        return Err(CliError::Input(
            "abstention input must contain at least one prediction".into(),
        ));
    }
    publish_json(
        &out,
        &Output {
            format: "marklab.abstention_decisions",
            version: 1,
            input_sha256: marklab_bayes::sha256_hex(&bytes),
            policy_source: "prespecified_validation_policy",
            maximum_uncertainty,
            maximum_ood_score,
            decisions,
        },
    )
}

pub(super) fn run_ood(args: OodScoreArgs) -> Result<(), CliError> {
    let OodScoreArgs {
        input,
        shrinkage,
        validation_quantile,
        out,
    } = args;
    let bytes = embedding_spatial::read(&input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
    let headers = reader.headers()?.clone();
    if headers.len() < 5
        || headers.iter().take(3).collect::<Vec<_>>() != ["unit_id", "split", "domain"]
    {
        return Err(CliError::Input("OOD CSV header differs".into()));
    }
    let feature_names = headers
        .iter()
        .skip(3)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut units = Vec::new();
    for row in reader.records() {
        let row = row?;
        units.push(OodRepresentationUnit {
            unit_id: row[0].into(),
            split: row[1].into(),
            domain: row[2].into(),
            representation: row
                .iter()
                .skip(3)
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| CliError::Input("OOD representation is invalid".into()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    let result = mahalanobis_ood_score(MahalanobisOodSpec {
        units,
        feature_names,
        shrinkage,
        validation_quantile,
    })
    .map_err(map)?;
    publish_json(
        &out,
        &OodOutput {
            input_sha256: marklab_bayes::sha256_hex(&bytes),
            result,
        },
    )
}

fn map(error: PredictionSafetyError) -> CliError {
    CliError::Input(error.to_string())
}

#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    input_sha256: String,
    policy_source: &'static str,
    maximum_uncertainty: f64,
    maximum_ood_score: f64,
    decisions: Vec<AbstentionDecision>,
}

#[derive(Serialize)]
struct OodOutput {
    input_sha256: String,
    #[serde(flatten)]
    result: MahalanobisOodResult,
}
