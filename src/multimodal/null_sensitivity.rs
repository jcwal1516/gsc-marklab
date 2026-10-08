use serde::{Deserialize, Serialize};

use crate::{
    config::NeighborhoodNullModel,
    errors::Result,
    multimodal::{
        cells::{CellSection, FusedCell},
        labels::PrimaryLabelEncoding,
    },
    neighborhood::{
        enrichment::{edge_enrichment_with_strata_and_labels, LabelPair},
        graph::SpatialGraph,
    },
    output::NeighborhoodEnrichmentResult,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NullModelSensitivityResult {
    pub null_model: NeighborhoodNullModel,
    pub results: Vec<NeighborhoodEnrichmentResult>,
}

pub(super) struct NullModelAnalysisContext<'a> {
    pub(super) fused: &'a [FusedCell],
    pub(super) labels: &'a PrimaryLabelEncoding,
    pub(super) graph: &'a SpatialGraph,
    pub(super) label_pairs: &'a [LabelPair],
    pub(super) primary_source_section: &'a [NeighborhoodEnrichmentResult],
    pub(super) permutations: usize,
    pub(super) seed: u64,
}

pub(super) fn analyze_null_model(
    context: &NullModelAnalysisContext<'_>,
    null_model: NeighborhoodNullModel,
) -> Result<NullModelSensitivityResult> {
    let results = match null_model {
        NeighborhoodNullModel::SourceSection => context.primary_source_section.to_vec(),
        NeighborhoodNullModel::SourceSectionDensity => edge_enrichment_with_strata_and_labels(
            context.fused,
            context.labels,
            context.graph,
            context.label_pairs,
            context.permutations,
            context.seed,
            &source_section_density_strata(context.fused, context.graph),
        )?,
        NeighborhoodNullModel::SourceSectionCellClass => edge_enrichment_with_strata_and_labels(
            context.fused,
            context.labels,
            context.graph,
            context.label_pairs,
            context.permutations,
            context.seed,
            &source_section_cell_class_strata(context.fused, context.labels),
        )?,
        NeighborhoodNullModel::SourceSectionRegistrationQc => {
            edge_enrichment_with_strata_and_labels(
                context.fused,
                context.labels,
                context.graph,
                context.label_pairs,
                context.permutations,
                context.seed,
                &source_section_registration_qc_strata(context.fused, context.graph),
            )?
        }
    };
    Ok(NullModelSensitivityResult {
        null_model,
        results,
    })
}

fn source_section_density_strata(fused: &[FusedCell], graph: &SpatialGraph) -> Vec<u32> {
    let degrees = graph_degrees(fused.len(), graph);
    let mut sorted = degrees.clone();
    sorted.sort_unstable();
    let median_degree = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
    fused
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            section_code(cell.source_section) * 2 + u32::from(degrees[index] > median_degree)
        })
        .collect()
}

fn source_section_cell_class_strata(
    fused: &[FusedCell],
    labels: &PrimaryLabelEncoding,
) -> Vec<u64> {
    fused
        .iter()
        .enumerate()
        .map(|(index, cell)| match cell.source_section {
            CellSection::He => labels
                .id_at(index)
                .map_or(0, |id| u64::from(id.as_u32()) + 2),
            CellSection::Ihc => 1,
        })
        .collect()
}

fn source_section_registration_qc_strata(fused: &[FusedCell], graph: &SpatialGraph) -> Vec<u32> {
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
            section_code(cell.source_section) * 2 + u32::from(below_resolution_incident[index])
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

const fn section_code(section: CellSection) -> u32 {
    match section {
        CellSection::He => 0,
        CellSection::Ihc => 1,
    }
}
