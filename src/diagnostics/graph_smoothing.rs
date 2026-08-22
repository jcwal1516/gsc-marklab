use crate::{
    errors::{MarklabError, Result},
    multimodal::{
        cells::FusedCell,
        labels::{PrimaryLabelEncoding, PrimaryLabelId},
    },
    neighborhood::{enrichment::LabelPair, graph::SpatialGraph},
    output::GraphSmoothingSummary,
};

use crate::output::GraphSmoothingLabelPairSummary;

#[cfg(test)]
pub fn graph_smoothing(
    cells: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
) -> Result<GraphSmoothingSummary> {
    let labels = PrimaryLabelEncoding::new(cells)?;
    graph_smoothing_with_labels(cells, &labels, graph, label_pairs)
}

pub(crate) fn graph_smoothing_with_labels(
    cells: &[FusedCell],
    labels: &PrimaryLabelEncoding,
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
) -> Result<GraphSmoothingSummary> {
    validate_input(cells, graph)?;
    if labels.len() != cells.len() {
        return Err(MarklabError::Validation(format!(
            "primary label encoding has {} entries for {} graph-smoothing cells",
            labels.len(),
            cells.len()
        )));
    }

    let mut embeddings = initial_embeddings(labels);
    let adjacency = adjacency(graph.n_nodes, graph);
    for _ in 0..2 {
        embeddings = propagate(&embeddings, labels.label_count(), &adjacency);
    }

    let label_pair_scores = label_pairs
        .iter()
        .map(|pair| score_pair(pair, labels, &embeddings, graph))
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
        label_count: labels.label_count(),
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
        return Err(MarklabError::Validation(format!(
            "graph-smoothing node count {} does not match fused cell count {}",
            graph.n_nodes,
            cells.len()
        )));
    }
    Ok(())
}

fn initial_embeddings(labels: &PrimaryLabelEncoding) -> Vec<f64> {
    let label_count = labels.label_count();
    let mut embeddings = vec![0.0; labels.len().saturating_mul(label_count)];
    for (node, label) in labels.ids().iter().copied().enumerate() {
        if let Some(label) = label {
            embeddings[node * label_count + label.as_usize()] = 1.0;
        }
    }
    embeddings
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

fn propagate(embeddings: &[f64], label_count: usize, adjacency: &[Vec<usize>]) -> Vec<f64> {
    let mut output = vec![0.0; embeddings.len()];
    for (node, neighbors) in adjacency.iter().enumerate() {
        let row_start = node * label_count;
        let row_end = row_start + label_count;
        if neighbors.is_empty() {
            output[row_start..row_end].copy_from_slice(&embeddings[row_start..row_end]);
            continue;
        }
        for label in 0..label_count {
            let neighbor_sum = neighbors
                .iter()
                .map(|neighbor| embeddings[*neighbor * label_count + label])
                .sum::<f64>();
            output[row_start + label] =
                0.5 * embeddings[row_start + label] + 0.5 * neighbor_sum / neighbors.len() as f64;
        }
    }
    output
}

fn score_pair(
    pair: &LabelPair,
    labels: &PrimaryLabelEncoding,
    embeddings: &[f64],
    graph: &SpatialGraph,
) -> GraphSmoothingLabelPairSummary {
    let encoded_pair = EncodedLabelPair::new(labels, pair);
    let observed_edges = graph
        .edges
        .iter()
        .filter(|edge| encoded_pair.matches(labels.id_at(edge.source), labels.id_at(edge.target)))
        .count();
    let Some(index_a) = encoded_pair.a.map(PrimaryLabelId::as_usize) else {
        return GraphSmoothingLabelPairSummary {
            label_a: pair.label_a.clone(),
            label_b: pair.label_b.clone(),
            observed_edges,
            message_passing_score: 0.0,
        };
    };
    let Some(index_b) = encoded_pair.b.map(PrimaryLabelId::as_usize) else {
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
            let left = edge.source * labels.label_count();
            let right = edge.target * labels.label_count();
            if encoded_pair.a == encoded_pair.b {
                embeddings[left + index_a] * embeddings[right + index_a]
            } else {
                embeddings[left + index_a] * embeddings[right + index_b]
                    + embeddings[left + index_b] * embeddings[right + index_a]
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

    fn matches(self, left: Option<PrimaryLabelId>, right: Option<PrimaryLabelId>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) if self.a == self.b => {
                self.a == Some(left) && self.b == Some(right)
            }
            (Some(left), Some(right)) => {
                (self.a == Some(left) && self.b == Some(right))
                    || (self.b == Some(left) && self.a == Some(right))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{multimodal::cells::CellSection, neighborhood::graph::SpatialEdge};

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
