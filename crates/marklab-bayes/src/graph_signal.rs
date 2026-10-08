use std::collections::{HashMap, HashSet};

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use thiserror::Error;

use crate::{
    embedding_spatial::{FiniteNeumaierError, FiniteNeumaierSum},
    sha256_hex,
};

#[derive(Clone, Debug, Serialize)]
pub struct GraphSignalNode {
    pub node_id: String,
    pub signal: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSignalEdge {
    pub left_node_id: String,
    pub right_node_id: String,
    pub weight: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphLaplacian {
    Combinatorial,
    SymmetricNormalized,
}

impl GraphLaplacian {
    pub fn parse(value: &str) -> Result<Self, GraphSignalError> {
        match value {
            "combinatorial" => Ok(Self::Combinatorial),
            "symmetric_normalized" => Ok(Self::SymmetricNormalized),
            _ => Err(GraphSignalError::Invalid(
                "laplacian must be combinatorial or symmetric_normalized".into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEnergyNormalization {
    None,
    Signal,
    EdgeWeight,
}

impl GraphEnergyNormalization {
    pub fn parse(value: &str) -> Result<Self, GraphSignalError> {
        match value {
            "none" => Ok(Self::None),
            "signal" => Ok(Self::Signal),
            "edge_weight" => Ok(Self::EdgeWeight),
            _ => Err(GraphSignalError::Invalid(
                "normalization must be none, signal, or edge_weight".into(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GraphDirichletEnergySpec {
    pub nodes: Vec<GraphSignalNode>,
    pub feature_names: Vec<String>,
    pub edges: Vec<GraphSignalEdge>,
    pub laplacian: GraphLaplacian,
    pub normalization: GraphEnergyNormalization,
    pub maximum_component_edge_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphDirichletEnergyResult {
    pub format: &'static str,
    pub version: u32,
    pub node_count: u32,
    pub edge_count: u64,
    pub signal_dimension: u32,
    pub feature_names: Vec<String>,
    pub laplacian: GraphLaplacian,
    pub normalization: GraphEnergyNormalization,
    pub numerator: f64,
    pub denominator: f64,
    pub energy: f64,
    pub graph_digest: String,
    pub edge_representation: &'static str,
}

#[derive(Clone, Debug)]
pub struct StratifiedGraphSignalNode {
    pub node_id: String,
    pub permutation_stratum: String,
    pub signal: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct GraphSmoothnessPermutationSpec {
    pub nodes: Vec<StratifiedGraphSignalNode>,
    pub feature_names: Vec<String>,
    pub edges: Vec<GraphSignalEdge>,
    pub laplacian: GraphLaplacian,
    pub permutations: u32,
    pub seed: u64,
    pub maximum_component_edge_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphSmoothnessPermutationResult {
    pub format: &'static str,
    pub version: u32,
    pub laplacian: GraphLaplacian,
    pub normalization: GraphEnergyNormalization,
    pub alternative: &'static str,
    pub permutation_policy: &'static str,
    pub node_count: u32,
    pub edge_count: u64,
    pub signal_dimension: u32,
    pub stratum_count: u32,
    pub permutations: u32,
    pub seed: u64,
    pub component_edge_visits: u64,
    pub observed_numerator: f64,
    pub signal_denominator: f64,
    pub observed_energy: f64,
    pub null_mean: f64,
    pub null_sd: f64,
    pub null_min: f64,
    pub null_max: f64,
    pub null_at_or_below_observed: u32,
    pub p_low: f64,
    pub graph_digest: String,
}

#[derive(Clone, Debug)]
pub struct LocalEmbeddingRoughnessSpec {
    pub nodes: Vec<GraphSignalNode>,
    pub feature_names: Vec<String>,
    pub edges: Vec<GraphSignalEdge>,
    pub epsilon: f64,
    pub maximum_component_edge_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalEmbeddingRoughnessRow {
    pub node_id: String,
    pub neighbor_count: u32,
    pub weighted_degree: f64,
    pub weighted_squared_difference: f64,
    pub denominator: f64,
    pub local_roughness: f64,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalEmbeddingRoughnessResult {
    pub format: &'static str,
    pub version: u32,
    pub node_count: u32,
    pub edge_count: u64,
    pub signal_dimension: u32,
    pub feature_names: Vec<String>,
    pub epsilon: f64,
    pub graph_digest: String,
    pub claim_status: &'static str,
    pub rows: Vec<LocalEmbeddingRoughnessRow>,
}

#[derive(Debug, Error)]
pub enum GraphSignalError {
    #[error("invalid graph signal analysis: {0}")]
    Invalid(String),
    #[error("graph signal analysis exceeded its numeric range: {0}")]
    Numeric(String),
}

impl From<FiniteNeumaierError> for GraphSignalError {
    fn from(error: FiniteNeumaierError) -> Self {
        let message = match error {
            FiniteNeumaierError::NonFiniteInput => "a graph contribution is non-finite",
            FiniteNeumaierError::SumOverflow => "a graph sum overflowed",
            FiniteNeumaierError::CorrectionOverflow => "a graph correction overflowed",
            FiniteNeumaierError::TotalOverflow => "a graph total overflowed",
        };
        Self::Numeric(message.into())
    }
}

type StableSum = FiniteNeumaierSum;

struct ResolvedGraph {
    nodes: Vec<GraphSignalNode>,
    feature_names: Vec<String>,
    edges: Vec<(usize, usize, f64)>,
    degrees: Vec<f64>,
    graph_digest: String,
}

pub fn graph_dirichlet_energy(
    spec: GraphDirichletEnergySpec,
) -> Result<GraphDirichletEnergyResult, GraphSignalError> {
    let laplacian = spec.laplacian;
    let normalization = spec.normalization;
    let maximum_component_edge_visits = spec.maximum_component_edge_visits;
    let graph = resolve_graph(spec.nodes, spec.feature_names, spec.edges)?;
    let work = graph
        .edges
        .len()
        .checked_mul(graph.feature_names.len())
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| GraphSignalError::Invalid("component-edge work overflowed".into()))?;
    if maximum_component_edge_visits == 0
        || work > maximum_component_edge_visits
        || maximum_component_edge_visits > 250_000_000
    {
        return Err(GraphSignalError::Invalid(format!(
            "{work} component-edge visits exceed the declared or fixed resource bound"
        )));
    }
    let numerator = dirichlet_numerator(&graph, laplacian, None)?;
    let denominator = match normalization {
        GraphEnergyNormalization::None => 1.0,
        GraphEnergyNormalization::Signal => signal_variation(&graph.nodes)?,
        GraphEnergyNormalization::EdgeWeight => {
            let mut sum = StableSum::default();
            for (_, _, weight) in &graph.edges {
                sum.add(2.0 * weight)?;
            }
            sum.total()?
        }
    };
    if !denominator.is_finite() || denominator <= 1e-14 {
        return Err(GraphSignalError::Invalid(
            "selected graph-energy normalization has a zero denominator".into(),
        ));
    }
    let energy = numerator / denominator;
    if !energy.is_finite() {
        return Err(GraphSignalError::Numeric(
            "normalized graph energy is non-finite".into(),
        ));
    }
    Ok(GraphDirichletEnergyResult {
        format: "marklab.graph_dirichlet_energy",
        version: 1,
        node_count: graph.nodes.len() as u32,
        edge_count: graph.edges.len() as u64,
        signal_dimension: graph.feature_names.len() as u32,
        feature_names: graph.feature_names,
        laplacian,
        normalization,
        numerator,
        denominator,
        energy,
        graph_digest: graph.graph_digest,
        edge_representation: "unique_unordered_edges_imply_symmetric_zero_diagonal_weight_matrix",
    })
}

pub fn graph_smoothness_permutation_test(
    mut spec: GraphSmoothnessPermutationSpec,
) -> Result<GraphSmoothnessPermutationResult, GraphSignalError> {
    if !(20..=10_000).contains(&spec.permutations) || spec.maximum_component_edge_visits == 0 {
        return Err(GraphSignalError::Invalid(
            "smoothness permutation or work controls are invalid".into(),
        ));
    }
    spec.nodes
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let mut groups = std::collections::BTreeMap::<String, Vec<usize>>::new();
    for (index, node) in spec.nodes.iter().enumerate() {
        if node.permutation_stratum.is_empty()
            || node.permutation_stratum.trim() != node.permutation_stratum
        {
            return Err(GraphSignalError::Invalid(
                "permutation strata must be exact nonempty values".into(),
            ));
        }
        groups
            .entry(node.permutation_stratum.clone())
            .or_default()
            .push(index);
    }
    if groups.values().any(|indices| indices.len() < 2) {
        return Err(GraphSignalError::Invalid(
            "every graph permutation stratum requires at least two nodes".into(),
        ));
    }
    let nodes = spec
        .nodes
        .into_iter()
        .map(|node| GraphSignalNode {
            node_id: node.node_id,
            signal: node.signal,
        })
        .collect();
    let graph = resolve_graph(nodes, spec.feature_names, spec.edges)?;
    let component_edge_visits = (u64::from(spec.permutations) + 1)
        .checked_mul(graph.edges.len() as u64)
        .and_then(|value| value.checked_mul(graph.feature_names.len() as u64))
        .ok_or_else(|| GraphSignalError::Invalid("smoothness work overflowed".into()))?;
    if component_edge_visits > spec.maximum_component_edge_visits
        || spec.maximum_component_edge_visits > 250_000_000
    {
        return Err(GraphSignalError::Invalid(format!(
            "{component_edge_visits} component-edge visits exceed the declared or fixed resource bound"
        )));
    }
    let denominator = signal_variation(&graph.nodes)?;
    if denominator <= 1e-14 {
        return Err(GraphSignalError::Invalid(
            "SIGNAL normalization requires nonzero global signal variation".into(),
        ));
    }
    let observed_numerator = dirichlet_numerator(&graph, spec.laplacian, None)?;
    let observed_energy = observed_numerator / denominator;
    let identity = (0..graph.nodes.len()).collect::<Vec<_>>();
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut null = Vec::with_capacity(spec.permutations as usize);
    for _ in 0..spec.permutations {
        let mut donor = identity.clone();
        for indices in groups.values() {
            let mut shuffled = indices.clone();
            shuffled.shuffle(&mut rng);
            for (&receiver, &source) in indices.iter().zip(&shuffled) {
                donor[receiver] = source;
            }
        }
        null.push(dirichlet_numerator(&graph, spec.laplacian, Some(&donor))? / denominator);
    }
    let mut null_sum = StableSum::default();
    for value in &null {
        null_sum.add(*value)?;
    }
    let null_mean = null_sum.total()? / null.len() as f64;
    let mut squared = StableSum::default();
    for value in &null {
        squared.add((value - null_mean) * (value - null_mean))?;
    }
    let null_sd = (squared.total()? / (null.len() - 1) as f64).sqrt();
    let null_min = null.iter().copied().fold(f64::INFINITY, f64::min);
    let null_max = null.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let inclusive = null
        .iter()
        .filter(|value| **value <= observed_energy)
        .count() as u32;
    let p_low = f64::from(inclusive + 1) / f64::from(spec.permutations + 1);
    Ok(GraphSmoothnessPermutationResult {
        format: "marklab.graph_smoothness_permutation_test",
        version: 1,
        laplacian: spec.laplacian,
        normalization: GraphEnergyNormalization::Signal,
        alternative: "low_energy_spatial_smoothness",
        permutation_policy: "complete_signal_rows_within_declared_strata",
        node_count: graph.nodes.len() as u32,
        edge_count: graph.edges.len() as u64,
        signal_dimension: graph.feature_names.len() as u32,
        stratum_count: groups.len() as u32,
        permutations: spec.permutations,
        seed: spec.seed,
        component_edge_visits,
        observed_numerator,
        signal_denominator: denominator,
        observed_energy,
        null_mean,
        null_sd,
        null_min,
        null_max,
        null_at_or_below_observed: inclusive,
        p_low,
        graph_digest: graph.graph_digest,
    })
}

pub fn local_embedding_roughness(
    spec: LocalEmbeddingRoughnessSpec,
) -> Result<LocalEmbeddingRoughnessResult, GraphSignalError> {
    if !spec.epsilon.is_finite() || spec.epsilon <= 0.0 || spec.maximum_component_edge_visits == 0 {
        return Err(GraphSignalError::Invalid(
            "local roughness epsilon or work bound is invalid".into(),
        ));
    }
    let epsilon = spec.epsilon;
    let maximum_component_edge_visits = spec.maximum_component_edge_visits;
    let graph = resolve_graph(spec.nodes, spec.feature_names, spec.edges)?;
    let work = (graph.edges.len() as u64)
        .checked_mul(graph.feature_names.len() as u64)
        .ok_or_else(|| GraphSignalError::Invalid("local roughness work overflowed".into()))?;
    if work > maximum_component_edge_visits || maximum_component_edge_visits > 250_000_000 {
        return Err(GraphSignalError::Invalid(format!(
            "{work} component-edge visits exceed the declared or fixed resource bound"
        )));
    }
    let mut numerators = vec![StableSum::default(); graph.nodes.len()];
    let mut neighbor_counts = vec![0_u32; graph.nodes.len()];
    for (left, right, weight) in &graph.edges {
        let mut squared = StableSum::default();
        for (left_value, right_value) in graph.nodes[*left]
            .signal
            .iter()
            .zip(&graph.nodes[*right].signal)
        {
            squared.add((left_value - right_value) * (left_value - right_value))?;
        }
        let contribution = weight * squared.total()?;
        numerators[*left].add(contribution)?;
        numerators[*right].add(contribution)?;
        neighbor_counts[*left] += 1;
        neighbor_counts[*right] += 1;
    }
    let rows = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let numerator = numerators[index].total()?;
            let denominator = graph.degrees[index].max(epsilon);
            let local_roughness = numerator / denominator;
            if !local_roughness.is_finite() {
                return Err(GraphSignalError::Numeric(format!(
                    "local roughness for node {} is non-finite",
                    node.node_id
                )));
            }
            Ok(LocalEmbeddingRoughnessRow {
                node_id: node.node_id.clone(),
                neighbor_count: neighbor_counts[index],
                weighted_degree: graph.degrees[index],
                weighted_squared_difference: numerator,
                denominator,
                local_roughness,
                status: if neighbor_counts[index] == 0 {
                    "isolated_node"
                } else {
                    "available"
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LocalEmbeddingRoughnessResult {
        format: "marklab.local_embedding_roughness",
        version: 1,
        node_count: graph.nodes.len() as u32,
        edge_count: graph.edges.len() as u64,
        signal_dimension: graph.feature_names.len() as u32,
        feature_names: graph.feature_names,
        epsilon,
        graph_digest: graph.graph_digest,
        claim_status: "experimental_descriptive_no_hotspot_inference",
        rows,
    })
}

fn resolve_graph(
    mut nodes: Vec<GraphSignalNode>,
    feature_names: Vec<String>,
    edges: Vec<GraphSignalEdge>,
) -> Result<ResolvedGraph, GraphSignalError> {
    if !(2..=100_000).contains(&nodes.len())
        || !(1..=128).contains(&feature_names.len())
        || edges.is_empty()
        || edges.len() > 1_000_000
    {
        return Err(GraphSignalError::Invalid(
            "graph node, edge, or signal dimensions are invalid".into(),
        ));
    }
    let mut features = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("signal_")
            || !features.insert(name.as_str())
    }) {
        return Err(GraphSignalError::Invalid(
            "signal feature names must be unique exact signal_* names".into(),
        ));
    }
    nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let mut identifiers = HashSet::new();
    for node in &nodes {
        if node.node_id.is_empty()
            || node.node_id.trim() != node.node_id
            || !identifiers.insert(node.node_id.as_str())
            || node.signal.len() != feature_names.len()
            || node.signal.iter().any(|value| !value.is_finite())
        {
            return Err(GraphSignalError::Invalid(
                "graph nodes require unique exact IDs and complete finite signal vectors".into(),
            ));
        }
    }
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(edges.len());
    for edge in edges {
        let Some(&left) = indices.get(edge.left_node_id.as_str()) else {
            return Err(GraphSignalError::Invalid(format!(
                "edge references unknown node {}",
                edge.left_node_id
            )));
        };
        let Some(&right) = indices.get(edge.right_node_id.as_str()) else {
            return Err(GraphSignalError::Invalid(format!(
                "edge references unknown node {}",
                edge.right_node_id
            )));
        };
        let pair = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if left == right || !edge.weight.is_finite() || edge.weight <= 0.0 || !seen.insert(pair) {
            return Err(GraphSignalError::Invalid(
                "graph edges must be unique unordered positive-weight non-self pairs".into(),
            ));
        }
        resolved.push((pair.0, pair.1, edge.weight));
    }
    resolved.sort_by_key(|edge| (edge.0, edge.1));
    let mut degree_sums = vec![StableSum::default(); nodes.len()];
    for (left, right, weight) in &resolved {
        degree_sums[*left].add(*weight)?;
        degree_sums[*right].add(*weight)?;
    }
    let degrees = degree_sums
        .into_iter()
        .map(StableSum::total)
        .collect::<Result<Vec<_>, _>>()?;
    #[derive(Serialize)]
    struct DigestGraph<'a> {
        nodes: &'a [GraphSignalNode],
        feature_names: &'a [String],
        edges: &'a [(usize, usize, f64)],
    }
    let digest_bytes = serde_json::to_vec(&DigestGraph {
        nodes: &nodes,
        feature_names: &feature_names,
        edges: &resolved,
    })
    .map_err(|error| GraphSignalError::Invalid(format!("graph digest encoding failed: {error}")))?;
    Ok(ResolvedGraph {
        nodes,
        feature_names,
        edges: resolved,
        degrees,
        graph_digest: sha256_hex(&digest_bytes),
    })
}

fn dirichlet_numerator(
    graph: &ResolvedGraph,
    laplacian: GraphLaplacian,
    donor: Option<&[usize]>,
) -> Result<f64, GraphSignalError> {
    if laplacian == GraphLaplacian::SymmetricNormalized
        && graph.degrees.iter().any(|degree| *degree <= 0.0)
    {
        return Err(GraphSignalError::Invalid(
            "symmetric-normalized Laplacian requires positive degree at every node".into(),
        ));
    }
    let identity;
    let donor = if let Some(donor) = donor {
        donor
    } else {
        identity = (0..graph.nodes.len()).collect::<Vec<_>>();
        &identity
    };
    let mut numerator = StableSum::default();
    for (left, right, weight) in &graph.edges {
        let left_signal = &graph.nodes[donor[*left]].signal;
        let right_signal = &graph.nodes[donor[*right]].signal;
        let mut squared = StableSum::default();
        for (left_value, right_value) in left_signal.iter().zip(right_signal) {
            let (left_value, right_value) = match laplacian {
                GraphLaplacian::Combinatorial => (*left_value, *right_value),
                GraphLaplacian::SymmetricNormalized => (
                    *left_value / graph.degrees[*left].sqrt(),
                    *right_value / graph.degrees[*right].sqrt(),
                ),
            };
            squared.add((left_value - right_value) * (left_value - right_value))?;
        }
        numerator.add(weight * squared.total()?)?;
    }
    Ok(numerator.total()?)
}

fn signal_variation(nodes: &[GraphSignalNode]) -> Result<f64, GraphSignalError> {
    let dimension = nodes[0].signal.len();
    let mut means = vec![StableSum::default(); dimension];
    for node in nodes {
        for (sum, value) in means.iter_mut().zip(&node.signal) {
            sum.add(*value)?;
        }
    }
    let means = means
        .into_iter()
        .map(|sum| sum.total().map(|value| value / nodes.len() as f64))
        .collect::<Result<Vec<_>, _>>()?;
    let mut variation = StableSum::default();
    for node in nodes {
        for (value, mean) in node.signal.iter().zip(&means) {
            variation.add((value - mean) * (value - mean))?;
        }
    }
    Ok(variation.total()?)
}
