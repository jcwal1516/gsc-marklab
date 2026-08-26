use std::path::PathBuf;

use marklab_bayes::{
    multiscale_embedding_kernel, sha256_hex, MultiscaleBaseKernel, MultiscaleEmbeddingKernelResult,
    MultiscaleEmbeddingKernelSpec, MultiscaleEmbeddingSummary, MultiscaleKernelError,
    MultiscaleKernelWeight,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input: PathBuf,
    weights: PathBuf,
    sample_a: String,
    sample_b: String,
    base_kernel: String,
    kernel_scale: Option<f64>,
    maximum_component_scale_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
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

fn read_summaries(
    bytes: &[u8],
) -> Result<(Vec<MultiscaleEmbeddingSummary>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 3 || headers.iter().take(2).collect::<Vec<_>>() != ["sample_id", "scale_um"]
    {
        return Err(BayesCliError::Input(
            "multiscale summaries require sample_id,scale_um and embedding_* columns".into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(2)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        rows.push(MultiscaleEmbeddingSummary {
            sample_id: record[0].to_owned(),
            scale_um: record[1]
                .parse()
                .map_err(|_| BayesCliError::Input("summary scale is invalid".into()))?,
            embedding: (2..record.len())
                .map(|index| {
                    record[index].parse::<f64>().map_err(|_| {
                        BayesCliError::Input(format!(
                            "summary value {} is invalid",
                            &headers[index]
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((rows, feature_names))
}

fn read_weights(bytes: &[u8]) -> Result<Vec<MultiscaleKernelWeight>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["scale_um", "weight"] {
        return Err(BayesCliError::Input(
            "multiscale weight headers must be scale_um,weight".into(),
        ));
    }
    reader
        .records()
        .map(|record| {
            let record = record?;
            Ok(MultiscaleKernelWeight {
                scale_um: record[0]
                    .parse()
                    .map_err(|_| BayesCliError::Input("weight scale is invalid".into()))?,
                weight: record[1]
                    .parse()
                    .map_err(|_| BayesCliError::Input("scale weight is invalid".into()))?,
            })
        })
        .collect()
}

fn map_error(error: MultiscaleKernelError) -> BayesCliError {
    match error {
        MultiscaleKernelError::Invalid(message) => BayesCliError::Input(message),
        MultiscaleKernelError::Numeric => BayesCliError::Backend(error.to_string()),
    }
}

#[derive(Serialize)]
struct Output {
    input_sha256: String,
    weights_sha256: String,
    #[serde(flatten)]
    result: MultiscaleEmbeddingKernelResult,
}
