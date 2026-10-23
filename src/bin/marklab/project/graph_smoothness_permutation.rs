use crate::codecs::graph_signal::{read_edges, read_stratified_nodes as read_nodes};

use std::{borrow::Cow, collections::BTreeMap, io, path::Path};

use marklab_graph::{
    graph_smoothness_permutation_test, summarize_graph_signal_input, GraphLaplacian,
    GraphSignalEdge, GraphSignalNode, GraphSmoothnessPermutationResult,
    GraphSmoothnessPermutationSpec, StratifiedGraphSignalNode,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{
    GraphSmoothnessPermutationTestLimits, GraphSmoothnessPermutationTestProjectArgs,
};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const NODES_KIND: &str = "application/vnd.marklab.source.stratified-graph-signals-csv;version=1";
const EDGES_KIND: &str = "application/vnd.marklab.source.graph-edges-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.graph-smoothness-permutation+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-graph-smoothness-permutation-test-node-v1";

pub(super) fn run(args: GraphSmoothnessPermutationTestProjectArgs) -> Result<(), CliError> {
    let GraphSmoothnessPermutationTestProjectArgs {
        project: project_path,
        nodes: nodes_path,
        edges: edges_path,
        laplacian,
        permutations,
        seed,
        limits,
        out: output_path,
    } = args;
    let GraphSmoothnessPermutationTestLimits {
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "graph smoothness sources";
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
            "graph smoothness node, edge, dimension, work, and memory limits must be positive"
                .into(),
        ));
    }
    let laplacian_value = GraphLaplacian::parse(&laplacian).map_err(map_error)?;
    let node_bytes = read_bounded(&nodes_path, memory_bytes)?;
    let edge_bytes = read_bounded(&edges_path, memory_bytes)?;
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
        laplacian_value,
        permutations,
        limits,
    )?;
    let retained = retained_bytes(
        source_bytes,
        nodes.len(),
        edges.len(),
        feature_names.len(),
        permutations,
    )?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "graph smoothness retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;
    drop(node_bytes);
    drop(edge_bytes);

    let nodes_sha256 = sources[0].artifact().digest().to_string();
    let edges_sha256 = sources[1].artifact().digest().to_string();
    let analysis = GraphSmoothnessPermutationSpec {
        nodes,
        feature_names,
        edges,
        laplacian: laplacian_value,
        permutations,
        seed,
        maximum_component_edge_visits,
    };
    let (analysis, contract) = GraphSmoothness::new(
        analysis,
        requirements,
        nodes_sha256,
        edges_sha256,
        laplacian,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        GraphSmoothness::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.graph_smoothness_permutation_test", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "graph-smoothness-permutation-test",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn read_bounded(path: &Path, memory_bytes: usize) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "graph source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
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

fn preflight(
    nodes: &[StratifiedGraphSignalNode],
    feature_names: &[String],
    edges: &[GraphSignalEdge],
    laplacian: GraphLaplacian,
    permutations: u32,
    limits: GraphSmoothnessPermutationTestLimits,
) -> Result<Requirements, CliError> {
    let GraphSmoothnessPermutationTestLimits {
        maximum_nodes,
        maximum_edges,
        maximum_dimension,
        maximum_component_edge_visits,
        ..
    } = limits;
    if !(20..=10_000).contains(&permutations)
        || !(2..=100_000).contains(&nodes.len())
        || !(1..=128).contains(&feature_names.len())
        || edges.is_empty()
        || edges.len() > 1_000_000
    {
        return Err(CliError::Input(
            "smoothness graph dimensions or permutation controls are invalid".into(),
        ));
    }
    if nodes.len() > maximum_nodes {
        return Err(CliError::Input(format!(
            "graph node count exceeds maximum_nodes {maximum_nodes}"
        )));
    }
    if edges.len() > maximum_edges {
        return Err(CliError::Input(format!(
            "graph edge count exceeds maximum_edges {maximum_edges}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(CliError::Input(format!(
            "graph signal dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let mut strata = BTreeMap::<&str, usize>::new();
    for node in nodes {
        if node.permutation_stratum.is_empty()
            || node.permutation_stratum.trim() != node.permutation_stratum
        {
            return Err(CliError::Input(
                "smoothness nodes require unique exact IDs/strata and complete finite signal vectors"
                    .into(),
            ));
        }
        *strata.entry(node.permutation_stratum.as_str()).or_default() += 1;
    }
    if strata.values().any(|count| *count < 2) {
        return Err(CliError::Input(
            "every graph permutation stratum requires at least two nodes".into(),
        ));
    }
    let unstratified = nodes
        .iter()
        .map(|node| GraphSignalNode {
            node_id: node.node_id.clone(),
            signal: node.signal.clone(),
        })
        .collect::<Vec<_>>();
    let summary = summarize_graph_signal_input(&unstratified, feature_names, edges)
        .map_err(|error| CliError::Input(error.to_string()))?;
    if laplacian == GraphLaplacian::SymmetricNormalized && summary.neighbor_counts.contains(&0) {
        return Err(CliError::Input(
            "symmetric-normalized Laplacian requires positive degree at every node".into(),
        ));
    }
    let mut means = vec![0.0; feature_names.len()];
    for node in nodes {
        for (mean, value) in means.iter_mut().zip(&node.signal) {
            *mean += value;
            if !mean.is_finite() {
                return Err(CliError::Input("graph signal sum overflowed".into()));
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
                return Err(CliError::Input("graph signal variation overflowed".into()));
            }
        }
    }
    if variation <= 1e-14 {
        return Err(CliError::Input(
            "SIGNAL normalization requires nonzero global signal variation".into(),
        ));
    }
    let component_edge_visits = (u64::from(permutations) + 1)
        .checked_mul(edges.len() as u64)
        .and_then(|value| value.checked_mul(feature_names.len() as u64))
        .ok_or_else(|| CliError::Input("smoothness work overflowed".into()))?;
    if component_edge_visits > maximum_component_edge_visits
        || maximum_component_edge_visits > 250_000_000
    {
        return Err(CliError::Input(format!(
            "{component_edge_visits} component-edge visits exceed the declared or fixed resource bound"
        )));
    }
    Ok(Requirements {
        node_count: nodes.len(),
        edge_count: edges.len(),
        dimension: feature_names.len(),
        stratum_count: strata.len(),
        component_edge_visits,
        graph_digest: summary.graph_digest,
    })
}

fn retained_bytes(
    source_bytes: usize,
    nodes: usize,
    edges: usize,
    dimension: usize,
    permutations: u32,
) -> Result<usize, CliError> {
    let analysis = source_bytes
        .checked_add(nodes.saturating_mul(384 + dimension.saturating_mul(8)))
        .and_then(|value| value.checked_add(edges.saturating_mul(256)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .ok_or_else(|| CliError::Input("graph analysis memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(nodes.saturating_mul(128)))
        .and_then(|value| value.checked_add(edges.saturating_mul(64)))
        .and_then(|value| value.checked_add(permutations as usize * 16))
        .ok_or_else(|| CliError::Input("graph retained-memory estimate overflowed".into()))
}

struct GraphSmoothness {
    analysis: GraphSmoothnessPermutationSpec,
    requirements: Requirements,
    nodes_sha256: String,
    edges_sha256: String,
}

impl GraphSmoothness {
    fn new(
        analysis: GraphSmoothnessPermutationSpec,
        requirements: Requirements,
        nodes_sha256: String,
        edges_sha256: String,
        laplacian: String,
        limits: GraphSmoothnessPermutationTestLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let GraphSmoothnessPermutationTestLimits {
            maximum_nodes,
            maximum_edges,
            maximum_dimension,
            memory_budget_mib,
            ..
        } = limits;
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
        Ok((
            Self {
                analysis,
                requirements,
                nodes_sha256,
                edges_sha256,
            },
            NodeContract {
                id: "graph-smoothness-permutation-test",
                kind: "graph_smoothness_permutation_test",
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

    fn execute(&self) -> Result<Output, NodeError> {
        let result = graph_smoothness_permutation_test(self.analysis.clone())
            .map_err(NodeError::execution)?;
        Ok(Output::from_result(
            self.nodes_sha256.clone(),
            self.edges_sha256.clone(),
            result,
        ))
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

fn map_error(error: marklab_graph::GraphSignalError) -> CliError {
    match error {
        marklab_graph::GraphSignalError::Invalid(message) => CliError::Input(message),
        marklab_graph::GraphSignalError::Numeric(message) => CliError::Backend(message),
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
