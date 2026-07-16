use std::collections::BTreeMap;

use crate::{
    errors::{MmrspaceError, Result},
    multimodal::cell_table::FusedCell,
    neighborhood::{enrichment::LabelPair, graph::SpatialGraph},
    output::GraphSmoothingSummary,
};

use crate::multimodal::cell_table::primary_label;
use crate::output::GraphSmoothingLabelPairSummary;

pub fn graph_smoothing(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
) -> Result<GraphSmoothingSummary> {
    validate_input(cells, graph)?;

    let labels = cells
        .iter()
        .map(primary_label)
        .collect::<Vec<Option<String>>>();
    let label_index = label_index(&labels);
    let mut embeddings = initial_embeddings(&labels, &label_index);
    let adjacency = adjacency(graph.n_nodes, graph);
    for _ in 0..2 {
        embeddings = propagate(&embeddings, &adjacency);
    }

    let label_pair_scores = label_pairs
        .iter()
        .map(|pair| score_pair(pair, &labels, &label_index, &embeddings, graph))
        .collect::<Vec<_>>();
    let n_edges = graph.edges.len();
    let below_registration_resolution_edge_fraction = if n_edges == 0 {
        0.0
    } else {
        graph
            .edges
            .iter()
            .filter(|edge| edge.below_registration_resolution)
            .count() as f64
            / n_edges as f64
    };

    Ok(GraphSmoothingSummary {
        diagnostic_name: "deterministic_graph_smoothing_v1".into(),
        n_nodes: graph.n_nodes,
        n_edges,
        mean_degree: if graph.n_nodes == 0 {
            0.0
        } else {
            2.0 * n_edges as f64 / graph.n_nodes as f64
        },
        below_registration_resolution_edge_fraction,
        label_count: label_index.len(),
        label_pair_scores,
        diagnostics: vec![
            "Deterministic two-layer graph message passing diagnostic, not a trained ML backend."
                .into(),
            "Diagnostic output is exploratory and does not change neighborhood inference.".into(),
        ],
    })
}

fn validate_input(cells: &[FusedCell], graph: &SpatialGraph) -> Result<()> {
    if graph.n_nodes != cells.len() {
        return Err(MmrspaceError::Validation(format!(
            "graph-smoothing node count {} does not match fused cell count {}",
            graph.n_nodes,
            cells.len()
        )));
    }
    Ok(())
}

fn label_index(labels: &[Option<String>]) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for label in labels.iter().flatten() {
        let next = index.len();
        index.entry(label.clone()).or_insert(next);
    }
    index
}

fn initial_embeddings(
    labels: &[Option<String>],
    label_index: &BTreeMap<String, usize>,
) -> Vec<Vec<f64>> {
    labels
        .iter()
        .map(|label| {
            let mut row = vec![0.0; label_index.len()];
            if let Some(index) = label.as_ref().and_then(|label| label_index.get(label)) {
                row[*index] = 1.0;
            }
            row
        })
        .collect()
}

fn adjacency(n_nodes: usize, graph: &SpatialGraph) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); n_nodes];
    for edge in &graph.edges {
        if edge.source < n_nodes && edge.target < n_nodes {
            adjacency[edge.source].push(edge.target);
            adjacency[edge.target].push(edge.source);
        }
    }
    adjacency
}

fn propagate(embeddings: &[Vec<f64>], adjacency: &[Vec<usize>]) -> Vec<Vec<f64>> {
    embeddings
        .iter()
        .enumerate()
        .map(|(node, embedding)| {
            if adjacency[node].is_empty() {
                return embedding.clone();
            }

            let mut neighbor_mean = vec![0.0; embedding.len()];
            for neighbor in &adjacency[node] {
                for (slot, value) in neighbor_mean.iter_mut().zip(&embeddings[*neighbor]) {
                    *slot += *value;
                }
            }
            for value in &mut neighbor_mean {
                *value /= adjacency[node].len() as f64;
            }
            embedding
                .iter()
                .zip(neighbor_mean)
                .map(|(self_value, neighbor_value)| 0.5 * self_value + 0.5 * neighbor_value)
                .collect()
        })
        .collect()
}

fn score_pair(
    pair: &LabelPair,
    labels: &[Option<String>],
    label_index: &BTreeMap<String, usize>,
    embeddings: &[Vec<f64>],
    graph: &SpatialGraph,
) -> GraphSmoothingLabelPairSummary {
    let observed_edges = graph
        .edges
        .iter()
        .filter(|edge| labels_match_pair(labels, edge.source, edge.target, pair))
        .count();
    let Some(index_a) = label_index.get(&pair.label_a).copied() else {
        return GraphSmoothingLabelPairSummary {
            label_a: pair.label_a.clone(),
            label_b: pair.label_b.clone(),
            observed_edges,
            message_passing_score: 0.0,
        };
    };
    let Some(index_b) = label_index.get(&pair.label_b).copied() else {
        return GraphSmoothingLabelPairSummary {
            label_a: pair.label_a.clone(),
            label_b: pair.label_b.clone(),
            observed_edges,
            message_passing_score: 0.0,
        };
    };
    let score_sum = graph
        .edges
        .iter()
        .map(|edge| {
            let left = &embeddings[edge.source];
            let right = &embeddings[edge.target];
            if pair.label_a == pair.label_b {
                left[index_a] * right[index_a]
            } else {
                left[index_a] * right[index_b] + left[index_b] * right[index_a]
            }
        })
        .sum::<f64>();

    GraphSmoothingLabelPairSummary {
        label_a: pair.label_a.clone(),
        label_b: pair.label_b.clone(),
        observed_edges,
        message_passing_score: if graph.edges.is_empty() {
            0.0
        } else {
            score_sum / graph.edges.len() as f64
        },
    }
}

fn labels_match_pair(
    labels: &[Option<String>],
    source: usize,
    target: usize,
    pair: &LabelPair,
) -> bool {
    let left = labels.get(source).and_then(Option::as_deref);
    let right = labels.get(target).and_then(Option::as_deref);
    match (left, right) {
        (Some(left), Some(right)) if pair.label_a == pair.label_b => {
            left == pair.label_a && right == pair.label_a
        }
        (Some(left), Some(right)) => {
            (left == pair.label_a && right == pair.label_b)
                || (left == pair.label_b && right == pair.label_a)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{multimodal::cell_table::CellSection, neighborhood::graph::SpatialEdge};

    fn cell(
        source_section: CellSection,
        source_cell_id: &str,
        x_um_registered: f64,
        mmr_mark: Option<u8>,
        cell_type: Option<&str>,
    ) -> FusedCell {
        FusedCell {
            source_section,
            source_cell_id: source_cell_id.into(),
            x_um_registered,
            y_um_registered: 0.0,
            mmr_mark,
            mmr_probability: None,
            cell_type: cell_type.map(str::to_owned),
            cell_type_probability: cell_type.map(|_| 0.9),
            same_section: false,
            registration_error_um: Some(1.0),
            timepoint: "post".into(),
            case_id: "case_001".into(),
            protein: "MSH6".into(),
        }
    }

    #[test]
    fn returns_deterministic_graph_scores() {
        let cells = vec![
            cell(CellSection::He, "h1", 0.0, None, Some("lymphocyte")),
            cell(CellSection::Ihc, "m1", 1.0, Some(1), None),
            cell(CellSection::Ihc, "m2", 2.0, Some(0), None),
        ];
        let graph = SpatialGraph {
            n_nodes: 3,
            edges: vec![
                SpatialEdge {
                    source: 0,
                    target: 1,
                    distance_um: 1.0,
                    angle_rad: 0.0,
                    below_registration_resolution: true,
                },
                SpatialEdge {
                    source: 1,
                    target: 2,
                    distance_um: 1.0,
                    angle_rad: 0.0,
                    below_registration_resolution: true,
                },
            ],
        };
        let label_pairs = vec![LabelPair::new("mmr_abnormal", "lymphocyte")];

        let output =
            graph_smoothing(&cells, &graph, &label_pairs).expect("graph-smoothing diagnostic");

        assert_eq!(output.diagnostic_name, "deterministic_graph_smoothing_v1");
        assert_eq!(output.n_nodes, 3);
        assert_eq!(output.n_edges, 2);
        assert_eq!(output.label_count, 3);
        assert_eq!(output.label_pair_scores[0].observed_edges, 1);
        assert!(output.label_pair_scores[0].message_passing_score > 0.0);
        assert_eq!(output.below_registration_resolution_edge_fraction, 1.0);
    }
}
