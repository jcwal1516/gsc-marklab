use crate::codecs::embedding_envelope::read_rows;

use marklab_bayes::sha256_hex;
use marklab_embeddings::analysis::{
    test_embedding_spatial_dependence, EmbeddingEnvelopeError, EmbeddingSpatialCurveFunction,
    EmbeddingSpatialDependenceResult, EmbeddingSpatialDependenceSpec,
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
