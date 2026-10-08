use std::mem::size_of;

use crate::{
    config::{AnalysisConfig, NeighborhoodNullModel},
    errors::{MarklabError, Result},
    multimodal::{
        cells::{FusedCell, HeCell, IhcCell},
        registration_artifacts::{CellExtrapolationRecord, RegistrationResidual},
    },
    neighborhood::enrichment::LabelPair,
    output::{
        CrossInteractionCurve, CurveComparisonResult, GraphSmoothingLabelPairSummary,
        GraphSmoothingSummary, LabelFraction, NeighborhoodEnrichmentResult, NeighborhoodTerritory,
        TerritoryProfile, TimingStage,
    },
    registration::landmarks::LandmarkPair,
};

use super::{engine::MultimodalInput, labels::PrimaryLabelEncoding};

/// Conservative accounting for heap storage retained by one multimodal run.
///
/// Reservations are made before allocations. Sequential stage scratch is
/// observed against the already retained total and may raise the peak without
/// becoming part of the retained total.
#[derive(Clone, Debug)]
pub(super) struct MultimodalMemoryBudget {
    limit_bytes: usize,
    retained_bytes: usize,
    peak_bytes: usize,
}

impl MultimodalMemoryBudget {
    pub(super) fn for_run(config: &AnalysisConfig, input: &MultimodalInput) -> Result<Self> {
        let metadata = [
            input.case_id.as_str(),
            input.timepoint.as_str(),
            input.protein.as_str(),
        ];
        let mut budget = Self {
            limit_bytes: config
                .performance
                .memory_budget_mib
                .saturating_mul(1024 * 1024),
            retained_bytes: 0,
            peak_bytes: 0,
        };
        budget.reserve_retained(
            "input",
            input_storage_bytes(
                &input.he_cells,
                input.he_cells.capacity(),
                &input.ihc_cells,
                input.ihc_cells.capacity(),
                input.landmarks.capacity(),
                metadata,
            ),
        )?;
        budget.reserve_retained(
            "timing telemetry",
            timing_storage_bytes(config.neighborhood.null_models.len()),
        )?;
        budget.reserve_retained(
            "analysis metadata",
            analysis_metadata_storage_bytes(metadata),
        )?;
        budget.reserve_retained(
            "fused cells",
            projected_fused_storage_bytes(&input.he_cells, &input.ihc_cells),
        )?;
        Ok(budget)
    }

    pub(super) fn reserve_label_encoding_and_index(
        &mut self,
        fused: &[FusedCell],
    ) -> Result<usize> {
        let label_storage = PrimaryLabelEncoding::estimated_storage_upper_bound_for_cells(fused);
        self.reserve_retained("primary label encoding", label_storage)?;
        self.observe_transient("primary label encoding construction", label_storage)?;
        self.reserve_retained(
            "spatial index",
            crate::geom::spatial_index::SpatialIndex2D::estimated_storage_bytes_for_len(
                fused.len(),
            ),
        )?;
        self.observe_transient("spatial index construction", fused.len().saturating_mul(32))?;
        Ok(label_storage)
    }

    pub(super) fn reserve_configured_label_pairs(
        &mut self,
        label_pairs: &[[String; 2]],
    ) -> Result<()> {
        self.reserve_retained(
            "configured label pairs",
            configured_label_pair_storage_bytes(label_pairs),
        )
    }

    pub(super) fn reserve_registration_and_enrichment_results(
        &mut self,
        input: &MultimodalInput,
        fused: &[FusedCell],
        label_pairs: &[LabelPair],
        null_model_count: usize,
    ) -> Result<()> {
        self.reserve_retained(
            "registration artifacts",
            registration_artifact_storage_bytes(&input.landmarks, fused),
        )?;
        self.reserve_retained(
            "enrichment results",
            enrichment_retained_storage_bytes(label_pairs, 1usize.saturating_add(null_model_count)),
        )
    }

    pub(super) fn observe_registration_scratch(&mut self, landmark_count: usize) -> Result<()> {
        self.observe_transient(
            "registration artifact construction",
            registration_artifact_scratch_bytes(landmark_count),
        )
    }

    pub(super) fn observe_enrichment_scratch(
        &mut self,
        cell_count: usize,
        permutation_count: usize,
    ) -> Result<()> {
        self.observe_transient(
            "enrichment permutation scratch",
            enrichment_scratch_bytes(cell_count, permutation_count),
        )
    }

    pub(super) fn reserve_cross_curves(&mut self, curves: &[CrossInteractionCurve]) -> Result<()> {
        self.reserve_retained(
            "cross-interaction results",
            cross_curve_retained_storage_bytes(curves),
        )
    }

    pub(super) fn reserve_territories(
        &mut self,
        territories: &[NeighborhoodTerritory],
    ) -> Result<()> {
        self.reserve_retained(
            "neighborhood territory results",
            territory_retained_storage_bytes(territories),
        )
    }

    pub(super) fn reserve_profiles_and_comparisons(
        &mut self,
        territory_count: usize,
        labels: &PrimaryLabelEncoding,
    ) -> Result<()> {
        self.reserve_retained(
            "territory profiles and comparisons",
            profile_and_comparison_storage_upper_bound(
                territory_count,
                labels.label_count(),
                labels.total_name_bytes(),
            ),
        )?;
        self.observe_transient(
            "territory profile and comparison scratch",
            profile_and_comparison_scratch_bytes(labels.label_count(), labels.total_name_bytes()),
        )
    }

    pub(super) fn reserve_graph_smoothing(
        &mut self,
        node_count: usize,
        edge_count: usize,
        labels: &PrimaryLabelEncoding,
        label_pairs: &[LabelPair],
    ) -> Result<()> {
        let (retained, peak) = graph_smoothing_storage_bytes(
            node_count,
            edge_count,
            labels.label_count(),
            label_pairs,
        );
        self.observe_transient("graph-smoothing execution", peak)?;
        self.reserve_retained("graph-smoothing result", retained)
    }

    pub(super) fn reserve_interpretation(&mut self, interpretation: &str) -> Result<()> {
        self.reserve_retained("result interpretation", interpretation.len())
    }

    pub(super) fn reserve_retained(&mut self, label: &str, bytes: usize) -> Result<()> {
        let projected = self.retained_bytes.saturating_add(bytes);
        self.enforce(label, projected)?;
        self.retained_bytes = projected;
        self.peak_bytes = self.peak_bytes.max(projected);
        Ok(())
    }

    pub(super) fn observe_transient(&mut self, label: &str, bytes: usize) -> Result<()> {
        let projected = self.retained_bytes.saturating_add(bytes);
        self.enforce(label, projected)?;
        self.peak_bytes = self.peak_bytes.max(projected);
        Ok(())
    }

    pub(super) fn remaining_bytes(&self) -> usize {
        self.limit_bytes.saturating_sub(self.retained_bytes)
    }

    pub(super) fn peak_mib(&self) -> f64 {
        self.peak_bytes.max(1) as f64 / (1024.0 * 1024.0)
    }

    fn enforce(&self, label: &str, projected_bytes: usize) -> Result<()> {
        if projected_bytes > self.limit_bytes {
            return Err(MarklabError::Validation(format!(
                "estimated multimodal {label} peak storage {projected_bytes} bytes exceeds configured memory budget {} bytes",
                self.limit_bytes
            )));
        }
        Ok(())
    }
}

pub(super) fn input_storage_bytes(
    he_cells: &[HeCell],
    he_capacity: usize,
    ihc_cells: &[IhcCell],
    ihc_capacity: usize,
    landmark_capacity: usize,
    metadata: [&str; 3],
) -> usize {
    he_capacity
        .saturating_mul(size_of::<HeCell>())
        .saturating_add(
            he_cells
                .iter()
                .map(|cell| {
                    cell.cell_id
                        .capacity()
                        .saturating_add(cell.cell_type.as_ref().map_or(0, String::capacity))
                })
                .sum::<usize>(),
        )
        .saturating_add(ihc_capacity.saturating_mul(size_of::<IhcCell>()))
        .saturating_add(
            ihc_cells
                .iter()
                .map(|cell| cell.cell_id.capacity())
                .sum::<usize>(),
        )
        .saturating_add(landmark_capacity.saturating_mul(size_of::<LandmarkPair>()))
        .saturating_add(metadata.into_iter().map(str::len).sum::<usize>())
}

pub(super) fn projected_fused_storage_bytes(he_cells: &[HeCell], ihc_cells: &[IhcCell]) -> usize {
    he_cells
        .len()
        .saturating_add(ihc_cells.len())
        .saturating_mul(size_of::<FusedCell>())
        .saturating_add(
            he_cells
                .iter()
                .map(|cell| {
                    cell.cell_id
                        .capacity()
                        .saturating_add(cell.cell_type.as_ref().map_or(0, String::capacity))
                })
                .sum::<usize>(),
        )
        .saturating_add(
            ihc_cells
                .iter()
                .map(|cell| cell.cell_id.capacity())
                .sum::<usize>(),
        )
}

pub(super) fn analysis_metadata_storage_bytes(metadata: [&str; 3]) -> usize {
    metadata.into_iter().map(str::len).sum()
}

fn timing_storage_bytes(null_stage_count: usize) -> usize {
    15usize
        .saturating_add(null_stage_count)
        .saturating_mul(size_of::<TimingStage>())
        // Every current static timing name is shorter than 64 bytes. Reserve
        // the full bound so adding a longer stage cannot silently undercount.
        .saturating_add(15usize.saturating_add(null_stage_count).saturating_mul(64))
}

fn configured_label_pair_storage_bytes(label_pairs: &[[String; 2]]) -> usize {
    label_pairs
        .len()
        .saturating_mul(size_of::<LabelPair>())
        .saturating_add(
            label_pairs
                .iter()
                .map(|pair| pair[0].capacity().saturating_add(pair[1].capacity()))
                .sum::<usize>(),
        )
}

pub(super) fn registration_artifact_storage_bytes(
    landmarks: &[LandmarkPair],
    fused: &[FusedCell],
) -> usize {
    landmarks
        .len()
        .saturating_mul(size_of::<RegistrationResidual>())
        .saturating_add(
            fused
                .len()
                .saturating_mul(size_of::<CellExtrapolationRecord>()),
        )
        .saturating_add(
            fused
                .iter()
                .map(|cell| cell.source_cell_id.capacity())
                .sum::<usize>(),
        )
}

pub(super) fn registration_artifact_scratch_bytes(landmark_count: usize) -> usize {
    landmark_count
        .saturating_mul(size_of::<[f64; 2]>())
        .saturating_mul(4)
}

pub(super) fn enrichment_retained_storage_bytes(
    label_pairs: &[LabelPair],
    result_set_count: usize,
) -> usize {
    let one_set = label_pairs
        .len()
        .saturating_mul(size_of::<NeighborhoodEnrichmentResult>())
        .saturating_add(
            label_pairs
                .iter()
                .map(|pair| pair.label_a.len().saturating_add(pair.label_b.len()))
                .sum::<usize>(),
        );
    result_set_count.saturating_mul(one_set).saturating_add(
        result_set_count
            .saturating_sub(1)
            .saturating_mul(size_of::<(
                NeighborhoodNullModel,
                Vec<NeighborhoodEnrichmentResult>,
            )>()),
    )
}

pub(super) fn enrichment_scratch_bytes(cell_count: usize, permutation_count: usize) -> usize {
    // Covers explicit strata, grouping maps/indices, shuffled compact labels,
    // degree/QC vectors, and one scalar null-count vector. The per-cell term
    // deliberately exceeds the concrete representations to cover BTreeMap
    // nodes without depending on standard-library internals.
    cell_count
        .saturating_mul(96)
        .saturating_add(permutation_count.saturating_mul(size_of::<usize>()))
}

pub(super) fn cross_curve_retained_storage_bytes(curves: &[CrossInteractionCurve]) -> usize {
    curves
        .len()
        .saturating_mul(size_of::<CrossInteractionCurve>())
        .saturating_add(
            curves
                .iter()
                .map(|curve| {
                    curve
                        .label_a
                        .capacity()
                        .saturating_add(curve.label_b.capacity())
                        .saturating_add(
                            curve
                                .points
                                .capacity()
                                .saturating_mul(size_of::<crate::output::CrossInteractionPoint>()),
                        )
                })
                .sum::<usize>(),
        )
}

pub(super) fn territory_retained_storage_bytes(territories: &[NeighborhoodTerritory]) -> usize {
    territories
        .len()
        .saturating_mul(size_of::<NeighborhoodTerritory>())
}

pub(super) fn profile_and_comparison_storage_upper_bound(
    territory_count: usize,
    label_count: usize,
    total_label_name_bytes: usize,
) -> usize {
    let fraction_bytes = territory_count
        .saturating_mul(label_count)
        .saturating_mul(size_of::<LabelFraction>())
        .saturating_add(territory_count.saturating_mul(total_label_name_bytes));
    let profile_bytes = territory_count
        .saturating_mul(size_of::<TerritoryProfile>())
        .saturating_add(fraction_bytes);
    let comparison_count = territory_count.saturating_mul(territory_count.saturating_sub(1)) / 2;
    let comparison_bytes =
        comparison_count.saturating_mul(size_of::<CurveComparisonResult>().saturating_add(384));
    profile_bytes
        .saturating_add(comparison_bytes)
        .saturating_add(128)
}

pub(super) fn profile_and_comparison_scratch_bytes(
    label_count: usize,
    total_label_name_bytes: usize,
) -> usize {
    label_count
        .saturating_mul(128)
        .saturating_add(total_label_name_bytes.saturating_mul(3))
}

pub(super) fn graph_smoothing_storage_bytes(
    node_count: usize,
    edge_count: usize,
    label_count: usize,
    label_pairs: &[LabelPair],
) -> (usize, usize) {
    let retained = size_of::<GraphSmoothingSummary>()
        .saturating_add(
            label_pairs
                .len()
                .saturating_mul(size_of::<GraphSmoothingLabelPairSummary>()),
        )
        .saturating_add(
            label_pairs
                .iter()
                .map(|pair| pair.label_a.len().saturating_add(pair.label_b.len()))
                .sum::<usize>(),
        )
        .saturating_add(256);
    let embeddings = node_count
        .saturating_mul(label_count)
        .saturating_mul(size_of::<f64>())
        .saturating_mul(2);
    let adjacency = node_count
        .saturating_mul(size_of::<Vec<usize>>())
        .saturating_add(
            edge_count
                .saturating_mul(4)
                .saturating_mul(size_of::<usize>()),
        );
    (
        retained,
        retained
            .saturating_add(embeddings)
            .saturating_add(adjacency),
    )
}
