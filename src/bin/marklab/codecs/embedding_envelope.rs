use marklab_embeddings::analysis::EmbeddingEnvelopeRow;

use crate::cli_error::CliError;

pub(crate) fn read_rows(
    bytes: &[u8],
) -> Result<(Vec<EmbeddingEnvelopeRow>, Vec<String>), CliError> {
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
