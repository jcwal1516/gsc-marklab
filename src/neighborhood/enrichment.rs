#[cfg(feature = "cli")]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[cfg(feature = "cli")]
use crate::permutation::labels::deterministic_shuffle;
use crate::{
    errors::{MmrspaceError, Result},
    multimodal::cell_table::{primary_label, FusedCell},
    output::NeighborhoodEnrichmentResult,
    permutation::rng::splitmix64,
};

use super::{graph::SpatialGraph, label_permutation::shuffle_labels_within_sections};

#[derive(Clone, Debug, PartialEq, Eq)]
/// Label pair requested for undirected edge enrichment.
///
/// `LabelPair::new` canonicalizes label order, so output labels may use the
/// canonical undirected order rather than the caller's argument order.
pub struct LabelPair {
    pub label_a: String,
    pub label_b: String,
}

impl LabelPair {
    /// Build a label pair in canonical order for undirected edge enrichment.
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let a = a.into();
        let b = b.into();
        if a <= b {
            Self {
                label_a: a,
                label_b: b,
            }
        } else {
            Self {
                label_a: b,
                label_b: a,
            }
        }
    }
}

pub fn edge_enrichment(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    validate_config(label_pairs, permutations)?;
    validate_graph(cells, graph)?;

    let labels = cells.iter().map(primary_label).collect::<Vec<_>>();
    let sections = cells
        .iter()
        .map(|cell| cell.source_section)
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(label_pairs.len());

    for pair in label_pairs {
        let observed_edges = count_pair_edges(&labels, graph, pair);
        let null_counts = permuted_counts(&labels, &sections, graph, pair, permutations, seed);
        let expected_edges = mean(&null_counts);
        let null_sd = sample_standard_deviation(&null_counts, expected_edges);
        let p_value = one_sided_high_p_value(observed_edges, &null_counts);
        let z_score = if null_sd > 0.0 {
            (observed_edges as f64 - expected_edges) / null_sd
        } else {
            0.0
        };

        rows.push(NeighborhoodEnrichmentResult {
            label_a: pair.label_a.clone(),
            label_b: pair.label_b.clone(),
            observed_edges,
            expected_edges,
            enrichment_ratio: enrichment_ratio(observed_edges, expected_edges),
            z_score,
            p_value: Some(p_value),
            q_value: None,
        });
    }

    apply_benjamini_hochberg(&mut rows);
    Ok(rows)
}

#[cfg(feature = "cli")]
pub fn edge_enrichment_with_strata(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
    strata: &[String],
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    validate_config(label_pairs, permutations)?;
    validate_graph(cells, graph)?;
    if strata.len() != cells.len() {
        return Err(MmrspaceError::Validation(format!(
            "null-model stratum count {} does not match cell count {}",
            strata.len(),
            cells.len()
        )));
    }

    let labels = cells.iter().map(primary_label).collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(label_pairs.len());
    for pair in label_pairs {
        let observed_edges = count_pair_edges(&labels, graph, pair);
        let null_counts =
            permuted_counts_with_strata(&labels, strata, graph, pair, permutations, seed);
        let expected_edges = mean(&null_counts);
        let null_sd = sample_standard_deviation(&null_counts, expected_edges);
        let p_value = one_sided_high_p_value(observed_edges, &null_counts);
        let z_score = if null_sd > 0.0 {
            (observed_edges as f64 - expected_edges) / null_sd
        } else {
            0.0
        };

        rows.push(NeighborhoodEnrichmentResult {
            label_a: pair.label_a.clone(),
            label_b: pair.label_b.clone(),
            observed_edges,
            expected_edges,
            enrichment_ratio: enrichment_ratio(observed_edges, expected_edges),
            z_score,
            p_value: Some(p_value),
            q_value: None,
        });
    }

    apply_benjamini_hochberg(&mut rows);
    Ok(rows)
}

fn validate_config(label_pairs: &[LabelPair], permutations: usize) -> Result<()> {
    if permutations == 0 {
        return Err(MmrspaceError::Config(
            "neighborhood enrichment permutations must be greater than zero".into(),
        ));
    }

    for (index, pair) in label_pairs.iter().enumerate() {
        if pair.label_a.trim().is_empty() || pair.label_b.trim().is_empty() {
            return Err(MmrspaceError::Config(format!(
                "label pair labels must be non-empty for pair {index}"
            )));
        }
    }

    Ok(())
}

fn validate_graph(cells: &[FusedCell], graph: &SpatialGraph) -> Result<()> {
    if graph.n_nodes != cells.len() {
        return Err(MmrspaceError::Validation(format!(
            "spatial graph node count {} does not match cell count {}",
            graph.n_nodes,
            cells.len()
        )));
    }

    let mut seen_edges = BTreeSet::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.source >= cells.len() || edge.target >= cells.len() {
            return Err(MmrspaceError::Validation(format!(
                "spatial graph edge {index} references a node outside the cell slice"
            )));
        }
        if edge.source == edge.target {
            return Err(MmrspaceError::Validation(format!(
                "spatial graph edge {index} is a self-edge"
            )));
        }
        let normalized = normalized_edge(edge.source, edge.target);
        if !seen_edges.insert(normalized) {
            return Err(MmrspaceError::Validation(format!(
                "spatial graph edge {index} is duplicate or mirrored"
            )));
        }
        if edge.source > edge.target {
            return Err(MmrspaceError::Validation(format!(
                "spatial graph edge {index} is duplicate or mirrored because endpoints are not canonical"
            )));
        }
    }

    Ok(())
}

fn normalized_edge(source: usize, target: usize) -> (usize, usize) {
    if source < target {
        (source, target)
    } else {
        (target, source)
    }
}

fn permuted_counts(
    labels: &[Option<String>],
    sections: &[crate::multimodal::cell_table::CellSection],
    graph: &SpatialGraph,
    pair: &LabelPair,
    permutations: usize,
    seed: u64,
) -> Vec<usize> {
    let mut counts = Vec::with_capacity(permutations);
    let mut shuffled = labels.to_vec();

    for permutation in 0..permutations {
        shuffle_labels_within_sections(
            labels,
            sections,
            &mut shuffled,
            splitmix64(seed ^ permutation as u64),
        );
        counts.push(count_pair_edges(&shuffled, graph, pair));
    }

    counts
}

#[cfg(feature = "cli")]
fn permuted_counts_with_strata(
    labels: &[Option<String>],
    strata: &[String],
    graph: &SpatialGraph,
    pair: &LabelPair,
    permutations: usize,
    seed: u64,
) -> Vec<usize> {
    let mut counts = Vec::with_capacity(permutations);
    let mut shuffled = labels.to_vec();

    for permutation in 0..permutations {
        shuffle_labels_within_strata(
            labels,
            strata,
            &mut shuffled,
            splitmix64(seed ^ permutation as u64),
        );
        counts.push(count_pair_edges(&shuffled, graph, pair));
    }

    counts
}

#[cfg(feature = "cli")]
fn shuffle_labels_within_strata<T: Clone>(
    labels: &[T],
    strata: &[String],
    shuffled: &mut [T],
    seed: u64,
) {
    shuffled.clone_from_slice(labels);
    let mut groups = BTreeMap::<&str, Vec<usize>>::new();
    for (index, stratum) in strata.iter().enumerate() {
        groups.entry(stratum.as_str()).or_default().push(index);
    }

    for (offset, indices) in groups.values().enumerate() {
        let mut values = indices
            .iter()
            .map(|index| labels[*index].clone())
            .collect::<Vec<_>>();
        deterministic_shuffle(&mut values, splitmix64(seed ^ offset as u64));
        for (index, value) in indices.iter().zip(values) {
            shuffled[*index] = value;
        }
    }
}

fn count_pair_edges(labels: &[Option<String>], graph: &SpatialGraph, pair: &LabelPair) -> usize {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge_matches_pair(
                labels[edge.source].as_deref(),
                labels[edge.target].as_deref(),
                pair,
            )
        })
        .count()
}

fn edge_matches_pair(left: Option<&str>, right: Option<&str>, pair: &LabelPair) -> bool {
    match (left, right) {
        (Some(left), Some(right)) if pair.label_a == pair.label_b => {
            left == pair.label_a && right == pair.label_b
        }
        (Some(left), Some(right)) => {
            (left == pair.label_a && right == pair.label_b)
                || (left == pair.label_b && right == pair.label_a)
        }
        _ => false,
    }
}

fn mean(values: &[usize]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.iter().sum::<usize>() as f64 / values.len() as f64
}

fn sample_standard_deviation(values: &[usize], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let variance = values
        .iter()
        .map(|value| {
            let delta = *value as f64 - mean;
            delta * delta
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;

    if variance > 0.0 {
        variance.sqrt()
    } else {
        0.0
    }
}

fn one_sided_high_p_value(observed: usize, null_counts: &[usize]) -> f64 {
    let extreme_count = null_counts
        .iter()
        .filter(|count| **count >= observed)
        .count();
    (extreme_count as f64 + 1.0) / (null_counts.len() as f64 + 1.0)
}

fn enrichment_ratio(observed_edges: usize, expected_edges: f64) -> f64 {
    if expected_edges > 0.0 {
        observed_edges as f64 / expected_edges
    } else if observed_edges == 0 {
        0.0
    } else {
        f64::INFINITY
    }
}

fn apply_benjamini_hochberg(rows: &mut [NeighborhoodEnrichmentResult]) {
    let mut indexed_p_values: Vec<_> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.p_value.map(|p_value| (index, p_value)))
        .collect();
    indexed_p_values.sort_by(|left, right| left.1.total_cmp(&right.1));

    let m = indexed_p_values.len();
    let mut next_q = 1.0;
    for (rank_from_zero, (index, p_value)) in indexed_p_values.into_iter().enumerate().rev() {
        let rank = rank_from_zero + 1;
        let q_value = (p_value * m as f64 / rank as f64).min(next_q).min(1.0);
        rows[index].q_value = Some(q_value);
        next_q = q_value;
    }
}
