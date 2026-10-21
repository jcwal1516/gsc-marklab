use std::{borrow::Cow, io, path::Path};

use marklab_graph::{
    local_embedding_roughness, summarize_graph_signal_input, GraphSignalEdge, GraphSignalNode,
    LocalEmbeddingRoughnessSpec,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{LocalEmbeddingRoughnessLimits, LocalEmbeddingRoughnessProjectArgs};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};
use marklab_bayes::ClaimStatus;

const NODES_KIND: &str = "application/vnd.marklab.source.graph-signal-nodes-csv;version=1";
const EDGES_KIND: &str = "application/vnd.marklab.source.graph-signal-edges-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.local-embedding-roughness+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-local-embedding-roughness-node-v1";

pub(super) fn run(args: LocalEmbeddingRoughnessProjectArgs) -> Result<(), CliError> {
    let LocalEmbeddingRoughnessProjectArgs {
        project: project_path,
        nodes: nodes_path,
        edges: edges_path,
        epsilon,
        limits,
        out: output_path,
    } = args;
    let LocalEmbeddingRoughnessLimits {
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "local embedding roughness sources";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if !epsilon.is_finite()
        || epsilon <= 0.0
        || maximum_nodes < 2
        || maximum_edges == 0
        || maximum_dimension == 0
        || maximum_component_edge_visits == 0
        || memory_bytes == 0
    {
        return Err(CliError::Input(
            "local roughness epsilon and resource limits must be positive and admit at least two nodes"
                .into(),
        ));
    }

    let node_bytes = read_bounded(&nodes_path, memory_bytes, "node")?;
    let edge_bytes = read_bounded(&edges_path, memory_bytes, "edge")?;
    let source_bytes = node_bytes
        .len()
        .checked_add(edge_bytes.len())
        .ok_or_else(|| CliError::Input("graph source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(CliError::Input(
            "graph source files exceed the retained-memory budget".into(),
        ));
    }
    let sources = vec![
        Source::from_bytes(nodes_path, NODES_KIND, &node_bytes)?,
        Source::from_bytes(edges_path, EDGES_KIND, &edge_bytes)?,
    ];
    let (nodes, feature_names) = read_nodes(&node_bytes)?;
    let edges = read_edges(&edge_bytes)?;
    let requirements = preflight(
        &nodes,
        &feature_names,
        &edges,
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
    )?;
    let retained = retained_bytes(source_bytes, nodes.len(), feature_names.len(), edges.len())?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "local embedding roughness retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;

    let nodes_sha256 = sources[0].artifact().digest().to_string();
    let edges_sha256 = sources[1].artifact().digest().to_string();
    let (analysis, contract) = LocalEmbeddingRoughness::new(
        LocalEmbeddingRoughnessSpec {
            nodes,
            feature_names,
            edges,
            epsilon,
            maximum_component_edge_visits,
        },
        requirements,
        nodes_sha256,
        edges_sha256,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        LocalEmbeddingRoughness::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.local_embedding_roughness", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "local-embedding-roughness",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn read_bounded(path: &Path, memory_bytes: usize, label: &str) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "graph {label} source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

fn read_nodes(bytes: &[u8]) -> Result<(Vec<GraphSignalNode>, Vec<String>), CliError> {
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

fn read_edges(bytes: &[u8]) -> Result<Vec<GraphSignalEdge>, CliError> {
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

struct Requirements {
    component_edge_visits: u64,
    graph_digest: String,
    sorted_node_ids: Vec<String>,
    neighbor_counts: Vec<u32>,
}

fn preflight(
    nodes: &[GraphSignalNode],
    feature_names: &[String],
    edges: &[GraphSignalEdge],
    maximum_nodes: usize,
    maximum_edges: usize,
    maximum_dimension: usize,
    maximum_component_edge_visits: u64,
) -> Result<Requirements, CliError> {
    if nodes.len() > maximum_nodes {
        return Err(CliError::Input(format!(
            "node count exceeds maximum_nodes {maximum_nodes}"
        )));
    }
    if edges.len() > maximum_edges {
        return Err(CliError::Input(format!(
            "edge count exceeds maximum_edges {maximum_edges}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(CliError::Input(format!(
            "signal dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let summary = summarize_graph_signal_input(nodes, feature_names, edges)
        .map_err(|error| CliError::Input(error.to_string()))?;

    let component_edge_visits = (edges.len() as u64)
        .checked_mul(feature_names.len() as u64)
        .ok_or_else(|| CliError::Input("local roughness work overflowed".into()))?;
    if component_edge_visits > maximum_component_edge_visits
        || maximum_component_edge_visits > 250_000_000
    {
        return Err(CliError::Input(format!(
            "{component_edge_visits} component-edge visits exceed the declared or fixed resource bound"
        )));
    }

    Ok(Requirements {
        component_edge_visits,
        graph_digest: summary.graph_digest,
        sorted_node_ids: summary.sorted_node_ids,
        neighbor_counts: summary.neighbor_counts,
    })
}

fn retained_bytes(
    source_bytes: usize,
    nodes: usize,
    dimension: usize,
    edges: usize,
) -> Result<usize, CliError> {
    let signal_bytes = nodes
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(2 * std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("graph signal memory overflowed".into()))?;
    source_bytes
        .checked_add(signal_bytes)
        .and_then(|value| value.checked_add(nodes.saturating_mul(768)))
        .and_then(|value| value.checked_add(edges.saturating_mul(256)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .ok_or_else(|| CliError::Input("local roughness memory estimate overflowed".into()))
}

struct LocalEmbeddingRoughness {
    analysis: LocalEmbeddingRoughnessSpec,
    requirements: Requirements,
    nodes_sha256: String,
    edges_sha256: String,
}

impl LocalEmbeddingRoughness {
    fn new(
        analysis: LocalEmbeddingRoughnessSpec,
        requirements: Requirements,
        nodes_sha256: String,
        edges_sha256: String,
        limits: LocalEmbeddingRoughnessLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let LocalEmbeddingRoughnessLimits {
            maximum_nodes,
            maximum_edges,
            maximum_dimension,
            memory_budget_mib,
            ..
        } = limits;
        let epsilon_bits = analysis.epsilon.to_bits().to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-local-embedding-roughness-configuration-v1".as_slice(),
            epsilon_bits.as_bytes(),
            maximum_nodes.to_string().as_bytes(),
            maximum_edges.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            analysis
                .maximum_component_edge_visits
                .to_string()
                .as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;unique-unordered-positive-weight-edges;per-node-weighted-squared-difference;epsilon_bits={epsilon_bits};maximum_nodes={maximum_nodes};maximum_edges={maximum_edges};maximum_dimension={maximum_dimension};maximum_component_edge_visits={};memory_budget_mib={memory_budget_mib}",
            analysis.maximum_component_edge_visits
        )
        .into_bytes();
        Ok((
            Self {
                analysis,
                requirements,
                nodes_sha256,
                edges_sha256,
            },
            NodeContract {
                id: "local-embedding-roughness",
                kind: "local_embedding_roughness",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::ExactFloatJson,
            },
        ))
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        if output.nodes_sha256 != self.nodes_sha256
            || output.edges_sha256 != self.edges_sha256
            || output.format != "marklab.local_embedding_roughness"
            || output.version != 1
            || output.node_count as usize != self.analysis.nodes.len()
            || output.edge_count as usize != self.analysis.edges.len()
            || output.signal_dimension as usize != self.analysis.feature_names.len()
            || output.feature_names != self.analysis.feature_names
            || output.epsilon.to_bits() != self.analysis.epsilon.to_bits()
            || output.graph_digest != self.requirements.graph_digest
            || output.claim_status != ClaimStatus::ExperimentalDescriptiveNoHotspotInference
            || output.rows.len() != self.requirements.sorted_node_ids.len()
            || self.requirements.component_edge_visits > self.analysis.maximum_component_edge_visits
        {
            return Err(invalid(
                "decoded local embedding roughness identity differs",
            ));
        }
        for ((row, expected_id), expected_neighbors) in output
            .rows
            .iter()
            .zip(&self.requirements.sorted_node_ids)
            .zip(&self.requirements.neighbor_counts)
        {
            let isolated = *expected_neighbors == 0;
            if &row.node_id != expected_id
                || row.neighbor_count != *expected_neighbors
                || !row.weighted_degree.is_finite()
                || row.weighted_degree < 0.0
                || !row.weighted_squared_difference.is_finite()
                || row.weighted_squared_difference < 0.0
                || !row.denominator.is_finite()
                || row.denominator.to_bits()
                    != row.weighted_degree.max(self.analysis.epsilon).to_bits()
                || !row.local_roughness.is_finite()
                || row.local_roughness.to_bits()
                    != (row.weighted_squared_difference / row.denominator).to_bits()
                || row.status
                    != if isolated {
                        "isolated_node"
                    } else {
                        "available"
                    }
                || (isolated
                    && (row.weighted_degree != 0.0
                        || row.weighted_squared_difference != 0.0
                        || row.local_roughness != 0.0))
            {
                return Err(invalid("decoded local roughness row differs"));
            }
        }
        Ok(())
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let result =
            local_embedding_roughness(self.analysis.clone()).map_err(NodeError::execution)?;
        Ok(Output {
            nodes_sha256: self.nodes_sha256.clone(),
            edges_sha256: self.edges_sha256.clone(),
            format: result.format.into(),
            version: result.version,
            node_count: result.node_count,
            edge_count: result.edge_count,
            signal_dimension: result.signal_dimension,
            feature_names: result.feature_names,
            epsilon: result.epsilon,
            graph_digest: result.graph_digest,
            claim_status: result.claim_status,
            rows: result
                .rows
                .into_iter()
                .map(|row| Row {
                    node_id: row.node_id,
                    neighbor_count: row.neighbor_count,
                    weighted_degree: row.weighted_degree,
                    weighted_squared_difference: row.weighted_squared_difference,
                    denominator: row.denominator,
                    local_roughness: row.local_roughness,
                    status: row.status.as_str().into(),
                })
                .collect(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    nodes_sha256: String,
    edges_sha256: String,
    format: String,
    version: u32,
    node_count: u32,
    edge_count: u64,
    signal_dimension: u32,
    feature_names: Vec<String>,
    epsilon: f64,
    graph_digest: String,
    claim_status: ClaimStatus,
    rows: Vec<Row>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Row {
    node_id: String,
    neighbor_count: u32,
    weighted_degree: f64,
    weighted_squared_difference: f64,
    denominator: f64,
    local_roughness: f64,
    status: String,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
