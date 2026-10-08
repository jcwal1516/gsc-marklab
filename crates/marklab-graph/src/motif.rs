use std::{
    collections::{BTreeMap, HashSet},
    mem::size_of,
};

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::GraphError;

const MAXIMUM_FORWARD_WEDGES: u64 = 2_000_000;
const MAXIMUM_TRIANGLES: usize = 250_000;
const MAXIMUM_DENSE_ADJACENCY_BYTES: usize = 128 * 1024 * 1024;
const MAXIMUM_TOKEN_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotifNodeInput {
    pub id: String,
    pub label: String,
    pub stratum: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotifEdgeInput {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedTriangleMotifSpec {
    pub nodes: Vec<MotifNodeInput>,
    pub edges: Vec<MotifEdgeInput>,
    pub motif_id: String,
    pub required_labels: [String; 3],
    pub permutations: usize,
    pub seed: u64,
    pub maximum_triples: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TypedTriangleMotifResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub motif_id: String,
    pub required_labels: [String; 3],
    pub nodes: Vec<MotifNodeInput>,
    pub edges: Vec<MotifEdgeInput>,
    pub observed_count: u64,
    pub instances: Vec<[String; 3]>,
    pub motif_adjacency: Vec<Vec<u64>>,
    pub null_model: &'static str,
    pub null_counts: Vec<u64>,
    pub p_value_upper: f64,
    pub permutations_completed: usize,
    pub seed: u64,
    pub triples_evaluated: u64,
    pub claim_status: &'static str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedTriangleMotifSummaryResult {
    pub format: String,
    pub version: u32,
    pub graph_digest: String,
    pub motif_id: String,
    pub required_labels: [String; 3],
    pub node_count: usize,
    pub edge_count: usize,
    pub observed_count: u64,
    pub null_model: String,
    pub null_counts: Vec<u64>,
    pub p_value_upper: f64,
    pub permutations_completed: usize,
    pub seed: u64,
    pub triples_evaluated: u64,
    pub claim_status: String,
}

impl TypedTriangleMotifSummaryResult {
    pub fn validate_for_spec(&self, spec: &TypedTriangleMotifSpec) -> Result<(), GraphError> {
        let mut required_labels = spec.required_labels.clone();
        required_labels.sort();
        let exceedance_count = self
            .null_counts
            .iter()
            .filter(|count| **count >= self.observed_count)
            .count();
        let expected_p_value = (1 + exceedance_count) as f64 / (spec.permutations + 1) as f64;
        if self.format != "marklab.typed_triangle_motif_summary"
            || self.version != 1
            || self.graph_digest.len() != 64
            || !self
                .graph_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.motif_id != spec.motif_id
            || self.required_labels != required_labels
            || self.node_count != spec.nodes.len()
            || self.edge_count != spec.edges.len()
            || self.observed_count > MAXIMUM_TRIANGLES as u64
            || self.null_model != "labels_permuted_within_declared_strata"
            || self.null_counts.len() != spec.permutations
            || self
                .null_counts
                .iter()
                .any(|count| *count > MAXIMUM_TRIANGLES as u64)
            || self.p_value_upper.to_bits() != expected_p_value.to_bits()
            || self.permutations_completed != spec.permutations
            || self.seed != spec.seed
            || self.triples_evaluated > spec.maximum_triples.min(MAXIMUM_FORWARD_WEDGES)
            || self.claim_status != "experimental_typed_triangle_motif_summary"
        {
            return Err(GraphError::Invalid(
                "typed triangle motif summary does not match its specification".into(),
            ));
        }
        Ok(())
    }
}

struct MotifAnalysis {
    graph_digest: String,
    motif_id: String,
    required_labels: [String; 3],
    nodes: Vec<MotifNodeInput>,
    edges: Vec<MotifEdgeInput>,
    matching: Vec<[usize; 3]>,
    null_counts: Vec<u64>,
    p_value_upper: f64,
    permutations_completed: usize,
    seed: u64,
    triples_evaluated: u64,
}

pub fn typed_triangle_motif_workflow(
    spec: TypedTriangleMotifSpec,
) -> Result<TypedTriangleMotifResult, GraphError> {
    let analysis = analyze_typed_triangle_motif(spec)?;
    let mut motif_adjacency = zero_motif_adjacency(analysis.nodes.len())?;
    for triangle in &analysis.matching {
        for left in 0..3 {
            for right in (left + 1)..3 {
                motif_adjacency[triangle[left]][triangle[right]] += 1;
                motif_adjacency[triangle[right]][triangle[left]] += 1;
            }
        }
    }
    let instances = analysis
        .matching
        .iter()
        .map(|triangle| triangle.map(|index| analysis.nodes[index].id.clone()))
        .collect();
    Ok(TypedTriangleMotifResult {
        format: "marklab.typed_triangle_motif",
        version: 1,
        graph_digest: analysis.graph_digest,
        motif_id: analysis.motif_id,
        required_labels: analysis.required_labels,
        nodes: analysis.nodes,
        edges: analysis.edges,
        observed_count: analysis.matching.len() as u64,
        instances,
        motif_adjacency,
        null_model: "labels_permuted_within_declared_strata",
        null_counts: analysis.null_counts,
        p_value_upper: analysis.p_value_upper,
        permutations_completed: analysis.permutations_completed,
        seed: analysis.seed,
        triples_evaluated: analysis.triples_evaluated,
        claim_status: "experimental_typed_triangle_motif",
    })
}

pub fn typed_triangle_motif_summary_workflow(
    spec: TypedTriangleMotifSpec,
) -> Result<TypedTriangleMotifSummaryResult, GraphError> {
    let analysis = analyze_typed_triangle_motif(spec)?;
    Ok(TypedTriangleMotifSummaryResult {
        format: "marklab.typed_triangle_motif_summary".to_owned(),
        version: 1,
        graph_digest: analysis.graph_digest,
        motif_id: analysis.motif_id,
        required_labels: analysis.required_labels,
        node_count: analysis.nodes.len(),
        edge_count: analysis.edges.len(),
        observed_count: analysis.matching.len() as u64,
        null_model: "labels_permuted_within_declared_strata".to_owned(),
        null_counts: analysis.null_counts,
        p_value_upper: analysis.p_value_upper,
        permutations_completed: analysis.permutations_completed,
        seed: analysis.seed,
        triples_evaluated: analysis.triples_evaluated,
        claim_status: "experimental_typed_triangle_motif_summary".to_owned(),
    })
}

fn analyze_typed_triangle_motif(
    mut spec: TypedTriangleMotifSpec,
) -> Result<MotifAnalysis, GraphError> {
    validate(&spec)?;
    spec.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    spec.required_labels.sort();
    let index = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut edge_indices = HashSet::new();
    let mut edges = Vec::with_capacity(spec.edges.len());
    for edge in spec.edges {
        let left = index.get(edge.source_id.as_str()).copied().ok_or_else(|| {
            GraphError::Invalid(format!("unknown motif edge node {}", edge.source_id))
        })?;
        let right = index.get(edge.target_id.as_str()).copied().ok_or_else(|| {
            GraphError::Invalid(format!("unknown motif edge node {}", edge.target_id))
        })?;
        if left == right {
            return Err(GraphError::Invalid("motif graph forbids self edges".into()));
        }
        let pair = (left.min(right), left.max(right));
        if !edge_indices.insert(pair) {
            return Err(GraphError::Invalid("duplicate motif graph edge".into()));
        }
        edges.push(MotifEdgeInput {
            source_id: spec.nodes[pair.0].id.clone(),
            target_id: spec.nodes[pair.1].id.clone(),
        });
    }
    edges.sort_by(|left, right| {
        (&left.source_id, &left.target_id).cmp(&(&right.source_id, &right.target_id))
    });
    let (triangles, triples_evaluated) =
        enumerate_triangles(spec.nodes.len(), &edge_indices, spec.maximum_triples)?;
    let observed_labels = spec
        .nodes
        .iter()
        .map(|node| node.label.clone())
        .collect::<Vec<_>>();
    let matching = matching_triangles(&triangles, &observed_labels, &spec.required_labels)?;
    let mut strata = BTreeMap::<&str, Vec<usize>>::new();
    for (node_index, node) in spec.nodes.iter().enumerate() {
        strata
            .entry(node.stratum.as_str())
            .or_default()
            .push(node_index);
    }
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut labels = observed_labels.clone();
    let mut null_counts = Vec::with_capacity(spec.permutations);
    for _ in 0..spec.permutations {
        labels.clone_from(&observed_labels);
        for indices in strata.values() {
            let mut donors = indices
                .iter()
                .map(|index| observed_labels[*index].clone())
                .collect::<Vec<_>>();
            donors.shuffle(&mut rng);
            for (receiver, label) in indices.iter().zip(donors) {
                labels[*receiver] = label;
            }
        }
        null_counts
            .push(count_matching_triangles(&triangles, &labels, &spec.required_labels) as u64);
    }
    let observed_count = matching.len() as u64;
    let p_value_upper = (1 + null_counts
        .iter()
        .filter(|count| **count >= observed_count)
        .count()) as f64
        / (spec.permutations + 1) as f64;
    let digest = serde_json::to_vec(&serde_json::json!({"nodes": &spec.nodes, "edges": &edges}))
        .map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(MotifAnalysis {
        graph_digest: format!("{:x}", Sha256::digest(digest)),
        motif_id: spec.motif_id,
        required_labels: spec.required_labels,
        nodes: spec.nodes,
        edges,
        matching,
        null_counts,
        p_value_upper,
        permutations_completed: spec.permutations,
        seed: spec.seed,
        triples_evaluated,
    })
}

fn enumerate_triangles(
    node_count: usize,
    edges: &HashSet<(usize, usize)>,
    maximum_triples: u64,
) -> Result<(Vec<[usize; 3]>, u64), GraphError> {
    let mut forward = vec![Vec::new(); node_count];
    for &(left, right) in edges {
        forward[left].push(right);
    }
    for neighbors in &mut forward {
        neighbors.sort_unstable();
    }
    let triples_evaluated = forward.iter().try_fold(0_u64, |total, neighbors| {
        let degree = u64::try_from(neighbors.len())
            .map_err(|_| GraphError::Invalid("motif triple work overflow".into()))?;
        let wedges = degree
            .checked_mul(degree.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| GraphError::Invalid("motif triple work overflow".into()))?;
        total
            .checked_add(wedges)
            .ok_or_else(|| GraphError::Invalid("motif triple work overflow".into()))
    })?;
    let effective_maximum = maximum_triples.min(MAXIMUM_FORWARD_WEDGES);
    if triples_evaluated > effective_maximum {
        return Err(GraphError::Invalid(format!(
            "motif forward wedges {triples_evaluated} exceed effective maximum {effective_maximum}"
        )));
    }
    let capacity = usize::try_from(triples_evaluated)
        .map_err(|_| GraphError::Invalid("motif triangle capacity overflow".into()))?;
    let mut triangles = Vec::new();
    triangles
        .try_reserve(capacity)
        .map_err(|_| GraphError::Invalid("motif triangle allocation failed".into()))?;
    for (first, neighbors) in forward.iter().enumerate() {
        for second_offset in 0..neighbors.len() {
            for third_offset in (second_offset + 1)..neighbors.len() {
                let second = neighbors[second_offset];
                let third = neighbors[third_offset];
                if edges.contains(&(second, third)) {
                    if triangles.len() == MAXIMUM_TRIANGLES {
                        return Err(GraphError::Invalid(format!(
                            "motif triangles exceed hard maximum {MAXIMUM_TRIANGLES}"
                        )));
                    }
                    triangles.push([first, second, third]);
                }
            }
        }
    }
    Ok((triangles, triples_evaluated))
}

fn matching_triangles(
    triangles: &[[usize; 3]],
    labels: &[String],
    required: &[String; 3],
) -> Result<Vec<[usize; 3]>, GraphError> {
    let mut matching = Vec::new();
    matching
        .try_reserve(triangles.len())
        .map_err(|_| GraphError::Invalid("motif matching allocation failed".into()))?;
    for triangle in triangles.iter().copied() {
        if triangle_matches(triangle, labels, required) {
            matching.push(triangle);
        }
    }
    Ok(matching)
}

fn count_matching_triangles(
    triangles: &[[usize; 3]],
    labels: &[String],
    required: &[String; 3],
) -> usize {
    triangles
        .iter()
        .filter(|triangle| triangle_matches(**triangle, labels, required))
        .count()
}

fn triangle_matches(triangle: [usize; 3], labels: &[String], required: &[String; 3]) -> bool {
    let mut actual = triangle.map(|index| labels[index].as_str());
    actual.sort();
    actual == required.each_ref().map(String::as_str)
}

fn zero_motif_adjacency(node_count: usize) -> Result<Vec<Vec<u64>>, GraphError> {
    let element_count = node_count
        .checked_mul(node_count)
        .ok_or_else(|| GraphError::Invalid("motif adjacency size overflow".into()))?;
    let element_bytes = element_count
        .checked_mul(size_of::<u64>())
        .ok_or_else(|| GraphError::Invalid("motif adjacency size overflow".into()))?;
    let outer_bytes = node_count
        .checked_mul(size_of::<Vec<u64>>())
        .ok_or_else(|| GraphError::Invalid("motif adjacency size overflow".into()))?;
    let retained = element_bytes
        .checked_add(outer_bytes)
        .ok_or_else(|| GraphError::Invalid("motif adjacency size overflow".into()))?;
    if retained > MAXIMUM_DENSE_ADJACENCY_BYTES {
        return Err(GraphError::Invalid(format!(
            "motif adjacency bytes {retained} exceed hard maximum {MAXIMUM_DENSE_ADJACENCY_BYTES}"
        )));
    }
    let mut adjacency = Vec::new();
    adjacency
        .try_reserve_exact(node_count)
        .map_err(|_| GraphError::Invalid("motif adjacency allocation failed".into()))?;
    for _ in 0..node_count {
        let mut row = Vec::new();
        row.try_reserve_exact(node_count)
            .map_err(|_| GraphError::Invalid("motif adjacency allocation failed".into()))?;
        row.resize(node_count, 0);
        adjacency.push(row);
    }
    Ok(adjacency)
}

fn validate(spec: &TypedTriangleMotifSpec) -> Result<(), GraphError> {
    if !(3..=10_000).contains(&spec.nodes.len())
        || spec.edges.len() > 1_000_000
        || spec.motif_id.is_empty()
        || spec.motif_id.trim() != spec.motif_id
        || spec.motif_id.len() > MAXIMUM_TOKEN_BYTES
        || spec
            .required_labels
            .iter()
            .any(|label| label.is_empty() || label.len() > MAXIMUM_TOKEN_BYTES)
        || !(20..=100_000).contains(&spec.permutations)
        || spec.maximum_triples == 0
    {
        return Err(GraphError::Invalid(
            "typed triangle dimensions, motif, permutations, or work are invalid".into(),
        ));
    }
    let mut ids = HashSet::new();
    if spec.nodes.iter().any(|node| {
        node.id.is_empty()
            || node.id.trim() != node.id
            || !ids.insert(node.id.as_str())
            || node.label.is_empty()
            || node.stratum.is_empty()
            || node.id.len() > MAXIMUM_TOKEN_BYTES
            || node.label.len() > MAXIMUM_TOKEN_BYTES
            || node.stratum.len() > MAXIMUM_TOKEN_BYTES
            || node.label.trim() != node.label
            || node.stratum.trim() != node.stratum
    }) {
        return Err(GraphError::Invalid(
            "motif nodes require unique exact IDs, labels, and strata".into(),
        ));
    }
    Ok(())
}
