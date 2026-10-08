use std::collections::{BTreeMap, BTreeSet, HashSet};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use crate::LongitudinalError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeEdge {
    pub left_node: String,
    pub right_node: String,
    pub branch_length: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloneSpatialRecord {
    pub clone_id: String,
    pub tree_node: String,
    pub centroid_um: [f64; 3],
    pub patient_id: String,
    pub specimen_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhylogeneticSpatialAssociationSpec {
    pub tree_provenance: String,
    pub tree_edges: Vec<TreeEdge>,
    pub clones: Vec<CloneSpatialRecord>,
    pub permutations: usize,
    pub seed: u64,
    pub maximum_pair_permutation_evaluations: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhylogeneticSpatialPair {
    pub left_clone_id: String,
    pub right_clone_id: String,
    pub patient_id: String,
    pub specimen_id: String,
    pub tree_distance: f64,
    pub spatial_distance_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhylogeneticSpatialAssociationResult {
    pub format: &'static str,
    pub version: u32,
    pub tree_provenance: String,
    pub statistic: &'static str,
    pub observed_statistic: f64,
    pub pairs: Vec<PhylogeneticSpatialPair>,
    pub null_statistics: Vec<f64>,
    pub permutations: usize,
    pub extreme_permutations: usize,
    pub p_value: f64,
    pub pair_permutation_evaluations: u64,
    pub tree_traversal_edge_visits_upper_bound: u64,
    pub random_seed_namespace: &'static str,
    pub interpretation: &'static str,
}

struct Tree {
    node_indices: BTreeMap<String, usize>,
    adjacency: Vec<Vec<(usize, f64)>>,
}

pub fn phylogenetic_spatial_association(
    mut spec: PhylogeneticSpatialAssociationSpec,
) -> Result<PhylogeneticSpatialAssociationResult, LongitudinalError> {
    spec.clones.sort_by(|left, right| {
        left.patient_id
            .cmp(&right.patient_id)
            .then_with(|| left.specimen_id.cmp(&right.specimen_id))
            .then_with(|| left.clone_id.cmp(&right.clone_id))
    });
    validate_records(&spec)?;
    let tree = compile_tree(&spec.tree_edges)?;
    let clone_tree_nodes = spec
        .clones
        .iter()
        .map(|clone| {
            tree.node_indices
                .get(&clone.tree_node)
                .copied()
                .ok_or_else(|| {
                    LongitudinalError::Invalid(format!(
                        "clone {} references missing tree node {}",
                        clone.clone_id, clone.tree_node
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tree_distances = clone_tree_distances(&tree, &clone_tree_nodes)?;
    let blocks = blocks(&spec.clones);
    if blocks.values().all(|members| members.len() < 2) {
        return Err(LongitudinalError::Invalid(
            "at least one patient/specimen block must contain two clones".into(),
        ));
    }
    let pair_indices = blocks
        .values()
        .flat_map(|members| {
            (0..members.len()).flat_map(move |left| {
                ((left + 1)..members.len()).map(move |right| (members[left], members[right]))
            })
        })
        .collect::<Vec<_>>();
    let evaluations = u64::try_from(pair_indices.len())
        .ok()
        .and_then(|pairs| {
            u64::try_from(spec.permutations + 1)
                .ok()
                .and_then(|replicates| pairs.checked_mul(replicates))
        })
        .ok_or_else(|| LongitudinalError::Resource("pair-permutation work overflowed".into()))?;
    if evaluations > spec.maximum_pair_permutation_evaluations {
        return Err(LongitudinalError::Resource(format!(
            "pair-permutation evaluations {evaluations} exceed declared maximum {}",
            spec.maximum_pair_permutation_evaluations
        )));
    }
    let spatial_distances = pair_indices
        .iter()
        .map(|&(left, right)| spatial_distance(&spec.clones[left], &spec.clones[right]))
        .collect::<Vec<_>>();
    let observed_tree_distances = pair_indices
        .iter()
        .map(|&(left, right)| tree_distances[left][right])
        .collect::<Vec<_>>();
    let observed_statistic = correlation(&observed_tree_distances, &spatial_distances)?;
    let pairs = pair_indices
        .iter()
        .zip(&observed_tree_distances)
        .zip(&spatial_distances)
        .map(|((pair, &tree_distance), &spatial_distance_um)| {
            let left = &spec.clones[pair.0];
            let right = &spec.clones[pair.1];
            PhylogeneticSpatialPair {
                left_clone_id: left.clone_id.clone(),
                right_clone_id: right.clone_id.clone(),
                patient_id: left.patient_id.clone(),
                specimen_id: left.specimen_id.clone(),
                tree_distance,
                spatial_distance_um,
            }
        })
        .collect::<Vec<_>>();

    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut null_statistics = Vec::with_capacity(spec.permutations);
    for _ in 0..spec.permutations {
        let mut assignment = (0..spec.clones.len()).collect::<Vec<_>>();
        for members in blocks.values() {
            shuffle_assignments(&mut assignment, members, &mut rng);
        }
        let permuted = pair_indices
            .iter()
            .map(|&(left, right)| tree_distances[assignment[left]][assignment[right]])
            .collect::<Vec<_>>();
        null_statistics.push(correlation(&permuted, &spatial_distances)?);
    }
    let extreme_permutations = null_statistics
        .iter()
        .filter(|value| value.abs() >= observed_statistic.abs())
        .count();
    let p_value = (extreme_permutations + 1) as f64 / (spec.permutations + 1) as f64;
    let tree_work = u64::try_from(spec.clones.len())
        .ok()
        .and_then(|clones| {
            u64::try_from(tree.adjacency.len() + 2 * spec.tree_edges.len())
                .ok()
                .and_then(|per_clone| clones.checked_mul(per_clone))
        })
        .ok_or_else(|| LongitudinalError::Resource("tree traversal work overflowed".into()))?;
    Ok(PhylogeneticSpatialAssociationResult {
        format: "marklab.phylogenetic_spatial_association",
        version: 1,
        tree_provenance: spec.tree_provenance,
        statistic: "within_block_pearson_tree_vs_euclidean_distance",
        observed_statistic,
        pairs,
        null_statistics,
        permutations: spec.permutations,
        extreme_permutations,
        p_value,
        pair_permutation_evaluations: evaluations,
        tree_traversal_edge_visits_upper_bound: tree_work,
        random_seed_namespace: "phylogenetic_spatial_association_v1_chacha20",
        interpretation: "cross_sectional_noncausal_association_only",
    })
}

fn validate_records(spec: &PhylogeneticSpatialAssociationSpec) -> Result<(), LongitudinalError> {
    if spec.tree_provenance.trim().is_empty() {
        return Err(LongitudinalError::Invalid(
            "tree provenance must be nonempty".into(),
        ));
    }
    if spec.clones.len() < 3 || spec.permutations == 0 {
        return Err(LongitudinalError::Invalid(
            "at least three clones and one permutation are required".into(),
        ));
    }
    let mut clone_ids = HashSet::new();
    let mut tree_nodes = HashSet::new();
    for clone in &spec.clones {
        if clone.clone_id.trim().is_empty()
            || clone.tree_node.trim().is_empty()
            || clone.patient_id.trim().is_empty()
            || clone.specimen_id.trim().is_empty()
            || !clone_ids.insert(clone.clone_id.as_str())
            || !tree_nodes.insert(clone.tree_node.as_str())
            || clone.centroid_um.iter().any(|value| !value.is_finite())
        {
            return Err(LongitudinalError::Invalid(
                "clone IDs/tree nodes must be unique and all clone fields finite/nonempty".into(),
            ));
        }
    }
    Ok(())
}

fn compile_tree(edges: &[TreeEdge]) -> Result<Tree, LongitudinalError> {
    if edges.is_empty() {
        return Err(LongitudinalError::Invalid(
            "tree must contain an edge".into(),
        ));
    }
    let mut names = BTreeSet::new();
    let mut unique_edges = BTreeSet::new();
    for edge in edges {
        let left = edge.left_node.trim();
        let right = edge.right_node.trim();
        let key = if left <= right {
            (left.to_owned(), right.to_owned())
        } else {
            (right.to_owned(), left.to_owned())
        };
        if left.is_empty()
            || right.is_empty()
            || left == right
            || !edge.branch_length.is_finite()
            || edge.branch_length <= 0.0
            || !unique_edges.insert(key)
        {
            return Err(LongitudinalError::Invalid(
                "tree edges must be unique, non-self, positive, finite, and named".into(),
            ));
        }
        names.insert(left.to_owned());
        names.insert(right.to_owned());
    }
    if edges.len() + 1 != names.len() {
        return Err(LongitudinalError::Invalid(
            "tree must be acyclic with exactly node_count-1 edges".into(),
        ));
    }
    let node_indices = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| (name, index))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency = vec![Vec::new(); node_indices.len()];
    for edge in edges {
        let left = node_indices[edge.left_node.trim()];
        let right = node_indices[edge.right_node.trim()];
        adjacency[left].push((right, edge.branch_length));
        adjacency[right].push((left, edge.branch_length));
    }
    let mut seen = vec![false; adjacency.len()];
    let mut stack = vec![0];
    seen[0] = true;
    while let Some(node) = stack.pop() {
        for &(neighbor, _) in &adjacency[node] {
            if !seen[neighbor] {
                seen[neighbor] = true;
                stack.push(neighbor);
            }
        }
    }
    if seen.iter().any(|value| !value) {
        return Err(LongitudinalError::Invalid("tree must be connected".into()));
    }
    Ok(Tree {
        node_indices,
        adjacency,
    })
}

fn clone_tree_distances(
    tree: &Tree,
    clone_nodes: &[usize],
) -> Result<Vec<Vec<f64>>, LongitudinalError> {
    let mut result = vec![vec![0.0; clone_nodes.len()]; clone_nodes.len()];
    for (source_clone, &source_node) in clone_nodes.iter().enumerate() {
        let mut distances = vec![f64::NAN; tree.adjacency.len()];
        distances[source_node] = 0.0;
        let mut stack = vec![(source_node, usize::MAX)];
        while let Some((node, parent)) = stack.pop() {
            for &(neighbor, length) in &tree.adjacency[node] {
                if neighbor != parent {
                    distances[neighbor] = distances[node] + length;
                    stack.push((neighbor, node));
                }
            }
        }
        for (target_clone, &target_node) in clone_nodes.iter().enumerate() {
            let distance = distances[target_node];
            if !distance.is_finite() {
                return Err(LongitudinalError::Numerical(
                    "tree path distance is not finite".into(),
                ));
            }
            result[source_clone][target_clone] = distance;
        }
    }
    Ok(result)
}

fn blocks(clones: &[CloneSpatialRecord]) -> BTreeMap<(String, String), Vec<usize>> {
    let mut result = BTreeMap::<(String, String), Vec<usize>>::new();
    for (index, clone) in clones.iter().enumerate() {
        result
            .entry((clone.patient_id.clone(), clone.specimen_id.clone()))
            .or_default()
            .push(index);
    }
    result
}

fn spatial_distance(left: &CloneSpatialRecord, right: &CloneSpatialRecord) -> f64 {
    left.centroid_um
        .iter()
        .zip(right.centroid_um)
        .map(|(left, right)| (left - right).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn correlation(left: &[f64], right: &[f64]) -> Result<f64, LongitudinalError> {
    let left_mean = left.iter().sum::<f64>() / left.len() as f64;
    let right_mean = right.iter().sum::<f64>() / right.len() as f64;
    let covariance = left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>();
    let left_ss = left
        .iter()
        .map(|value| (value - left_mean).powi(2))
        .sum::<f64>();
    let right_ss = right
        .iter()
        .map(|value| (value - right_mean).powi(2))
        .sum::<f64>();
    let statistic = covariance / (left_ss * right_ss).sqrt();
    if !statistic.is_finite() {
        return Err(LongitudinalError::Invalid(
            "tree and spatial pair distances must both have positive variation".into(),
        ));
    }
    Ok(statistic.clamp(-1.0, 1.0))
}

fn shuffle_assignments(assignment: &mut [usize], members: &[usize], rng: &mut ChaCha20Rng) {
    for position in (1..members.len()).rev() {
        let target = rng.gen_range(0..=position);
        assignment.swap(members[position], members[target]);
    }
}
