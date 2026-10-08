use std::path::PathBuf;

use marklab_bayes::{
    kernel_mark_correlation, sha256_hex, EmbeddingKernelError, EmbeddingKernelKind,
    EmbeddingKernelSpec, KernelEmbeddingRow, KernelMarkCorrelationResult,
    KernelMarkCorrelationSpec,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, BayesCliError};

pub(super) fn run(
    input: PathBuf,
    bins: PathBuf,
    kernel: String,
    global_reference_tolerance: f64,
    maximum_pair_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
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

fn read_rows(bytes: &[u8]) -> Result<(Vec<KernelEmbeddingRow>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 7
        || headers.iter().take(5).collect::<Vec<_>>()
            != ["object_id", "biological_unit", "split", "x_um", "y_um"]
    {
        return Err(BayesCliError::Input(
            "kernel input requires object_id,biological_unit,split,x_um,y_um and at least two embedding_* columns"
                .into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(5)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize| {
            record[index].parse::<f64>().map_err(|_| {
                BayesCliError::Input(format!(
                    "kernel numeric value in column {} is invalid",
                    &headers[index]
                ))
            })
        };
        rows.push(KernelEmbeddingRow {
            object_id: record[0].to_owned(),
            biological_unit: record[1].to_owned(),
            split: record[2].to_owned(),
            x_um: parse(3)?,
            y_um: parse(4)?,
            embedding: (5..record.len())
                .map(parse)
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((rows, feature_names))
}

fn map_error(error: EmbeddingKernelError) -> BayesCliError {
    match error {
        EmbeddingKernelError::Invalid(message) => BayesCliError::Input(message),
        EmbeddingKernelError::Numeric(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct Output {
    input_sha256: String,
    bins_sha256: String,
    #[serde(flatten)]
    result: KernelMarkCorrelationResult,
}
