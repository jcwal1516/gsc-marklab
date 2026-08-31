use geo::MultiPolygon as GeoMultiPolygon;
use marklab_data::CoordinateFrameId;
use marklab_workflow::ContentDigest;
use rstar::RTree;
use thiserror::Error;

use super::topology::{BoundarySegment, Polygon};

/// Fixed resource ceilings for decoding and validating one 2-D observation window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationWindowLimits {
    /// Maximum UTF-8 GeoJSON bytes.
    pub maximum_input_bytes: usize,
    /// Maximum polygon components.
    pub maximum_components: usize,
    /// Maximum exterior-plus-hole rings.
    pub maximum_rings: usize,
    /// Maximum positions across all rings, including closing positions.
    pub maximum_vertices: usize,
    /// Maximum segment-pair candidates inspected during topology validation.
    pub maximum_topology_candidates: usize,
}

impl ObservationWindowLimits {
    /// Validate positive explicit resource ceilings.
    pub fn new(
        maximum_input_bytes: usize,
        maximum_components: usize,
        maximum_rings: usize,
        maximum_vertices: usize,
        maximum_topology_candidates: usize,
    ) -> Result<Self, ObservationWindowError> {
        let limits = Self {
            maximum_input_bytes,
            maximum_components,
            maximum_rings,
            maximum_vertices,
            maximum_topology_candidates,
        };
        if [
            maximum_input_bytes,
            maximum_components,
            maximum_rings,
            maximum_vertices,
            maximum_topology_candidates,
        ]
        .contains(&0)
        {
            return Err(ObservationWindowError::InvalidResourceLimit);
        }
        Ok(limits)
    }
}

impl Default for ObservationWindowLimits {
    fn default() -> Self {
        Self {
            maximum_input_bytes: 16 * 1024 * 1024,
            maximum_components: 4_096,
            maximum_rings: 65_536,
            maximum_vertices: 1_000_000,
            maximum_topology_candidates: 8_000_000,
        }
    }
}

/// Bounded summary and identity of an exact physical 2-D observation window.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservationWindowDescriptor {
    /// Exact installed physical coordinate frame, absent only for the legacy unbound constructor.
    pub coordinate_frame_id: Option<CoordinateFrameId>,
    /// Area after subtracting holes, in square micrometres.
    pub area_um2: f64,
    /// Exterior-plus-hole boundary length in micrometres.
    pub perimeter_um: f64,
    /// Axis-aligned [min_x, min_y, max_x, max_y] bounds in micrometres.
    pub bounds_um: [f64; 4],
    /// Number of disjoint polygon components.
    pub component_count: usize,
    /// Number of holes.
    pub hole_count: usize,
    /// Total exterior-plus-hole rings.
    pub ring_count: usize,
    /// Total positions including one closing position per ring.
    pub vertex_count: usize,
    /// Canonical orientation- and signed-zero-normalized geometry identity.
    pub logical_digest: ContentDigest,
}

/// Exact bounded polygon/multipolygon domain for a physical 2-D point pattern.
#[derive(Clone, Debug)]
pub struct ObservationWindow2D {
    pub(super) polygons: Vec<Polygon>,
    pub(super) translation_geometry: GeoMultiPolygon<f64>,
    pub(super) boundary: RTree<BoundarySegment>,
    pub(super) geometry_digest: ContentDigest,
    pub(super) descriptor: ObservationWindowDescriptor,
}

impl PartialEq for ObservationWindow2D {
    fn eq(&self, other: &Self) -> bool {
        self.polygons == other.polygons && self.descriptor == other.descriptor
    }
}

pub(crate) struct TranslationOverlap2D {
    pub(crate) area_um2: f64,
    pub(crate) output_vertices: usize,
}

pub(crate) struct VisibleCircleArc2D {
    pub(crate) fraction: f64,
    pub(crate) boundary_intersection_angles: usize,
    pub(crate) membership_queries: usize,
}

/// Failure to decode, validate, or query an exact 2-D observation window.
#[derive(Debug, Error, PartialEq)]
pub enum ObservationWindowError {
    /// One or more caller resource limits are zero.
    #[error("observation-window resource limits must be positive")]
    InvalidResourceLimit,
    /// Input bytes exceed the caller ceiling.
    #[error("window GeoJSON has {observed} bytes; maximum is {maximum}")]
    InputByteLimitExceeded {
        /// Observed bytes.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Component count exceeds the caller ceiling.
    #[error("window has {observed} components; maximum is {maximum}")]
    ComponentLimitExceeded {
        /// Observed components.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Ring count exceeds the caller ceiling.
    #[error("window has {observed} rings; maximum is {maximum}")]
    RingLimitExceeded {
        /// Observed rings.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Position count exceeds the caller ceiling.
    #[error("window has {observed} positions; maximum is {maximum}")]
    VertexLimitExceeded {
        /// Observed positions.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Topology candidate work exceeds the caller ceiling.
    #[error("window topology requires more than {maximum} segment candidates")]
    TopologyCandidateLimitExceeded {
        /// Caller ceiling.
        maximum: usize,
    },
    /// GeoJSON cannot represent the exact accepted input shape.
    #[error("invalid observation-window GeoJSON: {reason}")]
    InvalidGeoJson {
        /// Redacted structural reason.
        reason: String,
    },
    /// Polygon topology is invalid or ambiguous.
    #[error("invalid observation-window topology: {reason}")]
    InvalidTopology {
        /// Deterministic topology reason.
        reason: String,
    },
    /// The requested coordinate frame is absent from the installed registry.
    #[error("observation-window coordinate frame {frame} is not installed")]
    UnknownCoordinateFrame {
        /// Missing frame.
        frame: CoordinateFrameId,
    },
    /// The requested frame is not physical `[X,Y]` micrometres.
    #[error("observation-window coordinate frame {frame} is not physical [X,Y] micrometres")]
    InvalidCoordinateFrame {
        /// Incompatible frame.
        frame: CoordinateFrameId,
    },
    /// A previously bound window was rebound to a different frame.
    #[error(
        "observation-window coordinate frame mismatch: expected {expected}, observed {observed}"
    )]
    CoordinateFrameMismatch {
        /// Existing exact frame.
        expected: CoordinateFrameId,
        /// Attempted exact frame.
        observed: CoordinateFrameId,
    },
    /// Boundary query coordinates are non-finite.
    #[error("observation-window query point must be finite")]
    NonFiniteQueryPoint,
    /// Translation displacement is non-finite.
    #[error("observation-window translation displacement must be finite")]
    NonFiniteTranslation,
    /// Translating a finite input coordinate overflowed.
    #[error("observation-window translated coordinate is non-finite")]
    TranslationCoordinateOverflow,
    /// Boolean-intersection output exceeds the caller ceiling.
    #[error("translation overlap produced {observed} positions; maximum is {maximum}")]
    TranslationOutputVertexLimitExceeded {
        /// Observed output positions.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Boolean-intersection area is non-finite.
    #[error("translation overlap area is non-finite")]
    NonFiniteTranslationOverlap,
    /// Circle center or radius is invalid.
    #[error("visible-circle center must be finite and radius must be finite and positive")]
    InvalidVisibleCircle,
    /// Circle center is not in the closed observation window.
    #[error("visible-circle center is outside the observation window")]
    VisibleCircleCenterOutsideWindow,
    /// Angular intersection allocation failed.
    #[error("visible-circle angular partition allocation failed")]
    VisibleCircleAllocationFailed,
    /// Arc classification exceeds the caller ceiling.
    #[error(
        "visible-circle partition requires {required} membership queries; maximum is {maximum}"
    )]
    VisibleArcMembershipQueryLimitExceeded {
        /// Required open-arc queries.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Visible circumference fraction is non-finite or outside `[0, 1]`.
    #[error("visible-circle arc fraction is non-finite or outside [0, 1]")]
    NonFiniteVisibleArcFraction,
    /// Checked count or byte arithmetic overflowed.
    #[error("observation-window size arithmetic overflow")]
    SizeOverflow,
}
