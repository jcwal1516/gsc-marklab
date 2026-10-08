use marklab_data::MeasurementStatus;
use marklab_project::{ArtifactId, ContentDigest};
use thiserror::Error;

use super::{
    matrix_artifact::slide_lineage_digest, DerivedSlideEmbeddingTableCandidate,
    EmbeddingEntityKind, MultiscaleEmbeddingProvenance, MultiscaleEmbeddingProvenanceVariant,
    VerifiedDerivedSlideEmbeddingArtifactGraph, VerifiedRegionEmbeddingTableArtifact,
};
use crate::EmbeddingStatus;

/// Availability of a lineage-exact slide aggregation-path discrepancy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlideEmbeddingAggregationPathDiscrepancyStatus {
    /// Both finalized slide paths contain a present vector.
    Available,
    /// At least one finalized slide path has no present vector.
    InsufficientComparablePaths,
}

/// Mean squared component discrepancy between exact patch- and region-sourced slide paths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlideEmbeddingAggregationPathDiscrepancy {
    status: SlideEmbeddingAggregationPathDiscrepancyStatus,
    patch_path_status: EmbeddingStatus,
    region_path_status: EmbeddingStatus,
    patch_measurement_status: MeasurementStatus,
    region_measurement_status: MeasurementStatus,
    dimension: u32,
    mean_squared_component_difference: Option<f64>,
    expected_slides_artifact_id: ArtifactId,
    expected_slides_logical_digest: ContentDigest,
    source_patch_table_artifact_id: ArtifactId,
    source_patch_table_logical_digest: ContentDigest,
    region_table_artifact_id: ArtifactId,
    region_table_logical_digest: ContentDigest,
    patch_region_link_artifact_id: ArtifactId,
    patch_region_link_logical_digest: ContentDigest,
    patch_slide_support_artifact_id: ArtifactId,
    patch_slide_support_logical_digest: ContentDigest,
    region_slide_support_artifact_id: ArtifactId,
    region_slide_support_logical_digest: ContentDigest,
    patch_slide_candidate_logical_digest: ContentDigest,
    region_slide_candidate_logical_digest: ContentDigest,
    patch_slide_provenance_artifact_id: ArtifactId,
    patch_slide_provenance_logical_digest: ContentDigest,
    region_slide_provenance_artifact_id: ArtifactId,
    region_slide_provenance_logical_digest: ContentDigest,
}

impl SlideEmbeddingAggregationPathDiscrepancy {
    /// Availability of the numeric discrepancy.
    pub fn status(self) -> SlideEmbeddingAggregationPathDiscrepancyStatus {
        self.status
    }

    /// Exact extraction status of the patch-sourced singleton row.
    pub fn patch_path_status(self) -> EmbeddingStatus {
        self.patch_path_status
    }

    /// Exact extraction status of the region-sourced singleton row.
    pub fn region_path_status(self) -> EmbeddingStatus {
        self.region_path_status
    }

    /// Measurement origin of the patch-sourced slide vector.
    pub fn patch_measurement_status(self) -> MeasurementStatus {
        self.patch_measurement_status
    }

    /// Measurement origin of the region-sourced slide vector.
    pub fn region_measurement_status(self) -> MeasurementStatus {
        self.region_measurement_status
    }

    /// Common positive component count.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Mean squared component difference, when both paths are present.
    pub fn mean_squared_component_difference(self) -> Option<f64> {
        self.mean_squared_component_difference
    }

    /// Exact expected-slide-set artifact identity shared by both paths.
    pub fn expected_slides_artifact_id(self) -> ArtifactId {
        self.expected_slides_artifact_id
    }

    /// Format-independent expected-slide-set identity shared by both paths.
    pub fn expected_slides_logical_digest(self) -> ContentDigest {
        self.expected_slides_logical_digest
    }

    /// Exact source patch-table artifact identity shared by both lineages.
    pub fn source_patch_table_artifact_id(self) -> ArtifactId {
        self.source_patch_table_artifact_id
    }

    /// Format-independent source patch-table identity shared by both lineages.
    pub fn source_patch_table_logical_digest(self) -> ContentDigest {
        self.source_patch_table_logical_digest
    }

    /// Exact verified intermediate region-table artifact identity.
    pub fn region_table_artifact_id(self) -> ArtifactId {
        self.region_table_artifact_id
    }

    /// Format-independent intermediate region-table identity.
    pub fn region_table_logical_digest(self) -> ContentDigest {
        self.region_table_logical_digest
    }

    /// Exact patch-region-link artifact identity retained by the region receipt.
    pub fn patch_region_link_artifact_id(self) -> ArtifactId {
        self.patch_region_link_artifact_id
    }

    /// Format-independent patch-region-link identity retained by the region receipt.
    pub fn patch_region_link_logical_digest(self) -> ContentDigest {
        self.patch_region_link_logical_digest
    }

    /// Exact patch-sourced slide-support artifact identity.
    pub fn patch_slide_support_artifact_id(self) -> ArtifactId {
        self.patch_slide_support_artifact_id
    }

    /// Format-independent patch-sourced slide-support identity.
    pub fn patch_slide_support_logical_digest(self) -> ContentDigest {
        self.patch_slide_support_logical_digest
    }

    /// Exact region-sourced slide-support artifact identity.
    pub fn region_slide_support_artifact_id(self) -> ArtifactId {
        self.region_slide_support_artifact_id
    }

    /// Format-independent region-sourced slide-support identity.
    pub fn region_slide_support_logical_digest(self) -> ContentDigest {
        self.region_slide_support_logical_digest
    }

    /// Format-independent patch-sourced slide-candidate identity.
    pub fn patch_slide_candidate_logical_digest(self) -> ContentDigest {
        self.patch_slide_candidate_logical_digest
    }

    /// Format-independent region-sourced slide-candidate identity.
    pub fn region_slide_candidate_logical_digest(self) -> ContentDigest {
        self.region_slide_candidate_logical_digest
    }

    /// Exact patch-sourced slide-provenance artifact identity.
    pub fn patch_slide_provenance_artifact_id(self) -> ArtifactId {
        self.patch_slide_provenance_artifact_id
    }

    /// Format-independent patch-sourced slide-provenance identity.
    pub fn patch_slide_provenance_logical_digest(self) -> ContentDigest {
        self.patch_slide_provenance_logical_digest
    }

    /// Exact region-sourced slide-provenance artifact identity.
    pub fn region_slide_provenance_artifact_id(self) -> ArtifactId {
        self.region_slide_provenance_artifact_id
    }

    /// Format-independent region-sourced slide-provenance identity.
    pub fn region_slide_provenance_logical_digest(self) -> ContentDigest {
        self.region_slide_provenance_logical_digest
    }
}

/// Invalid path variant, exact binding, common lineage, or work request.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SlideEmbeddingAggregationPathDiscrepancyError {
    /// Patch-path provenance is not deterministic slide derivation from patches.
    #[error(
        "slide path discrepancy requires patch-sourced slide provenance, observed {observed:?}"
    )]
    UnsupportedPatchProvenanceVariant {
        /// Supplied closed provenance variant.
        observed: MultiscaleEmbeddingProvenanceVariant,
    },
    /// Region-path provenance is not deterministic slide derivation from regions.
    #[error(
        "slide path discrepancy requires region-sourced slide provenance, observed {observed:?}"
    )]
    UnsupportedRegionProvenanceVariant {
        /// Supplied closed provenance variant.
        observed: MultiscaleEmbeddingProvenanceVariant,
    },
    /// Patch-sourced candidate, graph, provenance, or singleton binding disagrees.
    #[error("slide path discrepancy patch-path bindings disagree")]
    PatchPathBindingMismatch,
    /// Region-sourced candidate, graph, provenance, receipt, or singleton binding disagrees.
    #[error("slide path discrepancy region-path bindings disagree")]
    RegionPathBindingMismatch,
    /// The two valid paths do not share one exact slide, expected set, dimension, and derivation.
    #[error("slide path discrepancy common context disagrees")]
    CommonContextMismatch,
    /// The region-table receipt does not bridge both paths to one source patch table.
    #[error("slide path discrepancy source lineage disagrees")]
    SourceLineageMismatch,
    /// The exact component count exceeds the caller's explicit work limit.
    #[error(
        "slide path discrepancy component operations {required} exceed caller maximum {maximum}"
    )]
    ComponentOperationBudgetExceeded {
        /// Exact common component count.
        required: u64,
        /// Caller-provided maximum component operations.
        maximum: u64,
    },
}

/// Compare both completed deterministic slide aggregation paths over one exact patch lineage.
///
/// All candidate, graph, provenance, common-context, and source-lineage bindings are checked before
/// the exact dimension work cap. Present vectors are then compared sequentially in stored component
/// order using `f64`, with one final division. Runtime is `O(D)` with `O(1)` additional retained
/// state and no allocation. The result is descriptive aggregation-path discrepancy only; it proves
/// no agreement, accuracy, embedding quality, geometry, spatial dependence, inference, path
/// preference, real-source result, or biological meaning.
///
/// # Errors
///
/// Returns a typed category when a closed path variant, exact binding, common lineage, or caller
/// work limit disagrees.
#[allow(clippy::too_many_arguments)]
pub fn slide_embedding_aggregation_path_discrepancy(
    patch_candidate: &DerivedSlideEmbeddingTableCandidate,
    patch_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    patch_provenance: &MultiscaleEmbeddingProvenance,
    region_candidate: &DerivedSlideEmbeddingTableCandidate,
    region_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    region_provenance: &MultiscaleEmbeddingProvenance,
    region_receipt: VerifiedRegionEmbeddingTableArtifact,
    maximum_component_operations: u64,
) -> Result<SlideEmbeddingAggregationPathDiscrepancy, SlideEmbeddingAggregationPathDiscrepancyError>
{
    validate_variants(patch_provenance, region_provenance)?;
    validate_path(
        patch_candidate,
        patch_graph,
        patch_provenance,
        EmbeddingEntityKind::Patch,
    )
    .then_some(())
    .ok_or(SlideEmbeddingAggregationPathDiscrepancyError::PatchPathBindingMismatch)?;
    validate_path(
        region_candidate,
        region_graph,
        region_provenance,
        EmbeddingEntityKind::Region,
    )
    .then_some(())
    .ok_or(SlideEmbeddingAggregationPathDiscrepancyError::RegionPathBindingMismatch)?;
    validate_common_context(patch_candidate, patch_graph, region_candidate, region_graph)?;
    validate_lineage(patch_graph, region_graph, region_receipt)?;

    let patch_row = patch_candidate
        .table()
        .row(0)
        .map_err(|_| SlideEmbeddingAggregationPathDiscrepancyError::PatchPathBindingMismatch)?;
    let region_row = region_candidate
        .table()
        .row(0)
        .map_err(|_| SlideEmbeddingAggregationPathDiscrepancyError::RegionPathBindingMismatch)?;
    if patch_row.slide_id() != patch_candidate.table().owning_slide_id()
        || region_row.slide_id() != region_candidate.table().owning_slide_id()
    {
        return Err(SlideEmbeddingAggregationPathDiscrepancyError::CommonContextMismatch);
    }

    let required = u64::from(patch_candidate.dimension());
    if required > maximum_component_operations {
        return Err(
            SlideEmbeddingAggregationPathDiscrepancyError::ComponentOperationBudgetExceeded {
                required,
                maximum: maximum_component_operations,
            },
        );
    }

    let patch_path_status = patch_row.status();
    let region_path_status = region_row.status();
    let (status, mean_squared_component_difference) =
        match (patch_row.vector(), region_row.vector()) {
            (Some(patch), Some(region)) => {
                let mut squared_difference = 0.0_f64;
                for (&patch, &region) in patch.iter().zip(region) {
                    let difference = f64::from(patch) - f64::from(region);
                    squared_difference += difference * difference;
                }
                let mean = squared_difference / f64::from(patch_candidate.dimension());
                (
                    SlideEmbeddingAggregationPathDiscrepancyStatus::Available,
                    Some(if mean == 0.0 { 0.0 } else { mean }),
                )
            }
            _ => (
                SlideEmbeddingAggregationPathDiscrepancyStatus::InsufficientComparablePaths,
                None,
            ),
        };

    Ok(SlideEmbeddingAggregationPathDiscrepancy {
        status,
        patch_path_status,
        region_path_status,
        patch_measurement_status: patch_provenance.measurement_status(),
        region_measurement_status: region_provenance.measurement_status(),
        dimension: patch_candidate.dimension(),
        mean_squared_component_difference,
        expected_slides_artifact_id: patch_graph.expected_slides_artifact_id,
        expected_slides_logical_digest: patch_graph.expected_slides_logical_digest,
        source_patch_table_artifact_id: region_receipt.source_patch_table_artifact_id(),
        source_patch_table_logical_digest: region_receipt.source_patch_table_logical_digest(),
        region_table_artifact_id: region_receipt.artifact_id(),
        region_table_logical_digest: region_receipt.logical_digest(),
        patch_region_link_artifact_id: region_receipt.patch_region_link_artifact_id(),
        patch_region_link_logical_digest: region_receipt.patch_region_link_logical_digest(),
        patch_slide_support_artifact_id: patch_graph.slide_support_artifact_id,
        patch_slide_support_logical_digest: patch_graph.slide_support_logical_digest,
        region_slide_support_artifact_id: region_graph.slide_support_artifact_id,
        region_slide_support_logical_digest: region_graph.slide_support_logical_digest,
        patch_slide_candidate_logical_digest: patch_candidate.logical_digest(),
        region_slide_candidate_logical_digest: region_candidate.logical_digest(),
        patch_slide_provenance_artifact_id: patch_graph.provenance_artifact_id,
        patch_slide_provenance_logical_digest: patch_provenance.logical_digest(),
        region_slide_provenance_artifact_id: region_graph.provenance_artifact_id,
        region_slide_provenance_logical_digest: region_provenance.logical_digest(),
    })
}

fn validate_variants(
    patch: &MultiscaleEmbeddingProvenance,
    region: &MultiscaleEmbeddingProvenance,
) -> Result<(), SlideEmbeddingAggregationPathDiscrepancyError> {
    let patch_variant = patch.variant();
    if patch_variant != MultiscaleEmbeddingProvenanceVariant::DerivedSlideFromPatches {
        return Err(
            SlideEmbeddingAggregationPathDiscrepancyError::UnsupportedPatchProvenanceVariant {
                observed: patch_variant,
            },
        );
    }
    let region_variant = region.variant();
    if region_variant != MultiscaleEmbeddingProvenanceVariant::DerivedSlideFromRegions {
        return Err(
            SlideEmbeddingAggregationPathDiscrepancyError::UnsupportedRegionProvenanceVariant {
                observed: region_variant,
            },
        );
    }
    Ok(())
}

fn validate_path(
    candidate: &DerivedSlideEmbeddingTableCandidate,
    graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    provenance: &MultiscaleEmbeddingProvenance,
    source_kind: EmbeddingEntityKind,
) -> bool {
    let table = candidate.table();
    #[cfg(feature = "parquet")]
    if candidate.graph != graph {
        return false;
    }
    table.row_count() == 1
        && table.owning_slide_id() == provenance.owning_slide_id()
        && slide_lineage_digest(table.owning_slide_id()) == graph.owning_slide_binding_digest
        && graph.source_entity_kind == source_kind
        && table.expected_entities_artifact_id() == graph.expected_slides_artifact_id
        && table.expected_entities_logical_digest() == graph.expected_slides_logical_digest
        && table.support_artifact_id() == graph.slide_support_artifact_id
        && table.support_logical_digest() == graph.slide_support_logical_digest
        && table.provenance_artifact_id() == graph.provenance_artifact_id
        && table.provenance_logical_digest() == graph.provenance_logical_digest
        && table.provenance_logical_digest() == provenance.logical_digest()
        && table.dimension() == graph.output_dimension
        && table.dimension() == provenance.output_dimension()
        && provenance.pooling_or_aggregation() == "arithmetic_mean"
        && provenance.measurement_status() == MeasurementStatus::DerivedSummary
}

fn validate_common_context(
    patch_candidate: &DerivedSlideEmbeddingTableCandidate,
    patch_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    region_candidate: &DerivedSlideEmbeddingTableCandidate,
    region_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
) -> Result<(), SlideEmbeddingAggregationPathDiscrepancyError> {
    let patch = patch_candidate.table();
    let region = region_candidate.table();
    if patch.owning_slide_id() != region.owning_slide_id()
        || patch.expected_entities_artifact_id() != region.expected_entities_artifact_id()
        || patch.expected_entities_logical_digest() != region.expected_entities_logical_digest()
        || patch.dimension() == 0
        || patch.dimension() != region.dimension()
        || patch_graph.owning_slide_binding_digest != region_graph.owning_slide_binding_digest
        || patch_graph.derivation_artifact_id != region_graph.derivation_artifact_id
        || patch_graph.derivation_logical_digest != region_graph.derivation_logical_digest
    {
        return Err(SlideEmbeddingAggregationPathDiscrepancyError::CommonContextMismatch);
    }
    Ok(())
}

fn validate_lineage(
    patch_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    region_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    region_receipt: VerifiedRegionEmbeddingTableArtifact,
) -> Result<(), SlideEmbeddingAggregationPathDiscrepancyError> {
    if patch_graph.source_table_artifact_id != region_receipt.source_patch_table_artifact_id()
        || patch_graph.source_table_logical_digest
            != region_receipt.source_patch_table_logical_digest()
    {
        return Err(SlideEmbeddingAggregationPathDiscrepancyError::SourceLineageMismatch);
    }
    if region_graph.source_table_artifact_id != region_receipt.artifact_id()
        || region_graph.source_table_logical_digest != region_receipt.logical_digest()
        || region_graph.source_support_artifact_id != region_receipt.region_support_artifact_id()
        || region_graph.source_support_logical_digest
            != region_receipt.region_support_logical_digest()
        || region_graph.source_table_row_count != region_receipt.row_count()
        || region_graph.output_dimension != region_receipt.dimension()
    {
        return Err(SlideEmbeddingAggregationPathDiscrepancyError::RegionPathBindingMismatch);
    }
    Ok(())
}
