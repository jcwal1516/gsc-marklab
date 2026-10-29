use crate::codecs::embedding_kernel::read_rows;

use marklab_bayes::sha256_hex;
use marklab_embeddings::analysis::{
    kernel_mark_correlation, EmbeddingKernelError, EmbeddingKernelKind, EmbeddingKernelSpec,
    KernelMarkCorrelationResult, KernelMarkCorrelationSpec,
};
use serde::Serialize;

use super::cli_schema::KernelMarkCorrelationArgs;
use super::{embedding_spatial, publish_json, CliError};

pub(super) fn run(args: KernelMarkCorrelationArgs) -> Result<(), CliError> {
    let KernelMarkCorrelationArgs {
        input,
        bins,
        kernel,
        global_reference_tolerance,
        maximum_pair_visits,
        out,
    } = args;
    let input_bytes = embedding_spatial::read(&input)?;
    let bin_bytes = embedding_spatial::read(&bins)?;
    let (rows, feature_names) = read_rows(&input_bytes)?;
    let bins_value = embedding_spatial::read_bins(&bin_bytes)?;
    let result = kernel_mark_correlation(KernelMarkCorrelationSpec {
        kernel_spec: EmbeddingKernelSpec {
            rows,
            feature_names,
            kind: EmbeddingKernelKind::parse(&kernel).map_err(map_error)?,
        },
        bins: bins_value,
        global_reference_tolerance,
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

fn map_error(error: EmbeddingKernelError) -> CliError {
    match error {
        EmbeddingKernelError::Invalid(message) => CliError::Input(message),
        EmbeddingKernelError::Numeric(message) => CliError::Backend(message),
    }
}

#[derive(Serialize)]
struct Output {
    input_sha256: String,
    bins_sha256: String,
    #[serde(flatten)]
    result: KernelMarkCorrelationResult,
}
