use serde::{Deserialize, Serialize};

use crate::{
    config::NeighborhoodNullModel,
    errors::Result,
    multimodal::cell_table::{primary_label, CellSection, FusedCell},
    neighborhood::{
        enrichment::{edge_enrichment_with_strata, LabelPair},
        graph::SpatialGraph,
    },
    output::NeighborhoodEnrichmentResult,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NullModelSensitivityResult {
    pub null_model: NeighborhoodNullModel,
    pub results: Vec<NeighborhoodEnrichmentResult>,
}

pub(super) fn analyze_null_model_sensitivity(
    fused: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    null_models: &[NeighborhoodNullModel],
    primary_source_section: &[NeighborhoodEnrichmentResult],
    permutations: usize,
    seed: u64,
) -> Result<Vec<NullModelSensitivityResult>> {
    null_models
        .iter()
        .copied()
        .map(|null_model| {
            let results = match null_model {
                NeighborhoodNullModel::SourceSection => primary_source_section.to_vec(),
                NeighborhoodNullModel::SourceSectionDensity => edge_enrichment_with_strata(
                    fused,
                    graph,
                    label_pairs,
                    permutations,
                    seed,
                    &source_section_density_strata(fused, graph),
                )?,
                NeighborhoodNullModel::SourceSectionCellClass => edge_enrichment_with_strata(
                    fused,
                    graph,
                    label_pairs,
                    permutations,
                    seed,
                    &source_section_cell_class_strata(fused),
                )?,
                NeighborhoodNullModel::SourceSectionRegistrationQc => edge_enrichment_with_strata(
                    fused,
                    graph,
                    label_pairs,
                    permutations,
                    seed,
                    &source_section_registration_qc_strata(fused, graph),
                )?,
            };
            Ok(NullModelSensitivityResult {
                null_model,
                results,
            })
        })
        .collect()
}

fn source_section_density_strata(fused: &[FusedCell], graph: &SpatialGraph) -> Vec<String> {
    let degrees = graph_degrees(fused.len(), graph);
    let mut sorted = degrees.clone();
    sorted.sort_unstable();
    let median_degree = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
    fused
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            format!(
                "{}:{}",
                section_name(cell.source_section),
                if degrees[index] <= median_degree {
                    "low_density"
                } else {
                    "high_density"
                }
            )
        })
        .collect()
}

fn source_section_cell_class_strata(fused: &[FusedCell]) -> Vec<String> {
    fused
        .iter()
        .map(|cell| match cell.source_section {
            CellSection::He => format!(
                "he:{}",
                primary_label(cell).unwrap_or_else(|| "unknown".into())
            ),
            CellSection::Ihc => "ihc:mmr_status".into(),
        })
        .collect()
}

fn source_section_registration_qc_strata(fused: &[FusedCell], graph: &SpatialGraph) -> Vec<String> {
    let mut below_resolution_incident = vec![false; fused.len()];
    for edge in &graph.edges {
        if edge.below_registration_resolution {
            below_resolution_incident[edge.source] = true;
            below_resolution_incident[edge.target] = true;
        }
    }
    fused
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            format!(
                "{}:{}",
                section_name(cell.source_section),
                if below_resolution_incident[index] {
                    "below_resolution_edge"
                } else {
                    "above_resolution_edges"
                }
            )
        })
        .collect()
}

fn graph_degrees(n_cells: usize, graph: &SpatialGraph) -> Vec<usize> {
    let mut degrees = vec![0usize; n_cells];
    for edge in &graph.edges {
        degrees[edge.source] += 1;
        degrees[edge.target] += 1;
    }
    degrees
}

const fn section_name(section: CellSection) -> &'static str {
    match section {
        CellSection::He => "he",
        CellSection::Ihc => "ihc",
    }
}
