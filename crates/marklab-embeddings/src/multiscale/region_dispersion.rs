use marklab_data::MeasurementStatus;
use marklab_project::{ArtifactId, ContentDigest};
use thiserror::Error;

use super::{
    DerivedRegionEmbeddingTableCandidate, MultiscaleEmbeddingProvenance,
    MultiscaleEmbeddingProvenanceVariant, PatchEmbeddingTable, PatchRegionDeclaration,
    PatchRegionLink, VerifiedDerivedRegionEmbeddingArtifactGraph,
};

/// Availability of the descriptive patch-to-derived-region dispersion value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchRegionEmbeddingDispersionStatus {
    /// At least one declared relation had present source and derived vectors.
    Available,
    /// No declared relation had positive eligible weight.
    InsufficientContributors,
}

/// Descriptive declared-fraction-weighted patch dispersion around derived region means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchRegionEmbeddingDispersion {
    status: PatchRegionEmbeddingDispersionStatus,
    source_measurement_status: MeasurementStatus,
    region_measurement_status: MeasurementStatus,
    total_relation_count: u64,
    eligible_relation_count: u64,
    excluded_relation_count: u64,
    dimension: u32,
    mean_squared_euclidean_distance: Option<f64>,
    source_table_artifact_id: ArtifactId,
    source_table_logical_digest: ContentDigest,
    source_provenance_artifact_id: ArtifactId,
    source_provenance_logical_digest: ContentDigest,
    link_artifact_id: ArtifactId,
    link_logical_digest: ContentDigest,
    region_table_logical_digest: ContentDigest,
    region_provenance_artifact_id: ArtifactId,
    region_provenance_logical_digest: ContentDigest,
}

impl PatchRegionEmbeddingDispersion {
    /// Availability of the numeric value.
    pub fn status(self) -> PatchRegionEmbeddingDispersionStatus {
        self.status
    }

    /// Measurement origin of the source patch vectors.
    pub fn source_measurement_status(self) -> MeasurementStatus {
        self.source_measurement_status
    }

    /// Measurement origin of the deterministically derived region vectors.
    pub fn region_measurement_status(self) -> MeasurementStatus {
        self.region_measurement_status
    }

    /// Total canonical nonzero patch-region relations.
    pub fn total_relation_count(self) -> u64 {
        self.total_relation_count
    }

    /// Relations whose source patch and finalized region vectors were present.
    pub fn eligible_relation_count(self) -> u64 {
        self.eligible_relation_count
    }

    /// Relations excluded because a required vector was non-present.
    pub fn excluded_relation_count(self) -> u64 {
        self.excluded_relation_count
    }

    /// Fixed source and derived embedding dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Fraction-weighted mean squared Euclidean distance, when available.
    pub fn mean_squared_euclidean_distance(self) -> Option<f64> {
        self.mean_squared_euclidean_distance
    }

    /// Exact verified source patch-table artifact identity.
    pub fn source_table_artifact_id(self) -> ArtifactId {
        self.source_table_artifact_id
    }

    /// Format-independent source patch-table identity.
    pub fn source_table_logical_digest(self) -> ContentDigest {
        self.source_table_logical_digest
    }

    /// Exact direct-patch provenance artifact identity bound by the source table.
    pub fn source_provenance_artifact_id(self) -> ArtifactId {
        self.source_provenance_artifact_id
    }

    /// Format-independent direct-patch provenance identity.
    pub fn source_provenance_logical_digest(self) -> ContentDigest {
        self.source_provenance_logical_digest
    }

    /// Exact verified patch-region link artifact identity.
    pub fn link_artifact_id(self) -> ArtifactId {
        self.link_artifact_id
    }

    /// Format-independent patch-region link identity.
    pub fn link_logical_digest(self) -> ContentDigest {
        self.link_logical_digest
    }

    /// Format-independent deterministic region-candidate table identity.
    pub fn region_table_logical_digest(self) -> ContentDigest {
        self.region_table_logical_digest
    }

    /// Exact derived-region provenance artifact identity bound by the candidate.
    pub fn region_provenance_artifact_id(self) -> ArtifactId {
        self.region_provenance_artifact_id
    }

    /// Format-independent derived-region provenance identity.
    pub fn region_provenance_logical_digest(self) -> ContentDigest {
        self.region_provenance_logical_digest
    }
}

/// Invalid provenance variant, exact binding, or bounded work request.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PatchRegionEmbeddingDispersionError {
    /// Source provenance is not direct patch extraction.
    #[error(
        "patch-region dispersion requires direct-patch source provenance, observed {observed:?}"
    )]
    UnsupportedSourceProvenanceVariant {
        /// Supplied closed source provenance variant.
        observed: MultiscaleEmbeddingProvenanceVariant,
    },
    /// Output provenance is not deterministic region derivation.
    #[error(
        "patch-region dispersion requires derived-region output provenance, observed {observed:?}"
    )]
    UnsupportedRegionProvenanceVariant {
        /// Supplied closed output provenance variant.
        observed: MultiscaleEmbeddingProvenanceVariant,
    },
    /// Source table, direct provenance, and verified graph disagree.
    #[error("patch-region dispersion source bindings disagree")]
    SourceBindingMismatch,
    /// Patch-region link and verified graph disagree.
    #[error("patch-region dispersion link bindings disagree")]
    LinkBindingMismatch,
    /// Derived candidate, provenance, and verified graph disagree.
    #[error("patch-region dispersion region candidate bindings disagree")]
    RegionCandidateBindingMismatch,
    /// The checked upper bound exceeds the caller's explicit work limit.
    #[error("patch-region component operations {required} exceed caller maximum {maximum}")]
    ComponentOperationBudgetExceeded {
        /// Checked `nonzero_relation_count * dimension` upper bound.
        required: u64,
        /// Caller-provided maximum component operations.
        maximum: u64,
    },
    /// A count or work-bound calculation overflowed.
    #[error("patch-region dispersion count calculation overflowed")]
    SizeOverflow,
}

/// Compute one bounded descriptive dispersion around finalized declared region means.
///
/// Every exact input binding is checked before the operation budget. Eligible relations and
/// components are then accumulated sequentially in canonical link order using `f64`. Runtime is
/// `O(R(log P + log G) + R*D)` with fixed-size additional retained state. This operation proves no
/// region geometry, tissue window, spatial dependence, inference, or biological interpretation.
///
/// # Errors
///
/// Returns a typed error when a provenance variant or exact artifact binding disagrees, a count
/// overflows, or the conservative component-operation bound exceeds the caller's maximum.
#[allow(clippy::too_many_arguments)]
pub fn patch_region_embedding_dispersion(
    source_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
    region_candidate: &DerivedRegionEmbeddingTableCandidate,
    graph: VerifiedDerivedRegionEmbeddingArtifactGraph,
    source_provenance: &MultiscaleEmbeddingProvenance,
    region_provenance: &MultiscaleEmbeddingProvenance,
    maximum_component_operations: u64,
) -> Result<PatchRegionEmbeddingDispersion, PatchRegionEmbeddingDispersionError> {
    validate_bindings(
        source_table,
        link,
        region_candidate,
        graph,
        source_provenance,
        region_provenance,
    )?;

    let total_relation_count = u64::try_from(link.nonzero_relation_count())
        .map_err(|_| PatchRegionEmbeddingDispersionError::SizeOverflow)?;
    let required_component_operations = total_relation_count
        .checked_mul(u64::from(source_table.dimension()))
        .ok_or(PatchRegionEmbeddingDispersionError::SizeOverflow)?;
    if required_component_operations > maximum_component_operations {
        return Err(
            PatchRegionEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: required_component_operations,
                maximum: maximum_component_operations,
            },
        );
    }

    let (eligible_relation_count, total_weight, total_weighted_distance) =
        accumulate_present_relations(source_table, link, region_candidate)?;
    let (status, mean_squared_euclidean_distance) = if eligible_relation_count == 0 {
        (
            PatchRegionEmbeddingDispersionStatus::InsufficientContributors,
            None,
        )
    } else {
        let mean = total_weighted_distance / total_weight;
        (
            PatchRegionEmbeddingDispersionStatus::Available,
            Some(if mean == 0.0 { 0.0 } else { mean }),
        )
    };

    Ok(PatchRegionEmbeddingDispersion {
        status,
        source_measurement_status: source_provenance.measurement_status(),
        region_measurement_status: region_provenance.measurement_status(),
        total_relation_count,
        eligible_relation_count,
        excluded_relation_count: total_relation_count - eligible_relation_count,
        dimension: source_table.dimension(),
        mean_squared_euclidean_distance,
        source_table_artifact_id: graph.source_patch_table_artifact_id,
        source_table_logical_digest: source_table.logical_digest(),
        source_provenance_artifact_id: source_table.provenance_artifact_id(),
        source_provenance_logical_digest: source_provenance.logical_digest(),
        link_artifact_id: graph.patch_region_link_artifact_id,
        link_logical_digest: link.logical_digest(),
        region_table_logical_digest: region_candidate.logical_digest(),
        region_provenance_artifact_id: graph.provenance_artifact_id,
        region_provenance_logical_digest: region_provenance.logical_digest(),
    })
}

fn validate_bindings(
    source_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
    region_candidate: &DerivedRegionEmbeddingTableCandidate,
    graph: VerifiedDerivedRegionEmbeddingArtifactGraph,
    source_provenance: &MultiscaleEmbeddingProvenance,
    region_provenance: &MultiscaleEmbeddingProvenance,
) -> Result<(), PatchRegionEmbeddingDispersionError> {
    let source_variant = source_provenance.variant();
    if source_variant != MultiscaleEmbeddingProvenanceVariant::DirectPatch {
        return Err(
            PatchRegionEmbeddingDispersionError::UnsupportedSourceProvenanceVariant {
                observed: source_variant,
            },
        );
    }
    let region_variant = region_provenance.variant();
    if region_variant != MultiscaleEmbeddingProvenanceVariant::DerivedRegion {
        return Err(
            PatchRegionEmbeddingDispersionError::UnsupportedRegionProvenanceVariant {
                observed: region_variant,
            },
        );
    }

    let source_row_count = u64::try_from(source_table.row_count())
        .map_err(|_| PatchRegionEmbeddingDispersionError::SizeOverflow)?;
    if source_table.owning_slide_id() != source_provenance.owning_slide_id()
        || source_table.dimension() != source_provenance.output_dimension()
        || source_table.provenance_logical_digest() != source_provenance.logical_digest()
        || source_table.logical_digest() != graph.source_patch_table_logical_digest
        || source_row_count != graph.source_patch_table_row_count
        || source_table.dimension() != graph.output_dimension
        || source_table.expected_entities_artifact_id() != graph.source_expected_patches_artifact_id
        || source_table.support_artifact_id() != graph.source_patch_support_artifact_id
        || source_table.provenance_logical_digest() != graph.source_patch_provenance_logical_digest
    {
        return Err(PatchRegionEmbeddingDispersionError::SourceBindingMismatch);
    }

    if link.owning_slide_id() != source_table.owning_slide_id()
        || link.expected_patches_artifact_id() != source_table.expected_entities_artifact_id()
        || link.expected_patches_logical_digest() != source_table.expected_entities_logical_digest()
        || link.logical_digest() != graph.patch_region_link_logical_digest
        || link.expected_regions_artifact_id() != graph.expected_regions_artifact_id
        || link.expected_regions_logical_digest() != graph.expected_regions_logical_digest
    {
        return Err(PatchRegionEmbeddingDispersionError::LinkBindingMismatch);
    }

    let region_table = region_candidate.table();
    #[cfg(feature = "parquet")]
    if region_candidate.graph != graph {
        return Err(PatchRegionEmbeddingDispersionError::RegionCandidateBindingMismatch);
    }
    if region_table.owning_slide_id() != source_table.owning_slide_id()
        || region_table.owning_slide_id() != region_provenance.owning_slide_id()
        || region_table.dimension() != source_table.dimension()
        || region_table.dimension() != region_provenance.output_dimension()
        || region_table.expected_entities_artifact_id() != graph.expected_regions_artifact_id
        || region_table.expected_entities_logical_digest() != graph.expected_regions_logical_digest
        || region_table.support_artifact_id() != graph.region_support_artifact_id
        || region_table.support_logical_digest() != graph.region_support_logical_digest
        || region_table.provenance_artifact_id() != graph.provenance_artifact_id
        || region_table.provenance_logical_digest() != graph.provenance_logical_digest
        || region_table.provenance_logical_digest() != region_provenance.logical_digest()
        || region_provenance.pooling_or_aggregation() != "weighted_mean"
    {
        return Err(PatchRegionEmbeddingDispersionError::RegionCandidateBindingMismatch);
    }
    Ok(())
}

fn accumulate_present_relations(
    source_table: &PatchEmbeddingTable,
    link: &PatchRegionLink,
    region_candidate: &DerivedRegionEmbeddingTableCandidate,
) -> Result<(u64, f64, f64), PatchRegionEmbeddingDispersionError> {
    let region_table = region_candidate.table();
    let mut eligible_relation_count = 0_u64;
    let mut total_weight = 0.0_f64;
    let mut total_weighted_distance = 0.0_f64;
    for relation in link.nonzero_relations() {
        let source_index = source_table
            .row_index(relation.patch_id())
            .ok_or(PatchRegionEmbeddingDispersionError::SourceBindingMismatch)?;
        let region_index = region_table
            .row_index(relation.region_id())
            .ok_or(PatchRegionEmbeddingDispersionError::RegionCandidateBindingMismatch)?;
        let source = source_table
            .row(source_index)
            .map_err(|_| PatchRegionEmbeddingDispersionError::SourceBindingMismatch)?;
        let region = region_table
            .row(region_index)
            .map_err(|_| PatchRegionEmbeddingDispersionError::RegionCandidateBindingMismatch)?;
        let (Some(source), Some(region)) = (source.vector(), region.vector()) else {
            continue;
        };

        let mut distance = 0.0_f64;
        for (&source, &region) in source.iter().zip(region) {
            let difference = f64::from(source) - f64::from(region);
            distance += difference * difference;
        }
        let weight = declared_weight(relation);
        total_weighted_distance += weight * distance;
        total_weight += weight;
        eligible_relation_count = eligible_relation_count
            .checked_add(1)
            .ok_or(PatchRegionEmbeddingDispersionError::SizeOverflow)?;
    }
    Ok((
        eligible_relation_count,
        total_weight,
        total_weighted_distance,
    ))
}

fn declared_weight(relation: &PatchRegionDeclaration) -> f64 {
    (relation.numerator() as f64) / (relation.denominator() as f64)
}
