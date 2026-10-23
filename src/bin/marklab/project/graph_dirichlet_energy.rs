use crate::codecs::graph_signal::{read_edges, read_nodes};

use std::{borrow::Cow, io, path::Path};

use marklab_graph::{
    graph_dirichlet_energy, summarize_graph_signal_input, GraphDirichletEnergySpec,
    GraphEnergyNormalization, GraphLaplacian, GraphSignalEdge, GraphSignalNode,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{GraphDirichletEnergyLimits, GraphDirichletEnergyProjectArgs};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const NODES_KIND: &str = "application/vnd.marklab.source.graph-signal-nodes-csv;version=1";
const EDGES_KIND: &str = "application/vnd.marklab.source.graph-signal-edges-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.graph-dirichlet-energy+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-graph-dirichlet-energy-node-v1";

pub(super) fn run(args: GraphDirichletEnergyProjectArgs) -> Result<(), CliError> {
    let GraphDirichletEnergyProjectArgs {
        project: project_path,
        nodes: nodes_path,
        edges: edges_path,
        laplacian,
        normalization,
        limits,
        out: output_path,
    } = args;
    let GraphDirichletEnergyLimits {
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "graph Dirichlet energy sources";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_nodes < 2
        || maximum_edges == 0
        || maximum_dimension == 0
        || maximum_component_edge_visits == 0
        || memory_bytes == 0
    {
        return Err(CliError::Input(
            "graph Dirichlet energy resource limits must be positive and admit at least two nodes"
                .into(),
        ));
    }
    let laplacian = GraphLaplacian::parse(&laplacian).map_err(map_error)?;
    let normalization = GraphEnergyNormalization::parse(&normalization).map_err(map_error)?;

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
        limits,
        laplacian,
        normalization,
    )?;
    let retained = retained_bytes(source_bytes, nodes.len(), feature_names.len(), edges.len())?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "graph Dirichlet energy retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;

    let nodes_sha256 = sources[0].artifact().digest().to_string();
    let edges_sha256 = sources[1].artifact().digest().to_string();
    let (analysis, contract) = GraphDirichletEnergy::new(
        GraphDirichletEnergySpec {
            nodes,
            feature_names,
            edges,
            laplacian,
            normalization,
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
        GraphDirichletEnergy::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.graph_dirichlet_energy", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "graph-dirichlet-energy",
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

struct Requirements {
    component_edge_visits: u64,
    graph_digest: String,
}

fn preflight(
    nodes: &[GraphSignalNode],
    feature_names: &[String],
    edges: &[GraphSignalEdge],
    limits: GraphDirichletEnergyLimits,
    laplacian: GraphLaplacian,
    normalization: GraphEnergyNormalization,
) -> Result<Requirements, CliError> {
    let GraphDirichletEnergyLimits {
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
        ..
    } = limits;
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
    if laplacian == GraphLaplacian::SymmetricNormalized && summary.neighbor_counts.contains(&0) {
        return Err(CliError::Input(
            "symmetric-normalized Laplacian requires positive degree at every node".into(),
        ));
    }
    if normalization == GraphEnergyNormalization::Signal {
        let mut sorted_nodes = nodes.to_vec();
        sorted_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        let mut means = vec![0.0; feature_names.len()];
        for node in &sorted_nodes {
            for (mean, value) in means.iter_mut().zip(&node.signal) {
                *mean += *value;
                if !mean.is_finite() {
                    return Err(CliError::Input("graph signal mean overflowed".into()));
                }
            }
        }
        for mean in &mut means {
            *mean /= sorted_nodes.len() as f64;
        }
        let mut variation = 0.0;
        for node in &sorted_nodes {
            for (value, mean) in node.signal.iter().zip(&means) {
                variation += (value - mean) * (value - mean);
                if !variation.is_finite() {
                    return Err(CliError::Input("graph signal variation overflowed".into()));
                }
            }
        }
        if variation <= 1e-14 {
            return Err(CliError::Input(
                "selected graph-energy normalization has a zero denominator".into(),
            ));
        }
    }

    let component_edge_visits = (edges.len() as u64)
        .checked_mul(feature_names.len() as u64)
        .ok_or_else(|| CliError::Input("graph Dirichlet energy work overflowed".into()))?;
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
        .ok_or_else(|| CliError::Input("graph Dirichlet energy memory estimate overflowed".into()))
}

struct GraphDirichletEnergy {
    analysis: GraphDirichletEnergySpec,
    requirements: Requirements,
    nodes_sha256: String,
    edges_sha256: String,
}

impl GraphDirichletEnergy {
    fn new(
        analysis: GraphDirichletEnergySpec,
        requirements: Requirements,
        nodes_sha256: String,
        edges_sha256: String,
        limits: GraphDirichletEnergyLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let GraphDirichletEnergyLimits {
            maximum_nodes,
            maximum_edges,
            maximum_dimension,
            memory_budget_mib,
            ..
        } = limits;
        let laplacian = laplacian_name(analysis.laplacian);
        let normalization = normalization_name(analysis.normalization);
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-graph-dirichlet-energy-configuration-v1".as_slice(),
            laplacian.as_bytes(),
            normalization.as_bytes(),
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
            "serial;unique-unordered-positive-weight-edges;quadratic-dirichlet-form;laplacian={laplacian};normalization={normalization};maximum_nodes={maximum_nodes};maximum_edges={maximum_edges};maximum_dimension={maximum_dimension};maximum_component_edge_visits={};memory_budget_mib={memory_budget_mib}",
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
                id: "graph-dirichlet-energy",
                kind: "graph_dirichlet_energy",
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
            || output.format != "marklab.graph_dirichlet_energy"
            || output.version != 1
            || output.node_count as usize != self.analysis.nodes.len()
            || output.edge_count as usize != self.analysis.edges.len()
            || output.signal_dimension as usize != self.analysis.feature_names.len()
            || output.feature_names != self.analysis.feature_names
            || output.laplacian != laplacian_name(self.analysis.laplacian)
            || output.normalization != normalization_name(self.analysis.normalization)
            || !output.numerator.is_finite()
            || output.numerator < 0.0
            || !output.denominator.is_finite()
            || output.denominator <= 0.0
            || !output.energy.is_finite()
            || output.energy.to_bits() != (output.numerator / output.denominator).to_bits()
            || output.graph_digest != self.requirements.graph_digest
            || output.edge_representation
                != "unique_unordered_edges_imply_symmetric_zero_diagonal_weight_matrix"
            || self.requirements.component_edge_visits > self.analysis.maximum_component_edge_visits
        {
            return Err(invalid("decoded graph Dirichlet energy identity differs"));
        }
        if self.analysis.normalization == GraphEnergyNormalization::None
            && output.denominator.to_bits() != 1.0_f64.to_bits()
        {
            return Err(invalid("decoded graph energy denominator differs"));
        }
        Ok(())
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let result = graph_dirichlet_energy(self.analysis.clone()).map_err(NodeError::execution)?;
        Ok(Output {
            nodes_sha256: self.nodes_sha256.clone(),
            edges_sha256: self.edges_sha256.clone(),
            format: result.format.into(),
            version: result.version,
            node_count: result.node_count,
            edge_count: result.edge_count,
            signal_dimension: result.signal_dimension,
            feature_names: result.feature_names,
            laplacian: laplacian_name(result.laplacian).into(),
            normalization: normalization_name(result.normalization).into(),
            numerator: result.numerator,
            denominator: result.denominator,
            energy: result.energy,
            graph_digest: result.graph_digest,
            edge_representation: result.edge_representation.into(),
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
    laplacian: String,
    normalization: String,
    numerator: f64,
    denominator: f64,
    energy: f64,
    graph_digest: String,
    edge_representation: String,
}

fn laplacian_name(value: GraphLaplacian) -> &'static str {
    match value {
        GraphLaplacian::Combinatorial => "combinatorial",
        GraphLaplacian::SymmetricNormalized => "symmetric_normalized",
    }
}

fn normalization_name(value: GraphEnergyNormalization) -> &'static str {
    match value {
        GraphEnergyNormalization::None => "none",
        GraphEnergyNormalization::Signal => "signal",
        GraphEnergyNormalization::EdgeWeight => "edge_weight",
    }
}

fn map_error(error: marklab_graph::GraphSignalError) -> CliError {
    match error {
        marklab_graph::GraphSignalError::Invalid(message) => CliError::Input(message),
        marklab_graph::GraphSignalError::Numeric(message) => CliError::Backend(message),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
