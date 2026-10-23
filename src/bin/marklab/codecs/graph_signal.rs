use marklab_graph::{GraphSignalEdge, GraphSignalNode, StratifiedGraphSignalNode};

use crate::cli_error::CliError;

pub(crate) fn read_nodes(bytes: &[u8]) -> Result<(Vec<GraphSignalNode>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 2 || headers.get(0) != Some("node_id") {
        return Err(CliError::Input(
            "graph signal nodes require node_id and signal_* columns".into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut nodes = Vec::new();
    for record in reader.records() {
        let record = record?;
        let signal = (1..record.len())
            .map(|index| {
                record[index].parse::<f64>().map_err(|_| {
                    CliError::Input(format!(
                        "graph signal value in column {} is invalid",
                        &headers[index]
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes.push(GraphSignalNode {
            node_id: record[0].to_owned(),
            signal,
        });
    }
    Ok((nodes, feature_names))
}

pub(crate) fn read_stratified_nodes(
    bytes: &[u8],
) -> Result<(Vec<StratifiedGraphSignalNode>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 3
        || headers.iter().take(2).collect::<Vec<_>>() != ["node_id", "permutation_stratum"]
    {
        return Err(CliError::Input(
            "smoothness nodes require node_id,permutation_stratum and signal_* columns".into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(2)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut nodes = Vec::new();
    for record in reader.records() {
        let record = record?;
        let signal = (2..record.len())
            .map(|index| {
                record[index].parse::<f64>().map_err(|_| {
                    CliError::Input(format!(
                        "smoothness signal value in column {} is invalid",
                        &headers[index]
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes.push(StratifiedGraphSignalNode {
            node_id: record[0].to_owned(),
            permutation_stratum: record[1].to_owned(),
            signal,
        });
    }
    Ok((nodes, feature_names))
}

pub(crate) fn read_edges(bytes: &[u8]) -> Result<Vec<GraphSignalEdge>, CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["left_node_id", "right_node_id", "weight"] {
        return Err(CliError::Input(
            "graph edge headers must be left_node_id,right_node_id,weight".into(),
        ));
    }
    let mut edges = Vec::new();
    for record in reader.records() {
        let record = record?;
        edges.push(GraphSignalEdge {
            left_node_id: record[0].to_owned(),
            right_node_id: record[1].to_owned(),
            weight: record[2]
                .parse::<f64>()
                .map_err(|_| CliError::Input("graph edge weight is invalid".into()))?,
        });
    }
    Ok(edges)
}
