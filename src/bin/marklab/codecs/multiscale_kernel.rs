use marklab_embeddings::analysis::{MultiscaleEmbeddingSummary, MultiscaleKernelWeight};

use crate::cli_error::CliError;

pub(crate) fn read_summaries(
    bytes: &[u8],
) -> Result<(Vec<MultiscaleEmbeddingSummary>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 3 || headers.iter().take(2).collect::<Vec<_>>() != ["sample_id", "scale_um"]
    {
        return Err(CliError::Input(
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
                .map_err(|_| CliError::Input("summary scale is invalid".into()))?,
            embedding: (2..record.len())
                .map(|index| {
                    record[index].parse::<f64>().map_err(|_| {
                        CliError::Input(format!("summary value {} is invalid", &headers[index]))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((rows, feature_names))
}

pub(crate) fn read_weights(bytes: &[u8]) -> Result<Vec<MultiscaleKernelWeight>, CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["scale_um", "weight"] {
        return Err(CliError::Input(
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
                    .map_err(|_| CliError::Input("weight scale is invalid".into()))?,
                weight: record[1]
                    .parse()
                    .map_err(|_| CliError::Input("scale weight is invalid".into()))?,
            })
        })
        .collect()
}
