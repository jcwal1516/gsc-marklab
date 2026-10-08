use std::collections::BTreeSet;

use crate::{
    common::{
        seeds::{derive_seed, SeedEndpoint},
        stats::{mean_all_finite, safe_finite_ratio, sample_standard_deviation},
    },
    errors::{MarklabError, Result},
    inference::{
        multiple_testing::benjamini_hochberg,
        scalar_pvalues::{permutation_p_value_with_spec, PermutationTestSpec, Tail},
    },
    multimodal::{
        cells::FusedCell,
        labels::{PrimaryLabelEncoding, PrimaryLabelId},
    },
    output::{EnrichmentStatisticUnavailableReason, NeighborhoodEnrichmentResult},
};

use super::{graph::SpatialGraph, label_permutation::LabelPermutationPlan};

struct EnrichmentExecution<'a> {
    cells: &'a [FusedCell],
    labels: &'a PrimaryLabelEncoding,
    graph: &'a SpatialGraph,
    label_pairs: &'a [LabelPair],
    permutations: usize,
    seed: u64,
}

struct PermutationExecution<'a> {
    labels: &'a [Option<PrimaryLabelId>],
    graph: &'a SpatialGraph,
    permutations: usize,
    seed: u64,
    plan: &'a LabelPermutationPlan,
    seed_endpoint: SeedEndpoint,
}

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

#[cfg(test)]
pub fn edge_enrichment(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    let labels = PrimaryLabelEncoding::new(cells)?;
    edge_enrichment_with_labels(cells, &labels, graph, label_pairs, permutations, seed)
}

pub(crate) fn edge_enrichment_with_labels(
    cells: &[FusedCell],
    labels: &PrimaryLabelEncoding,
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    let sections = cells
        .iter()
        .map(|cell| cell.source_section)
        .collect::<Vec<_>>();
    let plan = LabelPermutationPlan::for_source_sections(&sections);
    edge_enrichment_with_plan(
        EnrichmentExecution {
            cells,
            labels,
            graph,
            label_pairs,
            permutations,
            seed,
        },
        &plan,
        SeedEndpoint::NeighborhoodEnrichment,
    )
}

#[cfg(test)]
pub fn edge_enrichment_with_strata(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
    strata: &[String],
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    let labels = PrimaryLabelEncoding::new(cells)?;
    edge_enrichment_with_strata_and_labels(
        cells,
        &labels,
        graph,
        label_pairs,
        permutations,
        seed,
        strata,
    )
}

pub(crate) fn edge_enrichment_with_strata_and_labels<T: Ord>(
    cells: &[FusedCell],
    labels: &PrimaryLabelEncoding,
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    permutations: usize,
    seed: u64,
    strata: &[T],
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    if strata.len() != cells.len() {
        return Err(MarklabError::Validation(format!(
            "null-model stratum count {} does not match cell count {}",
            strata.len(),
            cells.len()
        )));
    }
    let plan = LabelPermutationPlan::for_explicit_strata(strata);
    edge_enrichment_with_plan(
        EnrichmentExecution {
            cells,
            labels,
            graph,
            label_pairs,
            permutations,
            seed,
        },
        &plan,
        SeedEndpoint::NeighborhoodStratifiedEnrichment,
    )
}

fn edge_enrichment_with_plan(
    execution: EnrichmentExecution<'_>,
    permutation_plan: &LabelPermutationPlan,
    seed_endpoint: SeedEndpoint,
) -> Result<Vec<NeighborhoodEnrichmentResult>> {
    validate_config(execution.label_pairs, execution.permutations)?;
    validate_graph(execution.cells, execution.graph)?;
    if execution.labels.len() != execution.cells.len() {
        return Err(MarklabError::Validation(format!(
            "primary label count {} does not match cell count {}",
            execution.labels.len(),
            execution.cells.len()
        )));
    }

    let mut rows = Vec::with_capacity(execution.label_pairs.len());
    let mut shuffled = Vec::with_capacity(execution.labels.len());
    let mut group_scratch = Vec::with_capacity(permutation_plan.maximum_group_size());
    let permutation_execution = PermutationExecution {
        labels: execution.labels.ids(),
        graph: execution.graph,
        permutations: execution.permutations,
        seed: execution.seed,
        plan: permutation_plan,
        seed_endpoint,
    };
    for pair in execution.label_pairs {
        let encoded_pair = EncodedLabelPair::new(execution.labels, pair);
        let observed_edges =
            count_pair_edges(execution.labels.ids(), execution.graph, encoded_pair);
        let null_counts = permuted_counts(
            &permutation_execution,
            encoded_pair,
            &mut shuffled,
            &mut group_scratch,
        )?;
        rows.push(enrichment_result(pair, observed_edges, &null_counts)?);
    }

    let adjusted = benjamini_hochberg(
        &rows
            .iter()
            .map(|row| row.p_value.expect("enrichment always computes a p-value"))
            .collect::<Vec<_>>(),
    )?;
    for (row, q_value) in rows.iter_mut().zip(adjusted) {
        row.q_value = Some(q_value);
    }
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
    execution: &PermutationExecution<'_>,
    pair: EncodedLabelPair,
    shuffled: &mut Vec<Option<PrimaryLabelId>>,
    group_scratch: &mut Vec<Option<PrimaryLabelId>>,
) -> Result<Vec<usize>> {
    let mut counts = Vec::with_capacity(execution.permutations);

    for permutation in 0..execution.permutations {
        execution.plan.permute_into(
            execution.labels,
            derive_seed(execution.seed, execution.seed_endpoint, permutation),
            shuffled,
            group_scratch,
        )?;
        counts.push(count_pair_edges(shuffled, execution.graph, pair));
    }

    Ok(counts)
}

fn count_pair_edges(
    labels: &[Option<PrimaryLabelId>],
    graph: &SpatialGraph,
    pair: EncodedLabelPair,
) -> usize {
    graph
        .edges
        .iter()
        .filter(|edge| edge_matches_pair(labels[edge.source], labels[edge.target], pair))
        .count()
}

fn edge_matches_pair(
    left: Option<PrimaryLabelId>,
    right: Option<PrimaryLabelId>,
    pair: EncodedLabelPair,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) if pair.a == pair.b => {
            pair.a == Some(left) && pair.b == Some(right)
        }
        (Some(left), Some(right)) => {
            (pair.a == Some(left) && pair.b == Some(right))
                || (pair.b == Some(left) && pair.a == Some(right))
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct EncodedLabelPair {
    a: Option<PrimaryLabelId>,
    b: Option<PrimaryLabelId>,
}

impl EncodedLabelPair {
    fn new(labels: &PrimaryLabelEncoding, pair: &LabelPair) -> Self {
        Self {
            a: labels.id_for(&pair.label_a),
            b: labels.id_for(&pair.label_b),
        }
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
