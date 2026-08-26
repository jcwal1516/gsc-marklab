use std::collections::{BTreeMap, HashSet};

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::GraphError;

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

pub fn typed_triangle_motif_workflow(
    mut spec: TypedTriangleMotifSpec,
) -> Result<TypedTriangleMotifResult, GraphError> {
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
    let triples_evaluated = (spec.nodes.len() as u64)
        .checked_mul(spec.nodes.len() as u64 - 1)
        .and_then(|value| value.checked_mul(spec.nodes.len() as u64 - 2))
        .and_then(|value| value.checked_div(6))
        .ok_or_else(|| GraphError::Invalid("motif triple work overflow".into()))?;
    if triples_evaluated > spec.maximum_triples {
        return Err(GraphError::Invalid(format!(
            "motif triples {triples_evaluated} exceed caller maximum {}",
            spec.maximum_triples
        )));
    }
    let triangles = enumerate_triangles(spec.nodes.len(), &edge_indices);
    let observed_labels = spec
        .nodes
        .iter()
        .map(|node| node.label.clone())
        .collect::<Vec<_>>();
    let matching = matching_triangles(&triangles, &observed_labels, &spec.required_labels);
    let mut motif_adjacency = vec![vec![0_u64; spec.nodes.len()]; spec.nodes.len()];
    for triangle in &matching {
        for left in 0..3 {
            for right in (left + 1)..3 {
                motif_adjacency[triangle[left]][triangle[right]] += 1;
                motif_adjacency[triangle[right]][triangle[left]] += 1;
            }
        }
    }
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
            .push(matching_triangles(&triangles, &labels, &spec.required_labels).len() as u64);
    }
    let observed_count = matching.len() as u64;
    let p_value_upper = (1 + null_counts
        .iter()
        .filter(|count| **count >= observed_count)
        .count()) as f64
        / (spec.permutations + 1) as f64;
    let instances = matching
        .iter()
        .map(|triangle| triangle.map(|index| spec.nodes[index].id.clone()))
        .collect();
    let digest = serde_json::to_vec(&serde_json::json!({"nodes": &spec.nodes, "edges": &edges}))
        .map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(TypedTriangleMotifResult {
        format: "marklab.typed_triangle_motif",
        version: 1,
        graph_digest: format!("{:x}", Sha256::digest(digest)),
        motif_id: spec.motif_id,
        required_labels: spec.required_labels,
        nodes: spec.nodes,
        edges,
        observed_count,
        instances,
        motif_adjacency,
        null_model: "labels_permuted_within_declared_strata",
        null_counts,
        p_value_upper,
        permutations_completed: spec.permutations,
        seed: spec.seed,
        triples_evaluated,
        claim_status: "experimental_typed_triangle_motif",
    })
}

fn enumerate_triangles(node_count: usize, edges: &HashSet<(usize, usize)>) -> Vec<[usize; 3]> {
    let mut triangles = Vec::new();
    for first in 0..node_count {
        for second in (first + 1)..node_count {
            for third in (second + 1)..node_count {
                if edges.contains(&(first, second))
                    && edges.contains(&(first, third))
                    && edges.contains(&(second, third))
                {
                    triangles.push([first, second, third]);
                }
            }
        }
    }
    triangles
}

fn matching_triangles(
    triangles: &[[usize; 3]],
    labels: &[String],
    required: &[String; 3],
) -> Vec<[usize; 3]> {
    triangles
        .iter()
        .copied()
        .filter(|triangle| {
            let mut actual = triangle.map(|index| labels[index].as_str());
            actual.sort();
            actual == required.each_ref().map(String::as_str)
        })
        .collect()
}

fn validate(spec: &TypedTriangleMotifSpec) -> Result<(), GraphError> {
    if !(3..=10_000).contains(&spec.nodes.len())
        || spec.edges.len() > 1_000_000
        || spec.motif_id.is_empty()
        || spec.motif_id.trim() != spec.motif_id
        || spec.required_labels.iter().any(|label| label.is_empty())
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
            || node.label.trim() != node.label
            || node.stratum.trim() != node.stratum
    }) {
        return Err(GraphError::Invalid(
            "motif nodes require unique exact IDs, labels, and strata".into(),
        ));
    }
    Ok(())
}
