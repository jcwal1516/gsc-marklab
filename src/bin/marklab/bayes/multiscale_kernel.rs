use crate::codecs::multiscale_kernel::{read_summaries, read_weights};

use marklab_bayes::sha256_hex;
use marklab_embeddings::analysis::{
    multiscale_embedding_kernel, MultiscaleBaseKernel, MultiscaleEmbeddingKernelResult,
    MultiscaleEmbeddingKernelSpec, MultiscaleKernelError,
};
use serde::Serialize;

use super::cli_schema::MultiscaleEmbeddingKernelArgs;
use super::{embedding_spatial, publish_json, CliError};

pub(super) fn run(args: MultiscaleEmbeddingKernelArgs) -> Result<(), CliError> {
    let MultiscaleEmbeddingKernelArgs {
        input,
        weights,
        sample_a,
        sample_b,
        base_kernel,
        kernel_scale,
        maximum_component_scale_visits,
        out,
    } = args;
    let input_bytes = embedding_spatial::read(&input)?;
    let weight_bytes = embedding_spatial::read(&weights)?;
    let (summaries, feature_names) = read_summaries(&input_bytes)?;
    let weights = read_weights(&weight_bytes)?;
    let result = multiscale_embedding_kernel(MultiscaleEmbeddingKernelSpec {
        summaries,
        feature_names,
        weights,
        sample_a,
        sample_b,
        base_kernel: MultiscaleBaseKernel::parse(&base_kernel).map_err(map_error)?,
        kernel_scale,
        maximum_component_scale_visits,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            input_sha256: sha256_hex(&input_bytes),
            weights_sha256: sha256_hex(&weight_bytes),
            result,
        },
    )
}

fn map_error(error: MultiscaleKernelError) -> CliError {
    match error {
        MultiscaleKernelError::Invalid(message) => CliError::Input(message),
        MultiscaleKernelError::Numeric => CliError::Backend(error.to_string()),
    }
}

#[derive(Serialize)]
struct Output {
    input_sha256: String,
    weights_sha256: String,
    #[serde(flatten)]
    result: MultiscaleEmbeddingKernelResult,
}
