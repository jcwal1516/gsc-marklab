use crate::codecs::graph_signal::{read_edges, read_nodes, read_stratified_nodes};

use marklab_bayes::sha256_hex;
use marklab_graph::{
    graph_dirichlet_energy, graph_smoothness_permutation_test, local_embedding_roughness,
    GraphDirichletEnergyResult, GraphDirichletEnergySpec, GraphEnergyNormalization, GraphLaplacian,
    GraphSignalError, GraphSmoothnessPermutationResult, GraphSmoothnessPermutationSpec,
    LocalEmbeddingRoughnessResult, LocalEmbeddingRoughnessSpec,
};
use serde::Serialize;

use super::cli_schema::GraphDirichletEnergyArgs;
use super::cli_schema::GraphSmoothnessPermutationTestArgs;
use super::cli_schema::LocalEmbeddingRoughnessArgs;
use super::{embedding_spatial, publish_json, CliError};

pub(super) fn run_energy(args: GraphDirichletEnergyArgs) -> Result<(), CliError> {
    let GraphDirichletEnergyArgs {
        nodes,
        edges,
        laplacian,
        normalization,
        maximum_component_edge_visits,
        out,
    } = args;
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

pub(super) fn run_local(args: LocalEmbeddingRoughnessArgs) -> Result<(), CliError> {
    let LocalEmbeddingRoughnessArgs {
        nodes,
        edges,
        epsilon,
        maximum_component_edge_visits,
        out,
    } = args;
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

pub(super) fn run_smoothness(args: GraphSmoothnessPermutationTestArgs) -> Result<(), CliError> {
    let GraphSmoothnessPermutationTestArgs {
        nodes,
        edges,
        laplacian,
        permutations,
        seed,
        maximum_component_edge_visits,
        out,
    } = args;
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

fn map_error(error: GraphSignalError) -> CliError {
    match error {
        GraphSignalError::Invalid(message) => CliError::Input(message),
        GraphSignalError::Numeric(message) => CliError::Backend(message),
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
