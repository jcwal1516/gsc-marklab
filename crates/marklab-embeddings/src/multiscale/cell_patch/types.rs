use std::fmt;

use marklab_data::{CellId, CoordinateFrameId, PatchId};
use marklab_project::{ArtifactId, ContentDigest};

use super::super::error::MultiscaleEmbeddingError;

/// Closed table-wide cell-to-patch assignment mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellPatchAssignmentMode {
    /// Assign every anchor to every containing half-open patch footprint.
    ContainedShared,
    /// Preserve one explicit canonical interpolation group per expected cell.
    DeclaredWeightedInterpolation,
}

impl CellPatchAssignmentMode {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::ContainedShared => "contained_shared",
            Self::DeclaredWeightedInterpolation => "declared_weighted_interpolation",
        }
    }
}

/// Closed per-cell assignment state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellPatchAssignmentStatus {
    /// One or more canonical patch edges are assigned.
    Assigned,
    /// No sampled half-open patch footprint contains the anchor.
    OutsideSampledSupport,
    /// The declared interpolation producer supplied no contributors.
    InterpolationUnavailable,
}

impl CellPatchAssignmentStatus {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::Assigned => "assigned",
            Self::OutsideSampledSupport => "outside_sampled_support",
            Self::InterpolationUnavailable => "interpolation_unavailable",
        }
    }
}

/// One finite cell anchor in the exact patch image frame.
#[derive(Clone, PartialEq)]
pub struct CellPatchAnchor {
    pub(super) cell_id: CellId,
    pub(super) anchor_px: [f64; 2],
}

impl fmt::Debug for CellPatchAnchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchAnchor")
            .finish_non_exhaustive()
    }
}

impl CellPatchAnchor {
    /// Validate finite X/Y values and canonicalize signed zero.
    pub fn new(cell_id: CellId, anchor_px: [f64; 2]) -> Result<Self, MultiscaleEmbeddingError> {
        if anchor_px.iter().any(|value| !value.is_finite()) {
            return Err(MultiscaleEmbeddingError::InvalidCellPatchAnchor);
        }
        Ok(Self {
            cell_id,
            anchor_px: anchor_px.map(canonical_zero),
        })
    }

    /// Canonical cell identity.
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Finite signed-zero-canonical X/Y anchor coordinates.
    pub fn anchor_px(&self) -> [f64; 2] {
        self.anchor_px
    }
}

/// One positive contributor in a declared interpolation group.
#[derive(Clone, Eq, PartialEq)]
pub struct CellPatchContributor {
    pub(super) patch_id: PatchId,
    pub(super) numerator: u64,
    pub(super) denominator: u64,
}

impl fmt::Debug for CellPatchContributor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchContributor")
            .finish_non_exhaustive()
    }
}

impl CellPatchContributor {
    /// Declare one positive numerator and denominator for a typed patch.
    pub fn new(
        patch_id: PatchId,
        numerator: u64,
        denominator: u64,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if numerator == 0 || denominator == 0 {
            return Err(MultiscaleEmbeddingError::InvalidCellPatchContributor);
        }
        Ok(Self {
            patch_id,
            numerator,
            denominator,
        })
    }

    /// Referenced patch identity.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    /// Positive interpolation numerator.
    pub fn numerator(&self) -> u64 {
        self.numerator
    }

    /// Positive common group denominator.
    pub fn denominator(&self) -> u64 {
        self.denominator
    }
}

/// One expected cell anchor plus its producer-declared interpolation contributors.
#[derive(Clone, PartialEq)]
pub struct DeclaredCellPatchAssignment {
    pub(super) anchor: CellPatchAnchor,
    pub(super) contributors: Vec<CellPatchContributor>,
}

impl fmt::Debug for DeclaredCellPatchAssignment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeclaredCellPatchAssignment")
            .field("contributor_count", &self.contributors.len())
            .finish()
    }
}

impl DeclaredCellPatchAssignment {
    /// Preserve one anchor and its declared contributors for link validation.
    pub fn new(anchor: CellPatchAnchor, contributors: Vec<CellPatchContributor>) -> Self {
        Self {
            anchor,
            contributors,
        }
    }

    /// Declared cell anchor.
    pub fn anchor(&self) -> &CellPatchAnchor {
        &self.anchor
    }

    /// Declared contributors in required canonical patch order.
    pub fn contributors(&self) -> &[CellPatchContributor] {
        &self.contributors
    }
}

/// Exact artifact and coordinate-frame bindings supplied to cell-link construction.
#[derive(Clone, Eq, PartialEq)]
pub struct CellPatchLinkBindings {
    pub(super) expected_cells_artifact_id: ArtifactId,
    pub(super) patch_footprints_artifact_id: ArtifactId,
    pub(super) producer_artifact_id: ArtifactId,
    pub(super) producer_content_digest: ContentDigest,
    pub(super) anchor_frame_id: CoordinateFrameId,
}

impl fmt::Debug for CellPatchLinkBindings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchLinkBindings")
            .finish_non_exhaustive()
    }
}

impl CellPatchLinkBindings {
    /// Bind expected cells, exact footprints, producer content, and anchor frame.
    pub fn new(
        expected_cells_artifact_id: ArtifactId,
        patch_footprints_artifact_id: ArtifactId,
        producer_artifact_id: ArtifactId,
        producer_content_digest: ContentDigest,
        anchor_frame_id: CoordinateFrameId,
    ) -> Self {
        Self {
            expected_cells_artifact_id,
            patch_footprints_artifact_id,
            producer_artifact_id,
            producer_content_digest,
            anchor_frame_id,
        }
    }

    /// Expected-cell artifact identity.
    pub fn expected_cells_artifact_id(&self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Exact patch-footprint artifact identity.
    pub fn patch_footprints_artifact_id(&self) -> ArtifactId {
        self.patch_footprints_artifact_id
    }

    /// Cell-link producer artifact identity.
    pub fn producer_artifact_id(&self) -> ArtifactId {
        self.producer_artifact_id
    }

    /// Cell-link producer exact content digest.
    pub fn producer_content_digest(&self) -> ContentDigest {
        self.producer_content_digest
    }

    /// Exact image frame in which anchors are declared.
    pub fn anchor_frame_id(&self) -> &CoordinateFrameId {
        &self.anchor_frame_id
    }
}

/// One positive exact interpolation weight.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CellPatchWeight {
    numerator: u64,
    denominator: u64,
}

impl fmt::Debug for CellPatchWeight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchWeight")
            .finish_non_exhaustive()
    }
}

impl CellPatchWeight {
    pub(super) fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Positive numerator.
    pub fn numerator(self) -> u64 {
        self.numerator
    }

    /// Positive common group denominator.
    pub fn denominator(self) -> u64 {
        self.denominator
    }
}

/// One canonical expected-cell assignment row.
#[derive(Clone, PartialEq)]
pub struct CellPatchAssignment {
    pub(super) cell_id: CellId,
    pub(super) status: CellPatchAssignmentStatus,
    pub(super) anchor_px: [f64; 2],
    pub(super) edge_start: u64,
    pub(super) edge_count: u64,
}

impl fmt::Debug for CellPatchAssignment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchAssignment")
            .field("status", &self.status)
            .field("edge_start", &self.edge_start)
            .field("edge_count", &self.edge_count)
            .finish()
    }
}

impl CellPatchAssignment {
    /// Canonical cell identity.
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Closed mode-compatible assignment status.
    pub fn status(&self) -> CellPatchAssignmentStatus {
        self.status
    }

    /// Finite anchor coordinates in the bound image frame.
    pub fn anchor_px(&self) -> [f64; 2] {
        self.anchor_px
    }

    /// Zero-based first edge row for this assignment.
    pub fn edge_start(&self) -> u64 {
        self.edge_start
    }

    /// Number of contiguous edge rows for this assignment.
    pub fn edge_count(&self) -> u64 {
        self.edge_count
    }
}

/// One vector-free canonical cell-to-patch edge.
#[derive(Clone, PartialEq)]
pub struct CellPatchEdge {
    pub(super) assignment_row: u64,
    pub(super) patch_id: PatchId,
    pub(super) weight: Option<CellPatchWeight>,
}

impl fmt::Debug for CellPatchEdge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchEdge")
            .field("assignment_row", &self.assignment_row)
            .field("weighted", &self.weight.is_some())
            .finish()
    }
}

impl CellPatchEdge {
    /// Owning zero-based assignment row.
    pub fn assignment_row(&self) -> u64 {
        self.assignment_row
    }

    /// Referenced shared patch row identity.
    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    /// Exact interpolation weight, absent in contained-shared mode.
    pub fn weight(&self) -> Option<CellPatchWeight> {
        self.weight
    }
}

pub(super) fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}
