use marklab_data::{MeasurementStatus, PatchId};
use marklab_project::{ArtifactId, ContentDigest};
use thiserror::Error;

use super::{
    overlap::PatchOverlapGraph,
    records::{
        MultiscaleEmbeddingProvenance, MultiscaleEmbeddingProvenanceVariant,
        MultiscaleEmbeddingSupport, MultiscaleEmbeddingSupportVariant,
    },
    table::{PatchEmbeddingTable, PatchEmbeddingView},
};

/// Availability of the descriptive patch-overlap embedding dispersion value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchOverlapEmbeddingDispersionStatus {
    /// At least one overlap edge had two present embedding vectors.
    Available,
    /// No overlap edge had two present embedding vectors.
    InsufficientPairs,
}

/// Descriptive mean squared Euclidean embedding distance across overlapping patches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchOverlapEmbeddingDispersion {
    status: PatchOverlapEmbeddingDispersionStatus,
    measurement_status: MeasurementStatus,
    total_edge_count: u64,
    eligible_edge_count: u64,
    excluded_edge_count: u64,
    dimension: u32,
    mean_squared_euclidean_distance: Option<f64>,
    table_logical_digest: ContentDigest,
    support_artifact_id: ArtifactId,
    support_logical_digest: ContentDigest,
    overlap_artifact_id: ArtifactId,
    overlap_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
}

impl PatchOverlapEmbeddingDispersion {
    /// Availability of the numeric value.
    pub fn status(self) -> PatchOverlapEmbeddingDispersionStatus {
        self.status
    }

    /// Declared and provenance-validated measurement origin.
    pub fn measurement_status(self) -> MeasurementStatus {
        self.measurement_status
    }

    /// Total canonical positive-area overlap edges.
    pub fn total_edge_count(self) -> u64 {
        self.total_edge_count
    }

    /// Edges whose two embedding rows were present.
    pub fn eligible_edge_count(self) -> u64 {
        self.eligible_edge_count
    }

    /// Edges excluded because at least one embedding row was non-present.
    pub fn excluded_edge_count(self) -> u64 {
        self.excluded_edge_count
    }

    /// Fixed embedding dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Mean squared Euclidean distance, or `None` when no pair was eligible.
    pub fn mean_squared_euclidean_distance(self) -> Option<f64> {
        self.mean_squared_euclidean_distance
    }

    /// Format-independent patch-table identity.
    pub fn table_logical_digest(self) -> ContentDigest {
        self.table_logical_digest
    }

    /// Exact patch-support artifact identity bound by the table and provenance.
    pub fn support_artifact_id(self) -> ArtifactId {
        self.support_artifact_id
    }

    /// Format-independent patch-support identity.
    pub fn support_logical_digest(self) -> ContentDigest {
        self.support_logical_digest
    }

    /// Exact overlap-graph artifact identity declared by patch support.
    pub fn overlap_artifact_id(self) -> ArtifactId {
        self.overlap_artifact_id
    }

    /// Format-independent overlap-graph identity.
    pub fn overlap_logical_digest(self) -> ContentDigest {
        self.overlap_logical_digest
    }

    /// Exact provenance artifact identity bound by the table.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Format-independent provenance identity.
    pub fn provenance_logical_digest(self) -> ContentDigest {
        self.provenance_logical_digest
    }
}

/// Invalid measurement declaration, input binding, or bounded work request.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PatchOverlapEmbeddingDispersionError {
    /// The declared status disagrees with the closed provenance variant.
    #[error(
        "patch-overlap measurement status mismatch: expected {expected:?}, observed {observed:?}"
    )]
    MeasurementStatusMismatch {
        /// Status required by provenance.
        expected: MeasurementStatus,
        /// Caller-declared status.
        observed: MeasurementStatus,
    },
    /// This computation accepts direct patch provenance only.
    #[error("patch-overlap dispersion requires direct-patch provenance, observed {observed:?}")]
    UnsupportedProvenanceVariant {
        /// Supplied closed provenance variant.
        observed: MultiscaleEmbeddingProvenanceVariant,
    },
    /// Table rows and overlap edges do not bind the same expected patch set.
    #[error("patch-overlap table and graph expected-patch bindings disagree")]
    ExpectedPatchBindingMismatch,
    /// Table and direct-patch provenance bindings disagree.
    #[error("patch-overlap table and provenance bindings disagree")]
    ProvenanceBindingMismatch,
    /// The supplied patch-support value does not match the table.
    #[error("patch-overlap support and table bindings disagree")]
    SupportBindingMismatch,
    /// Patch-support footprint or overlap bindings disagree with the supplied graph.
    #[error("patch-overlap support and graph bindings disagree")]
    OverlapGraphBindingMismatch,
    /// The checked upper bound exceeds the caller's explicit work limit.
    #[error("patch-overlap component operations {required} exceed caller maximum {maximum}")]
    ComponentOperationBudgetExceeded {
        /// Checked `edge_count * dimension` upper bound.
        required: u64,
        /// Caller-provided maximum component operations.
        maximum: u64,
    },
    /// A count or work-bound calculation overflowed.
    #[error("patch-overlap dispersion count calculation overflowed")]
    SizeOverflow,
}

/// Compute one bounded descriptive dispersion summary over declared patch overlaps.
///
/// Every input identity is validated before arithmetic. Components and canonical edges are
/// accumulated sequentially in `f64`. The operation is `O(E log P + E*D)` with `O(1)` additional
/// retained storage. It is not a variogram, spatial-autocorrelation coefficient, or inferential
/// result.
pub fn patch_overlap_embedding_dispersion(
    table: &PatchEmbeddingTable,
    support: &MultiscaleEmbeddingSupport,
    overlap: &PatchOverlapGraph,
    provenance: &MultiscaleEmbeddingProvenance,
    measurement_status: MeasurementStatus,
    maximum_component_operations: u64,
) -> Result<PatchOverlapEmbeddingDispersion, PatchOverlapEmbeddingDispersionError> {
    let overlap_artifact_id =
        validate_bindings(table, support, overlap, provenance, measurement_status)?;
    let total_edge_count = u64::try_from(overlap.edge_count())
        .map_err(|_| PatchOverlapEmbeddingDispersionError::SizeOverflow)?;
    let required_component_operations = total_edge_count
        .checked_mul(u64::from(table.dimension()))
        .ok_or(PatchOverlapEmbeddingDispersionError::SizeOverflow)?;
    if required_component_operations > maximum_component_operations {
        return Err(
            PatchOverlapEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: required_component_operations,
                maximum: maximum_component_operations,
            },
        );
    }
    preflight_edge_rows(table, overlap)?;

    let (eligible_edge_count, total) = accumulate_present_edges(table, overlap)?;
    let (status, mean_squared_euclidean_distance) = if eligible_edge_count == 0 {
        (
            PatchOverlapEmbeddingDispersionStatus::InsufficientPairs,
            None,
        )
    } else {
        let mean = total / eligible_edge_count as f64;
        (
            PatchOverlapEmbeddingDispersionStatus::Available,
            Some(if mean == 0.0 { 0.0 } else { mean }),
        )
    };
    Ok(PatchOverlapEmbeddingDispersion {
        status,
        measurement_status,
        total_edge_count,
        eligible_edge_count,
        excluded_edge_count: total_edge_count - eligible_edge_count,
        dimension: table.dimension(),
        mean_squared_euclidean_distance,
        table_logical_digest: table.logical_digest(),
        support_artifact_id: table.support_artifact_id(),
        support_logical_digest: support.logical_digest(),
        overlap_artifact_id,
        overlap_logical_digest: overlap.logical_digest(),
        provenance_artifact_id: table.provenance_artifact_id(),
        provenance_logical_digest: provenance.logical_digest(),
    })
}

fn validate_bindings(
    table: &PatchEmbeddingTable,
    support: &MultiscaleEmbeddingSupport,
    overlap: &PatchOverlapGraph,
    provenance: &MultiscaleEmbeddingProvenance,
    observed_status: MeasurementStatus,
) -> Result<ArtifactId, PatchOverlapEmbeddingDispersionError> {
    let expected_status = provenance.measurement_status();
    if observed_status != expected_status {
        return Err(
            PatchOverlapEmbeddingDispersionError::MeasurementStatusMismatch {
                expected: expected_status,
                observed: observed_status,
            },
        );
    }
    let variant = provenance.variant();
    let roles = provenance.direct_patch_artifact_roles().ok_or(
        PatchOverlapEmbeddingDispersionError::UnsupportedProvenanceVariant { observed: variant },
    )?;
    if table.expected_entities_artifact_id() != overlap.expected_patches_artifact_id()
        || table.expected_entities_logical_digest() != overlap.expected_patches_logical_digest()
    {
        return Err(PatchOverlapEmbeddingDispersionError::ExpectedPatchBindingMismatch);
    }
    if table.owning_slide_id() != provenance.owning_slide_id()
        || table.dimension() != provenance.output_dimension()
        || table.provenance_logical_digest() != provenance.logical_digest()
        || table.expected_entities_artifact_id() != roles.expected_patches
        || table.support_artifact_id() != roles.patch_support
    {
        return Err(PatchOverlapEmbeddingDispersionError::ProvenanceBindingMismatch);
    }
    if support.variant() != MultiscaleEmbeddingSupportVariant::Patch
        || support.owning_slide_id() != table.owning_slide_id()
        || support.logical_digest() != table.support_logical_digest()
    {
        return Err(PatchOverlapEmbeddingDispersionError::SupportBindingMismatch);
    }
    let bindings = support
        .patch_bindings()
        .ok_or(PatchOverlapEmbeddingDispersionError::SupportBindingMismatch)?;
    if bindings.footprints.artifact_id() != overlap.patch_footprints_artifact_id()
        || bindings.footprints.logical_digest() != overlap.patch_footprints_logical_digest()
        || bindings.overlap.logical_digest() != overlap.logical_digest()
    {
        return Err(PatchOverlapEmbeddingDispersionError::OverlapGraphBindingMismatch);
    }
    Ok(bindings.overlap.artifact_id())
}

fn preflight_edge_rows(
    table: &PatchEmbeddingTable,
    overlap: &PatchOverlapGraph,
) -> Result<(), PatchOverlapEmbeddingDispersionError> {
    for edge in overlap.edges() {
        if table.row_index(edge.left_patch_id()).is_none()
            || table.row_index(edge.right_patch_id()).is_none()
        {
            return Err(PatchOverlapEmbeddingDispersionError::ExpectedPatchBindingMismatch);
        }
    }
    Ok(())
}

fn accumulate_present_edges(
    table: &PatchEmbeddingTable,
    overlap: &PatchOverlapGraph,
) -> Result<(u64, f64), PatchOverlapEmbeddingDispersionError> {
    let mut eligible_edge_count = 0_u64;
    let mut total = 0.0_f64;
    for edge in overlap.edges() {
        let left = table_row(table, edge.left_patch_id())?;
        let right = table_row(table, edge.right_patch_id())?;
        let (Some(left), Some(right)) = (left.vector(), right.vector()) else {
            continue;
        };
        let mut edge_distance = 0.0_f64;
        for (&left, &right) in left.iter().zip(right) {
            let difference = f64::from(left) - f64::from(right);
            edge_distance += difference * difference;
        }
        total += edge_distance;
        eligible_edge_count += 1;
    }
    Ok((eligible_edge_count, total))
}

fn table_row<'a>(
    table: &'a PatchEmbeddingTable,
    patch_id: &PatchId,
) -> Result<PatchEmbeddingView<'a>, PatchOverlapEmbeddingDispersionError> {
    let index = table
        .row_index(patch_id)
        .ok_or(PatchOverlapEmbeddingDispersionError::ExpectedPatchBindingMismatch)?;
    table
        .row(index)
        .map_err(|_| PatchOverlapEmbeddingDispersionError::ExpectedPatchBindingMismatch)
}
