use marklab_embeddings::analysis::KernelEmbeddingRow;

use crate::cli_error::CliError;

pub(crate) fn read_rows(bytes: &[u8]) -> Result<(Vec<KernelEmbeddingRow>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 7
        || headers.iter().take(5).collect::<Vec<_>>()
            != ["object_id", "biological_unit", "split", "x_um", "y_um"]
    {
        return Err(CliError::Input(
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
                CliError::Input(format!(
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
