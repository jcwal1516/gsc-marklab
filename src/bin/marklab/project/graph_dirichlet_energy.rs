use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use marklab_bayes::{
    graph_dirichlet_energy, sha256_hex, GraphDirichletEnergySpec, GraphEnergyNormalization,
    GraphLaplacian, GraphSignalEdge, GraphSignalNode,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const NODES_KIND: &str = "application/vnd.marklab.source.graph-signal-nodes-csv;version=1";
const EDGES_KIND: &str = "application/vnd.marklab.source.graph-signal-edges-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.graph-dirichlet-energy+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-graph-dirichlet-energy-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    nodes_path: PathBuf,
    edges_path: PathBuf,
    laplacian: String,
    normalization: String,
    maximum_nodes: usize,
    maximum_edges: usize,
    maximum_dimension: usize,
    maximum_component_edge_visits: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_nodes < 2
        || maximum_edges == 0
        || maximum_dimension == 0
        || maximum_component_edge_visits == 0
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "graph Dirichlet energy resource limits must be positive and admit at least two nodes"
                .into(),
        ));
    }
    let laplacian = GraphLaplacian::parse(&laplacian).map_err(map_error)?;
    let normalization = GraphEnergyNormalization::parse(&normalization).map_err(map_error)?;

    let paths = SourcePaths {
        nodes: nodes_path,
        edges: edges_path,
    };
    let node_bytes = read_bounded(&paths.nodes, memory_bytes, "node")?;
    let edge_bytes = read_bounded(&paths.edges, memory_bytes, "edge")?;
    let source_bytes = node_bytes
        .len()
        .checked_add(edge_bytes.len())
        .ok_or_else(|| BayesCliError::Input("graph source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(BayesCliError::Input(
            "graph source files exceed the retained-memory budget".into(),
        ));
    }
    let prepared_artifacts = source_artifacts_from_bytes(&node_bytes, &edge_bytes)?;
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
        laplacian,
        normalization,
    )?;
    let retained = retained_bytes(source_bytes, nodes.len(), feature_names.len(), edges.len())?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "graph Dirichlet energy retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if prepared_artifacts != source_artifacts(&paths)? {
        return Err(BayesCliError::Input(
            "graph Dirichlet energy sources changed while prepared".into(),
        ));
    }

    let nodes_sha256 = prepared_artifacts[0].digest().to_string();
    let edges_sha256 = prepared_artifacts[1].digest().to_string();
    let node = GraphDirichletEnergyProjectNode::new(
        paths,
        prepared_artifacts.clone(),
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
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        memory_budget_mib,
    )?;
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for artifact in &prepared_artifacts {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let execution = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.graph_dirichlet_energy", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project graph-dirichlet-energy cache_status={cache_status}");
    Ok(())
}

#[derive(Clone)]
struct SourcePaths {
    nodes: PathBuf,
    edges: PathBuf,
}

fn source_artifacts(paths: &SourcePaths) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        source_artifact(&paths.nodes, NODES_KIND)?,
        source_artifact(&paths.edges, EDGES_KIND)?,
    ])
}

fn source_artifacts_from_bytes(
    node_bytes: &[u8],
    edge_bytes: &[u8],
) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        ArtifactRef::from_bytes(NODES_KIND, node_bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        ArtifactRef::from_bytes(EDGES_KIND, edge_bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
    ])
}

fn read_bounded(path: &Path, memory_bytes: usize, label: &str) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "graph {label} source must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
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

struct Requirements {
    component_edge_visits: u64,
    graph_digest: String,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    nodes: &[GraphSignalNode],
    feature_names: &[String],
    edges: &[GraphSignalEdge],
    maximum_nodes: usize,
    maximum_edges: usize,
    maximum_dimension: usize,
    maximum_component_edge_visits: u64,
    laplacian: GraphLaplacian,
    normalization: GraphEnergyNormalization,
) -> Result<Requirements, BayesCliError> {
    if !(2..=100_000).contains(&nodes.len())
        || !(1..=128).contains(&feature_names.len())
        || edges.is_empty()
        || edges.len() > 1_000_000
    {
        return Err(BayesCliError::Input(
            "graph node, edge, or signal dimensions are invalid".into(),
        ));
    }
    if nodes.len() > maximum_nodes {
        return Err(BayesCliError::Input(format!(
            "node count exceeds maximum_nodes {maximum_nodes}"
        )));
    }
    if edges.len() > maximum_edges {
        return Err(BayesCliError::Input(format!(
            "edge count exceeds maximum_edges {maximum_edges}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(BayesCliError::Input(format!(
            "signal dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let mut features = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("signal_")
            || !features.insert(name.as_str())
    }) {
        return Err(BayesCliError::Input(
            "signal feature names must be unique exact signal_* names".into(),
        ));
    }

    let mut sorted_nodes = nodes.to_vec();
    sorted_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let mut identifiers = HashSet::new();
    if sorted_nodes.iter().any(|node| {
        node.node_id.is_empty()
            || node.node_id.trim() != node.node_id
            || !identifiers.insert(node.node_id.as_str())
            || node.signal.len() != feature_names.len()
            || node.signal.iter().any(|value| !value.is_finite())
    }) {
        return Err(BayesCliError::Input(
            "graph nodes require unique exact IDs and complete finite signal vectors".into(),
        ));
    }
    let indices = sorted_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(edges.len());
    let mut neighbor_counts = vec![0_u32; nodes.len()];
    let mut degree_sums = vec![0.0_f64; nodes.len()];
    for edge in edges {
        let left = indices
            .get(edge.left_node_id.as_str())
            .copied()
            .ok_or_else(|| {
                BayesCliError::Input(format!(
                    "edge references unknown node {}",
                    edge.left_node_id
                ))
            })?;
        let right = indices
            .get(edge.right_node_id.as_str())
            .copied()
            .ok_or_else(|| {
                BayesCliError::Input(format!(
                    "edge references unknown node {}",
                    edge.right_node_id
                ))
            })?;
        let pair = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if left == right || !edge.weight.is_finite() || edge.weight <= 0.0 || !seen.insert(pair) {
            return Err(BayesCliError::Input(
                "graph edges must be unique unordered positive-weight non-self pairs".into(),
            ));
        }
        neighbor_counts[left] = neighbor_counts[left]
            .checked_add(1)
            .ok_or_else(|| BayesCliError::Input("graph neighbor count overflowed".into()))?;
        neighbor_counts[right] = neighbor_counts[right]
            .checked_add(1)
            .ok_or_else(|| BayesCliError::Input("graph neighbor count overflowed".into()))?;
        degree_sums[left] += edge.weight;
        degree_sums[right] += edge.weight;
        if !degree_sums[left].is_finite() || !degree_sums[right].is_finite() {
            return Err(BayesCliError::Input("graph degree sum overflowed".into()));
        }
        resolved.push((pair.0, pair.1, edge.weight));
    }
    resolved.sort_by_key(|edge| (edge.0, edge.1));
    if laplacian == GraphLaplacian::SymmetricNormalized
        && neighbor_counts.iter().any(|count| *count == 0)
    {
        return Err(BayesCliError::Input(
            "symmetric-normalized Laplacian requires positive degree at every node".into(),
        ));
    }
    if normalization == GraphEnergyNormalization::Signal {
        let mut means = vec![0.0; feature_names.len()];
        for node in &sorted_nodes {
            for (mean, value) in means.iter_mut().zip(&node.signal) {
                *mean += *value;
                if !mean.is_finite() {
                    return Err(BayesCliError::Input(
                        "graph signal mean overflowed during admission".into(),
                    ));
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
                    return Err(BayesCliError::Input(
                        "graph signal variation overflowed during admission".into(),
                    ));
                }
            }
        }
        if variation <= 1e-14 {
            return Err(BayesCliError::Input(
                "selected graph-energy normalization has a zero denominator".into(),
            ));
        }
    }

    let component_edge_visits = (edges.len() as u64)
        .checked_mul(feature_names.len() as u64)
        .ok_or_else(|| BayesCliError::Input("graph Dirichlet energy work overflowed".into()))?;
    if component_edge_visits > maximum_component_edge_visits
        || maximum_component_edge_visits > 250_000_000
    {
        return Err(BayesCliError::Input(format!(
            "{component_edge_visits} component-edge visits exceed the declared or fixed resource bound"
        )));
    }

    #[derive(Serialize)]
    struct DigestGraph<'a> {
        nodes: &'a [GraphSignalNode],
        feature_names: &'a [String],
        edges: &'a [(usize, usize, f64)],
    }
    let digest_bytes = serde_json::to_vec(&DigestGraph {
        nodes: &sorted_nodes,
        feature_names,
        edges: &resolved,
    })
    .map_err(|error| BayesCliError::Input(format!("graph digest encoding failed: {error}")))?;
    Ok(Requirements {
        component_edge_visits,
        graph_digest: sha256_hex(&digest_bytes),
    })
}

fn retained_bytes(
    source_bytes: usize,
    nodes: usize,
    dimension: usize,
    edges: usize,
) -> Result<usize, BayesCliError> {
    let signal_bytes = nodes
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(2 * std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("graph signal memory overflowed".into()))?;
    source_bytes
        .checked_add(signal_bytes)
        .and_then(|value| value.checked_add(nodes.saturating_mul(768)))
        .and_then(|value| value.checked_add(edges.saturating_mul(256)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .ok_or_else(|| {
            BayesCliError::Input("graph Dirichlet energy memory estimate overflowed".into())
        })
}

struct GraphDirichletEnergyProjectNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    analysis: GraphDirichletEnergySpec,
    requirements: Requirements,
    nodes_sha256: String,
    edges_sha256: String,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl GraphDirichletEnergyProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        analysis: GraphDirichletEnergySpec,
        requirements: Requirements,
        nodes_sha256: String,
        edges_sha256: String,
        maximum_nodes: usize,
        maximum_edges: usize,
        maximum_dimension: usize,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 2 {
            return Err(BayesCliError::Input(
                "graph Dirichlet energy source identities are incomplete".into(),
            ));
        }
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
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("graph-dirichlet-energy")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_dirichlet_energy",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            paths,
            input_artifacts,
            analysis,
            requirements,
            nodes_sha256,
            edges_sha256,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
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
}

impl WorkflowNode for GraphDirichletEnergyProjectNode {
    type Output = Output;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = source_artifacts(&self.paths).map_err(NodeError::input)?;
        if current != self.input_artifacts {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "graph Dirichlet energy source identity changed",
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
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

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: Output = marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
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

fn map_error(error: marklab_bayes::GraphSignalError) -> BayesCliError {
    match error {
        marklab_bayes::GraphSignalError::Invalid(message) => BayesCliError::Input(message),
        marklab_bayes::GraphSignalError::Numeric(message) => BayesCliError::Backend(message),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
