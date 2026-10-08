use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use marklab_bayes::{
    graph_smoothness_permutation_test, sha256_hex, GraphLaplacian, GraphSignalEdge,
    GraphSignalNode, GraphSmoothnessPermutationResult, GraphSmoothnessPermutationSpec,
    StratifiedGraphSignalNode,
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

const NODES_KIND: &str = "application/vnd.marklab.source.stratified-graph-signals-csv;version=1";
const EDGES_KIND: &str = "application/vnd.marklab.source.graph-edges-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.graph-smoothness-permutation+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-graph-smoothness-permutation-test-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    nodes_path: PathBuf,
    edges_path: PathBuf,
    laplacian: String,
    permutations: u32,
    seed: u64,
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
            "graph smoothness node, edge, dimension, work, and memory limits must be positive"
                .into(),
        ));
    }
    let laplacian_value = GraphLaplacian::parse(&laplacian).map_err(map_error)?;
    let paths = SourcePaths {
        nodes: nodes_path,
        edges: edges_path,
    };
    let node_bytes = read_bounded(&paths.nodes, memory_bytes)?;
    let edge_bytes = read_bounded(&paths.edges, memory_bytes)?;
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
        laplacian_value,
        permutations,
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
    )?;
    let retained = retained_bytes(
        source_bytes,
        nodes.len(),
        edges.len(),
        feature_names.len(),
        permutations,
    )?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "graph smoothness retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if source_artifacts(&paths)? != prepared_artifacts {
        return Err(BayesCliError::Input(
            "graph smoothness sources changed while prepared".into(),
        ));
    }
    drop(node_bytes);
    drop(edge_bytes);

    let nodes_sha256 = prepared_artifacts[0].digest().to_string();
    let edges_sha256 = prepared_artifacts[1].digest().to_string();
    let analysis = GraphSmoothnessPermutationSpec {
        nodes,
        feature_names,
        edges,
        laplacian: laplacian_value,
        permutations,
        seed,
        maximum_component_edge_visits,
    };
    let node = GraphSmoothnessProjectNode::new(
        paths,
        prepared_artifacts.clone(),
        analysis,
        requirements,
        nodes_sha256,
        edges_sha256,
        laplacian,
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
        ArtifactSchema::new("marklab.graph_smoothness_permutation_test", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project graph-smoothness-permutation-test cache_status={cache_status}");
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

fn read_bounded(path: &Path, memory_bytes: usize) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "graph source must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_nodes(
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

#[derive(Clone)]
struct Requirements {
    node_count: usize,
    edge_count: usize,
    dimension: usize,
    stratum_count: usize,
    component_edge_visits: u64,
    graph_digest: String,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    nodes: &[StratifiedGraphSignalNode],
    feature_names: &[String],
    edges: &[GraphSignalEdge],
    laplacian: GraphLaplacian,
    permutations: u32,
    maximum_nodes: usize,
    maximum_edges: usize,
    maximum_dimension: usize,
    maximum_component_edge_visits: u64,
) -> Result<Requirements, BayesCliError> {
    if !(20..=10_000).contains(&permutations)
        || !(2..=100_000).contains(&nodes.len())
        || !(1..=128).contains(&feature_names.len())
        || edges.is_empty()
        || edges.len() > 1_000_000
    {
        return Err(BayesCliError::Input(
            "smoothness graph dimensions or permutation controls are invalid".into(),
        ));
    }
    if nodes.len() > maximum_nodes {
        return Err(BayesCliError::Input(format!(
            "graph node count exceeds maximum_nodes {maximum_nodes}"
        )));
    }
    if edges.len() > maximum_edges {
        return Err(BayesCliError::Input(format!(
            "graph edge count exceeds maximum_edges {maximum_edges}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(BayesCliError::Input(format!(
            "graph signal dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let mut feature_set = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("signal_")
            || !feature_set.insert(name.as_str())
    }) {
        return Err(BayesCliError::Input(
            "signal feature names must be unique exact signal_* names".into(),
        ));
    }
    let mut node_ids = HashMap::new();
    let mut strata = BTreeMap::<&str, usize>::new();
    for (index, node) in nodes.iter().enumerate() {
        if node.node_id.is_empty()
            || node.node_id.trim() != node.node_id
            || node_ids.insert(node.node_id.as_str(), index).is_some()
            || node.permutation_stratum.is_empty()
            || node.permutation_stratum.trim() != node.permutation_stratum
            || node.signal.len() != feature_names.len()
            || node.signal.iter().any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "smoothness nodes require unique exact IDs/strata and complete finite signal vectors"
                    .into(),
            ));
        }
        *strata.entry(node.permutation_stratum.as_str()).or_default() += 1;
    }
    if strata.values().any(|count| *count < 2) {
        return Err(BayesCliError::Input(
            "every graph permutation stratum requires at least two nodes".into(),
        ));
    }
    let mut pairs = HashSet::new();
    let mut degree = vec![0.0_f64; nodes.len()];
    for edge in edges {
        let Some(&left) = node_ids.get(edge.left_node_id.as_str()) else {
            return Err(BayesCliError::Input(format!(
                "edge references unknown node {}",
                edge.left_node_id
            )));
        };
        let Some(&right) = node_ids.get(edge.right_node_id.as_str()) else {
            return Err(BayesCliError::Input(format!(
                "edge references unknown node {}",
                edge.right_node_id
            )));
        };
        let pair = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if left == right || !edge.weight.is_finite() || edge.weight <= 0.0 || !pairs.insert(pair) {
            return Err(BayesCliError::Input(
                "graph edges must be unique unordered positive-weight non-self pairs".into(),
            ));
        }
        degree[left] += edge.weight;
        degree[right] += edge.weight;
        if !degree[left].is_finite() || !degree[right].is_finite() {
            return Err(BayesCliError::Input("graph degree overflowed".into()));
        }
    }
    if laplacian == GraphLaplacian::SymmetricNormalized && degree.iter().any(|value| *value <= 0.0)
    {
        return Err(BayesCliError::Input(
            "symmetric-normalized Laplacian requires positive degree at every node".into(),
        ));
    }
    let mut means = vec![0.0; feature_names.len()];
    for node in nodes {
        for (mean, value) in means.iter_mut().zip(&node.signal) {
            *mean += value;
            if !mean.is_finite() {
                return Err(BayesCliError::Input("graph signal sum overflowed".into()));
            }
        }
    }
    for mean in &mut means {
        *mean /= nodes.len() as f64;
    }
    let mut variation = 0.0;
    for node in nodes {
        for (value, mean) in node.signal.iter().zip(&means) {
            variation += (value - mean) * (value - mean);
            if !variation.is_finite() {
                return Err(BayesCliError::Input(
                    "graph signal variation overflowed".into(),
                ));
            }
        }
    }
    if variation <= 1e-14 {
        return Err(BayesCliError::Input(
            "SIGNAL normalization requires nonzero global signal variation".into(),
        ));
    }
    let component_edge_visits = (u64::from(permutations) + 1)
        .checked_mul(edges.len() as u64)
        .and_then(|value| value.checked_mul(feature_names.len() as u64))
        .ok_or_else(|| BayesCliError::Input("smoothness work overflowed".into()))?;
    if component_edge_visits > maximum_component_edge_visits
        || maximum_component_edge_visits > 250_000_000
    {
        return Err(BayesCliError::Input(format!(
            "{component_edge_visits} component-edge visits exceed the declared or fixed resource bound"
        )));
    }
    let mut digest_nodes = nodes
        .iter()
        .map(|node| GraphSignalNode {
            node_id: node.node_id.clone(),
            signal: node.signal.clone(),
        })
        .collect::<Vec<_>>();
    digest_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let digest_indices = digest_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut digest_edges = edges
        .iter()
        .map(|edge| {
            let left = digest_indices[edge.left_node_id.as_str()];
            let right = digest_indices[edge.right_node_id.as_str()];
            if left < right {
                (left, right, edge.weight)
            } else {
                (right, left, edge.weight)
            }
        })
        .collect::<Vec<_>>();
    digest_edges.sort_by_key(|edge| (edge.0, edge.1));
    #[derive(Serialize)]
    struct DigestGraph<'a> {
        nodes: &'a [GraphSignalNode],
        feature_names: &'a [String],
        edges: &'a [(usize, usize, f64)],
    }
    let digest_bytes = serde_json::to_vec(&DigestGraph {
        nodes: &digest_nodes,
        feature_names,
        edges: &digest_edges,
    })
    .map_err(|error| BayesCliError::Input(format!("graph digest encoding failed: {error}")))?;
    Ok(Requirements {
        node_count: nodes.len(),
        edge_count: edges.len(),
        dimension: feature_names.len(),
        stratum_count: strata.len(),
        component_edge_visits,
        graph_digest: sha256_hex(&digest_bytes),
    })
}

fn retained_bytes(
    source_bytes: usize,
    nodes: usize,
    edges: usize,
    dimension: usize,
    permutations: u32,
) -> Result<usize, BayesCliError> {
    let analysis = source_bytes
        .checked_add(nodes.saturating_mul(384 + dimension.saturating_mul(8)))
        .and_then(|value| value.checked_add(edges.saturating_mul(256)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .ok_or_else(|| BayesCliError::Input("graph analysis memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(nodes.saturating_mul(128)))
        .and_then(|value| value.checked_add(edges.saturating_mul(64)))
        .and_then(|value| value.checked_add(permutations as usize * 16))
        .ok_or_else(|| BayesCliError::Input("graph retained-memory estimate overflowed".into()))
}

struct GraphSmoothnessProjectNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    analysis: GraphSmoothnessPermutationSpec,
    requirements: Requirements,
    nodes_sha256: String,
    edges_sha256: String,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl GraphSmoothnessProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        analysis: GraphSmoothnessPermutationSpec,
        requirements: Requirements,
        nodes_sha256: String,
        edges_sha256: String,
        laplacian: String,
        maximum_nodes: usize,
        maximum_edges: usize,
        maximum_dimension: usize,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 2 {
            return Err(BayesCliError::Input(
                "graph smoothness source identities are incomplete".into(),
            ));
        }
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-graph-smoothness-permutation-configuration-v1".as_slice(),
            laplacian.as_bytes(),
            analysis.permutations.to_string().as_bytes(),
            analysis.seed.to_string().as_bytes(),
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
            "serial;complete-signal-rows-within-declared-strata;low-energy-inclusive-permutation-test;laplacian={laplacian};normalization=signal;permutations={};seed={};maximum_nodes={maximum_nodes};maximum_edges={maximum_edges};maximum_dimension={maximum_dimension};maximum_component_edge_visits={};memory_budget_mib={memory_budget_mib}",
            analysis.permutations, analysis.seed, analysis.maximum_component_edge_visits
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("graph-smoothness-permutation-test")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_smoothness_permutation_test",
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
        let expected_laplacian = laplacian_name(self.analysis.laplacian);
        let expected_p =
            f64::from(output.null_at_or_below_observed + 1) / f64::from(output.permutations + 1);
        if output.nodes_sha256 != self.nodes_sha256
            || output.edges_sha256 != self.edges_sha256
            || output.format != "marklab.graph_smoothness_permutation_test"
            || output.version != 1
            || output.laplacian != expected_laplacian
            || output.normalization != "signal"
            || output.alternative != "low_energy_spatial_smoothness"
            || output.permutation_policy != "complete_signal_rows_within_declared_strata"
            || output.node_count as usize != self.requirements.node_count
            || output.edge_count as usize != self.requirements.edge_count
            || output.signal_dimension as usize != self.requirements.dimension
            || output.stratum_count as usize != self.requirements.stratum_count
            || output.permutations != self.analysis.permutations
            || output.seed != self.analysis.seed
            || output.component_edge_visits != self.requirements.component_edge_visits
            || output.component_edge_visits > self.analysis.maximum_component_edge_visits
            || output.null_at_or_below_observed > output.permutations
            || !nonnegative_finite(output.observed_numerator)
            || !positive_finite(output.signal_denominator)
            || !nonnegative_finite(output.observed_energy)
            || !nonnegative_finite(output.null_mean)
            || !nonnegative_finite(output.null_sd)
            || !nonnegative_finite(output.null_min)
            || !nonnegative_finite(output.null_max)
            || output.null_min > output.null_max
            || !positive_unit_interval(output.p_low)
            || output.p_low.to_bits() != expected_p.to_bits()
            || output.graph_digest != self.requirements.graph_digest
        {
            return Err(invalid("decoded graph smoothness result differs"));
        }
        Ok(())
    }
}

impl WorkflowNode for GraphSmoothnessProjectNode {
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
                "graph smoothness source identity changed",
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
        let result = graph_smoothness_permutation_test(self.analysis.clone())
            .map_err(NodeError::execution)?;
        Ok(Output::from_result(
            self.nodes_sha256.clone(),
            self.edges_sha256.clone(),
            result,
        ))
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
    laplacian: String,
    normalization: String,
    alternative: String,
    permutation_policy: String,
    node_count: u32,
    edge_count: u64,
    signal_dimension: u32,
    stratum_count: u32,
    permutations: u32,
    seed: u64,
    component_edge_visits: u64,
    observed_numerator: f64,
    signal_denominator: f64,
    observed_energy: f64,
    null_mean: f64,
    null_sd: f64,
    null_min: f64,
    null_max: f64,
    null_at_or_below_observed: u32,
    p_low: f64,
    graph_digest: String,
}

impl Output {
    fn from_result(
        nodes_sha256: String,
        edges_sha256: String,
        result: GraphSmoothnessPermutationResult,
    ) -> Self {
        Self {
            nodes_sha256,
            edges_sha256,
            format: result.format.into(),
            version: result.version,
            laplacian: laplacian_name(result.laplacian).into(),
            normalization: "signal".into(),
            alternative: result.alternative.into(),
            permutation_policy: result.permutation_policy.into(),
            node_count: result.node_count,
            edge_count: result.edge_count,
            signal_dimension: result.signal_dimension,
            stratum_count: result.stratum_count,
            permutations: result.permutations,
            seed: result.seed,
            component_edge_visits: result.component_edge_visits,
            observed_numerator: result.observed_numerator,
            signal_denominator: result.signal_denominator,
            observed_energy: result.observed_energy,
            null_mean: result.null_mean,
            null_sd: result.null_sd,
            null_min: result.null_min,
            null_max: result.null_max,
            null_at_or_below_observed: result.null_at_or_below_observed,
            p_low: result.p_low,
            graph_digest: result.graph_digest,
        }
    }
}

fn laplacian_name(value: GraphLaplacian) -> &'static str {
    match value {
        GraphLaplacian::Combinatorial => "combinatorial",
        GraphLaplacian::SymmetricNormalized => "symmetric_normalized",
    }
}

fn map_error(error: marklab_bayes::GraphSignalError) -> BayesCliError {
    match error {
        marklab_bayes::GraphSignalError::Invalid(message) => BayesCliError::Input(message),
        marklab_bayes::GraphSignalError::Numeric(message) => BayesCliError::Backend(message),
    }
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn nonnegative_finite(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn positive_unit_interval(value: f64) -> bool {
    positive_finite(value) && value <= 1.0
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
