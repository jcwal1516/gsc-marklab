use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::{
    common::{
        seeds::{derive_seed, splitmix64, SeedEndpoint},
        stats::{mean_all_finite, safe_finite_ratio, sample_standard_deviation},
    },
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value_with_spec, PermutationTestSpec, Tail},
    multimodal::{
        cells::{CellSection, FusedCell},
        labels::primary_label,
    },
    output::{EnrichmentStatisticUnavailableReason, NeighborhoodEnrichmentResult},
    permutation::labels::deterministic_shuffle,
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
    let sections = cells
        .iter()
        .map(|cell| cell.source_section)
        .collect::<Vec<_>>();
    edge_enrichment_with_strategy(
        cells,
        graph,
        label_pairs,
        permutations,
        seed,
        PermutationStrategy::SourceSections(&sections),
    )
}

pub fn edge_enrichment_with_strata(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
    strata: &[String],
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    edge_enrichment_with_strategy(
        cells,
        graph,
        label_pairs,
        permutations,
        seed,
        PermutationStrategy::ExplicitStrata(strata),
    )
}

#[derive(Clone, Copy)]
enum PermutationStrategy<'a> {
    SourceSections(&'a [CellSection]),
    ExplicitStrata(&'a [String]),
}

fn edge_enrichment_with_strategy(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
    strategy: PermutationStrategy<'_>,
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    validate_config(label_pairs, permutations)?;
    validate_graph(cells, graph)?;
    let strategy_len = match strategy {
        PermutationStrategy::SourceSections(sections) => sections.len(),
        PermutationStrategy::ExplicitStrata(strata) => strata.len(),
    };
    if strategy_len != cells.len() {
        return Err(MarklabError::Validation(format!(
            "null-model stratum count {} does not match cell count {}",
            strategy_len,
            cells.len()
        )));
    }

    let labels = cells.iter().map(primary_label).collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(label_pairs.len());
    for pair in label_pairs {
        let observed_edges = count_pair_edges(&labels, graph, pair);
        let null_counts = permuted_counts(&labels, graph, pair, permutations, seed, strategy);
        rows.push(enrichment_result(pair, observed_edges, &null_counts)?);
    }

    apply_benjamini_hochberg(&mut rows);
    Ok(rows)
}

fn validate_config(label_pairs: &[LabelPair], permutations: usize) -> Result<()> {
    if permutations == 0 {
        return Err(MarklabError::Config(
            "neighborhood enrichment permutations must be greater than zero".into(),
        ));
    }

    for (index, pair) in label_pairs.iter().enumerate() {
        if pair.label_a.trim().is_empty() || pair.label_b.trim().is_empty() {
            return Err(MarklabError::Config(format!(
                "label pair labels must be non-empty for pair {index}"
            )));
        }
    }

    Ok(())
}

fn validate_graph(cells: &[FusedCell], graph: &SpatialGraph) -> Result<()> {
    if graph.n_nodes != cells.len() {
        return Err(MarklabError::Validation(format!(
            "spatial graph node count {} does not match cell count {}",
            graph.n_nodes,
            cells.len()
        )));
    }

    let mut seen_edges = BTreeSet::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.source >= cells.len() || edge.target >= cells.len() {
            return Err(MarklabError::Validation(format!(
                "spatial graph edge {index} references a node outside the cell slice"
            )));
        }
        if edge.source == edge.target {
            return Err(MarklabError::Validation(format!(
                "spatial graph edge {index} is a self-edge"
            )));
        }
        let normalized = normalized_edge(edge.source, edge.target);
        if !seen_edges.insert(normalized) {
            return Err(MarklabError::Validation(format!(
                "spatial graph edge {index} is duplicate or mirrored"
            )));
        }
        if edge.source > edge.target {
            return Err(MarklabError::Validation(format!(
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
    labels: &[Option<&str>],
    graph: &SpatialGraph,
    pair: &LabelPair,
    permutations: usize,
    seed: u64,
    strategy: PermutationStrategy<'_>,
) -> Vec<usize> {
    let mut counts = Vec::with_capacity(permutations);
    let mut shuffled = labels.to_vec();

    for permutation in 0..permutations {
        match strategy {
            PermutationStrategy::SourceSections(sections) => shuffle_labels_within_sections(
                labels,
                sections,
                &mut shuffled,
                derive_seed(seed, SeedEndpoint::NeighborhoodEnrichment, permutation),
            ),
            PermutationStrategy::ExplicitStrata(strata) => shuffle_labels_within_strata(
                labels,
                strata,
                &mut shuffled,
                derive_seed(
                    seed,
                    SeedEndpoint::NeighborhoodStratifiedEnrichment,
                    permutation,
                ),
            ),
        }
        counts.push(count_pair_edges(&shuffled, graph, pair));
    }

    counts
}

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

fn count_pair_edges(labels: &[Option<&str>], graph: &SpatialGraph, pair: &LabelPair) -> usize {
    graph
        .edges
        .iter()
        .filter(|edge| edge_matches_pair(labels[edge.source], labels[edge.target], pair))
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

fn enrichment_result(
    pair: &LabelPair,
    observed_edges: usize,
    null_counts: &[usize],
) -> Result<NeighborhoodEnrichmentResult> {
    let null_counts_f64 = null_counts
        .iter()
        .map(|count| *count as f64)
        .collect::<Vec<_>>();
    let expected_edges = mean_all_finite(null_counts_f64.iter().copied())
        .ok_or_else(|| MarklabError::Compute("enrichment null mean is undefined".into()))?;
    let (enrichment_ratio, enrichment_ratio_unavailable_reason) =
        match safe_finite_ratio(observed_edges as f64, expected_edges) {
            Some(ratio) => (Some(ratio), None),
            None if expected_edges == 0.0 => (
                None,
                Some(EnrichmentStatisticUnavailableReason::ZeroExpectedEdges),
            ),
            None => (
                None,
                Some(EnrichmentStatisticUnavailableReason::NonFiniteComputation),
            ),
        };
    let (z_score, z_score_unavailable_reason) = match sample_standard_deviation(&null_counts_f64) {
        Some(null_sd) if null_sd > 0.0 => {
            match safe_finite_ratio(observed_edges as f64 - expected_edges, null_sd) {
                Some(z_score) => (Some(z_score), None),
                None => (
                    None,
                    Some(EnrichmentStatisticUnavailableReason::NonFiniteComputation),
                ),
            }
        }
        Some(_) => (
            None,
            Some(EnrichmentStatisticUnavailableReason::ZeroNullVariance),
        ),
        None => (
            None,
            Some(EnrichmentStatisticUnavailableReason::InsufficientNullSamples),
        ),
    };
    let p_value = permutation_p_value_with_spec(
        observed_edges as f64,
        &null_counts_f64,
        PermutationTestSpec::new(Tail::OneSidedHigh, 1),
    )?;

    Ok(NeighborhoodEnrichmentResult {
        label_a: pair.label_a.clone(),
        label_b: pair.label_b.clone(),
        observed_edges,
        expected_edges,
        enrichment_ratio,
        enrichment_ratio_unavailable_reason,
        z_score,
        z_score_unavailable_reason,
        p_value: Some(p_value),
        q_value: None,
    })
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
