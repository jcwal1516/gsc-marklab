use marklab_bayes::sha256_hex;
use marklab_embeddings::analysis::{
    test_embedding_spatial_dependence, EmbeddingEnvelopeError, EmbeddingEnvelopeRow,
    EmbeddingSpatialCurveFunction, EmbeddingSpatialDependenceResult,
    EmbeddingSpatialDependenceSpec,
};
use serde::Serialize;

use super::cli_schema::EmbeddingSpatialDependenceEnvelopeArgs;
use super::{embedding_spatial, publish_json, CliError};

pub(super) fn run(args: EmbeddingSpatialDependenceEnvelopeArgs) -> Result<(), CliError> {
    let EmbeddingSpatialDependenceEnvelopeArgs {
        input,
        bins,
        curve,
        permutations,
        alpha,
        seed,
        maximum_pair_visits,
        out,
    } = args;
    let input_bytes = embedding_spatial::read(&input)?;
    let bin_bytes = embedding_spatial::read(&bins)?;
    let (rows, feature_names) = read_rows(&input_bytes)?;
    let bins_value = embedding_spatial::read_bins(&bin_bytes)?;
    let result = test_embedding_spatial_dependence(EmbeddingSpatialDependenceSpec {
        rows,
        feature_names,
        bins: bins_value,
        curve_function: EmbeddingSpatialCurveFunction::parse(&curve).map_err(map_error)?,
        permutations,
        alpha,
        seed,
        maximum_pair_visits,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            input_sha256: sha256_hex(&input_bytes),
            bins_sha256: sha256_hex(&bin_bytes),
            result,
        },
    )
}

fn read_rows(bytes: &[u8]) -> Result<(Vec<EmbeddingEnvelopeRow>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 6
        || headers.iter().take(4).collect::<Vec<_>>()
            != ["object_id", "permutation_stratum", "x_um", "y_um"]
    {
        return Err(CliError::Input(
            "embedding envelope input requires object_id,permutation_stratum,x_um,y_um and at least two embedding_* columns"
                .into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(4)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize| {
            record[index].parse::<f64>().map_err(|_| {
                CliError::Input(format!(
                    "embedding envelope numeric value in column {} is invalid",
                    &headers[index]
                ))
            })
        };
        rows.push(EmbeddingEnvelopeRow {
            object_id: record[0].to_owned(),
            permutation_stratum: record[1].to_owned(),
            x_um: parse(2)?,
            y_um: parse(3)?,
            embedding: (4..record.len())
                .map(parse)
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((rows, feature_names))
}

fn map_error(error: EmbeddingEnvelopeError) -> CliError {
    match error {
        EmbeddingEnvelopeError::Invalid(message) => CliError::Input(message),
        EmbeddingEnvelopeError::Numeric(message) => CliError::Backend(message),
    }
}

#[derive(Serialize)]
struct Output {
    input_sha256: String,
    bins_sha256: String,
    #[serde(flatten)]
    result: EmbeddingSpatialDependenceResult,
}
