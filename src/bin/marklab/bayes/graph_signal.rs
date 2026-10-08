use std::path::PathBuf;

use marklab_bayes::{
    graph_dirichlet_energy, graph_smoothness_permutation_test, local_embedding_roughness,
    sha256_hex, GraphDirichletEnergyResult, GraphDirichletEnergySpec, GraphEnergyNormalization,
    GraphLaplacian, GraphSignalEdge, GraphSignalError, GraphSignalNode,
    GraphSmoothnessPermutationResult, GraphSmoothnessPermutationSpec,
    LocalEmbeddingRoughnessResult, LocalEmbeddingRoughnessSpec, StratifiedGraphSignalNode,
};
use serde::Serialize;

use super::{embedding_spatial, publish_json, BayesCliError};

pub(super) fn run_energy(
    nodes: PathBuf,
    edges: PathBuf,
    laplacian: String,
    normalization: String,
    maximum_component_edge_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let node_bytes = embedding_spatial::read(&nodes)?;
    let edge_bytes = embedding_spatial::read(&edges)?;
    let (node_rows, feature_names) = read_nodes(&node_bytes)?;
    let edge_rows = read_edges(&edge_bytes)?;
    let result = graph_dirichlet_energy(GraphDirichletEnergySpec {
        nodes: node_rows,
        feature_names,
        edges: edge_rows,
        laplacian: GraphLaplacian::parse(&laplacian).map_err(map_error)?,
        normalization: GraphEnergyNormalization::parse(&normalization).map_err(map_error)?,
        maximum_component_edge_visits,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &EnergyOutput {
            nodes_sha256: sha256_hex(&node_bytes),
            edges_sha256: sha256_hex(&edge_bytes),
            result,
        },
    )
}

pub(super) fn run_local(
    nodes: PathBuf,
    edges: PathBuf,
    epsilon: f64,
    maximum_component_edge_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let node_bytes = embedding_spatial::read(&nodes)?;
    let edge_bytes = embedding_spatial::read(&edges)?;
    let (node_rows, feature_names) = read_nodes(&node_bytes)?;
    let edge_rows = read_edges(&edge_bytes)?;
    let result = local_embedding_roughness(LocalEmbeddingRoughnessSpec {
        nodes: node_rows,
        feature_names,
        edges: edge_rows,
        epsilon,
        maximum_component_edge_visits,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &LocalOutput {
            nodes_sha256: sha256_hex(&node_bytes),
            edges_sha256: sha256_hex(&edge_bytes),
            result,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_smoothness(
    nodes: PathBuf,
    edges: PathBuf,
    laplacian: String,
    permutations: u32,
    seed: u64,
    maximum_component_edge_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let node_bytes = embedding_spatial::read(&nodes)?;
    let edge_bytes = embedding_spatial::read(&edges)?;
    let (node_rows, feature_names) = read_stratified_nodes(&node_bytes)?;
    let edge_rows = read_edges(&edge_bytes)?;
    let result = graph_smoothness_permutation_test(GraphSmoothnessPermutationSpec {
        nodes: node_rows,
        feature_names,
        edges: edge_rows,
        laplacian: GraphLaplacian::parse(&laplacian).map_err(map_error)?,
        permutations,
        seed,
        maximum_component_edge_visits,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &SmoothnessOutput {
            nodes_sha256: sha256_hex(&node_bytes),
            edges_sha256: sha256_hex(&edge_bytes),
            result,
        },
    )
}

fn read_nodes(bytes: &[u8]) -> Result<(Vec<GraphSignalNode>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 2 || headers.get(0) != Some("node_id") {
        return Err(BayesCliError::Input(
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
                    BayesCliError::Input(format!(
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

fn read_stratified_nodes(
    bytes: &[u8],
) -> Result<(Vec<StratifiedGraphSignalNode>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 3
        || headers.iter().take(2).collect::<Vec<_>>() != ["node_id", "permutation_stratum"]
    {
        return Err(BayesCliError::Input(
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
                    BayesCliError::Input(format!(
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

fn read_edges(bytes: &[u8]) -> Result<Vec<GraphSignalEdge>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["left_node_id", "right_node_id", "weight"] {
        return Err(BayesCliError::Input(
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
                .map_err(|_| BayesCliError::Input("graph edge weight is invalid".into()))?,
        });
    }
    Ok(edges)
}

fn map_error(error: GraphSignalError) -> BayesCliError {
    match error {
        GraphSignalError::Invalid(message) => BayesCliError::Input(message),
        GraphSignalError::Numeric(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct EnergyOutput {
    nodes_sha256: String,
    edges_sha256: String,
    #[serde(flatten)]
    result: GraphDirichletEnergyResult,
}

#[derive(Serialize)]
struct SmoothnessOutput {
    nodes_sha256: String,
    edges_sha256: String,
    #[serde(flatten)]
    result: GraphSmoothnessPermutationResult,
}

#[derive(Serialize)]
struct LocalOutput {
    nodes_sha256: String,
    edges_sha256: String,
    #[serde(flatten)]
    result: LocalEmbeddingRoughnessResult,
}
