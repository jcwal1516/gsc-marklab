use std::path::PathBuf;

use marklab_bayes::{
    cross_modal_covariance_by_distance, sha256_hex, CrossModalCovarianceError,
    CrossModalCovarianceResult, CrossModalCovarianceSpec, CrossModalMeanPolicy, CrossModalPair,
    EmbeddingModalityRow,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    a_input: PathBuf,
    b_input: PathBuf,
    pairs: PathBuf,
    bins: PathBuf,
    mean_policy: String,
    permutations: u32,
    seed: u64,
    maximum_pairs: u64,
    maximum_component_pair_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let a_bytes = embedding_spatial::read(&a_input)?;
    let b_bytes = embedding_spatial::read(&b_input)?;
    let pair_bytes = embedding_spatial::read(&pairs)?;
    let bin_bytes = embedding_spatial::read(&bins)?;
    let (a_rows, a_feature_names) = read_modality(&a_bytes)?;
    let (b_rows, b_feature_names) = read_modality(&b_bytes)?;
    let pair_rows = read_pairs(&pair_bytes)?;
    let bins_value = embedding_spatial::read_bins(&bin_bytes)?;
    let result = cross_modal_covariance_by_distance(CrossModalCovarianceSpec {
        a_rows,
        a_feature_names,
        b_rows,
        b_feature_names,
        pairs: pair_rows,
        bins: bins_value,
        mean_policy: CrossModalMeanPolicy::parse(&mean_policy).map_err(map_error)?,
        permutations,
        seed,
        maximum_pairs,
        maximum_component_pair_visits,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            a_input_sha256: sha256_hex(&a_bytes),
            b_input_sha256: sha256_hex(&b_bytes),
            pairs_sha256: sha256_hex(&pair_bytes),
            bins_sha256: sha256_hex(&bin_bytes),
            result,
        },
    )
}

fn read_modality(bytes: &[u8]) -> Result<(Vec<EmbeddingModalityRow>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 4
        || headers.iter().take(3).collect::<Vec<_>>()
            != ["object_id", "source_section", "compartment"]
    {
        return Err(BayesCliError::Input(
            "cross-modal inputs require object_id,source_section,compartment and embedding_* columns"
                .into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(3)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let embedding = (3..record.len())
            .map(|index| {
                record[index].parse::<f64>().map_err(|_| {
                    BayesCliError::Input(format!(
                        "cross-modal numeric value in column {} is invalid",
                        &headers[index]
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(EmbeddingModalityRow {
            object_id: record[0].to_owned(),
            source_section: record[1].to_owned(),
            compartment: record[2].to_owned(),
            embedding,
        });
    }
    Ok((rows, feature_names))
}

fn read_pairs(bytes: &[u8]) -> Result<Vec<CrossModalPair>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>()
        != [
            "a_object_id",
            "b_object_id",
            "source_section",
            "compartment",
            "bin_id",
            "weight",
        ]
    {
        return Err(BayesCliError::Input(
            "cross-modal pair headers differ from the exact contract".into(),
        ));
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        rows.push(CrossModalPair {
            a_object_id: record[0].to_owned(),
            b_object_id: record[1].to_owned(),
            source_section: record[2].to_owned(),
            compartment: record[3].to_owned(),
            bin_id: record[4].to_owned(),
            weight: record[5]
                .parse::<f64>()
                .map_err(|_| BayesCliError::Input("cross-modal pair weight is invalid".into()))?,
        });
    }
    Ok(rows)
}

fn map_error(error: CrossModalCovarianceError) -> BayesCliError {
    match error {
        CrossModalCovarianceError::Invalid(message) => BayesCliError::Input(message),
        CrossModalCovarianceError::Numeric(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct Output {
    a_input_sha256: String,
    b_input_sha256: String,
    pairs_sha256: String,
    bins_sha256: String,
    #[serde(flatten)]
    result: CrossModalCovarianceResult,
}
