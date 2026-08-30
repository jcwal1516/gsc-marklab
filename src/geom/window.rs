use std::io::Write;

use geo::{
    Area, BooleanOps, Coord, LineString, MultiPolygon as GeoMultiPolygon, Polygon as GeoPolygon,
};
use geojson::{GeoJson, Geometry, Value};
use marklab_data::{
    CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit, SpatialAxis,
};
use marklab_workflow::{ContentDigest, ContentDigestWriter};
use rstar::RTree;
use thiserror::Error;

use crate::common::finite::canonical_zero;

mod compartment;
mod topology;
pub use compartment::{
    BinaryCompartmentPartition2D, CompartmentPartitionDescriptor, CompartmentPartitionError,
    CompartmentPartitionLimits,
};
use topology::{BoundarySegment, Point, Polygon, TopologySummary};

const DIGEST_DOMAIN: &[u8] = b"marklab-observation-window-2d-v1";

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
    polygons: Vec<Polygon>,
    translation_geometry: GeoMultiPolygon<f64>,
    boundary: RTree<BoundarySegment>,
    geometry_digest: ContentDigest,
    descriptor: ObservationWindowDescriptor,
}

impl PartialEq for ObservationWindow2D {
    fn eq(&self, other: &Self) -> bool {
        self.polygons == other.polygons && self.descriptor == other.descriptor
    }
}

impl ObservationWindow2D {
    /// Parse, bound, validate, and canonicalize one GeoJSON MultiPolygon window.
    pub fn from_geojson_str(
        text: &str,
        limits: ObservationWindowLimits,
    ) -> Result<Self, ObservationWindowError> {
        if text.len() > limits.maximum_input_bytes {
            return Err(ObservationWindowError::InputByteLimitExceeded {
                observed: text.len(),
                maximum: limits.maximum_input_bytes,
            });
        }
        let geojson =
            text.parse::<GeoJson>()
                .map_err(|error| ObservationWindowError::InvalidGeoJson {
                    reason: error.to_string(),
                })?;
        let geometry = extract_geometry(geojson)?;
        let Value::MultiPolygon(raw_polygons) = geometry.value else {
            return Err(ObservationWindowError::InvalidGeoJson {
                reason: "window must contain MultiPolygon geometry".into(),
            });
        };
        if raw_polygons.is_empty() {
            return Err(ObservationWindowError::InvalidTopology {
                reason: "MultiPolygon must contain at least one component".into(),
            });
        }
        if raw_polygons.len() > limits.maximum_components {
            return Err(ObservationWindowError::ComponentLimitExceeded {
                observed: raw_polygons.len(),
                maximum: limits.maximum_components,
            });
        }

        let mut ring_count = 0_usize;
        let mut vertex_count = 0_usize;
        let mut polygons = Vec::with_capacity(raw_polygons.len());
        for raw_polygon in raw_polygons {
            if raw_polygon.is_empty() {
                return Err(ObservationWindowError::InvalidTopology {
                    reason: "polygon must contain an exterior ring".into(),
                });
            }
            ring_count = ring_count
                .checked_add(raw_polygon.len())
                .ok_or(ObservationWindowError::SizeOverflow)?;
            if ring_count > limits.maximum_rings {
                return Err(ObservationWindowError::RingLimitExceeded {
                    observed: ring_count,
                    maximum: limits.maximum_rings,
                });
            }
            let mut rings = Vec::with_capacity(raw_polygon.len());
            for raw_ring in raw_polygon {
                vertex_count = vertex_count
                    .checked_add(raw_ring.len())
                    .ok_or(ObservationWindowError::SizeOverflow)?;
                if vertex_count > limits.maximum_vertices {
                    return Err(ObservationWindowError::VertexLimitExceeded {
                        observed: vertex_count,
                        maximum: limits.maximum_vertices,
                    });
                }
                rings.push(parse_ring(&raw_ring)?);
            }
            let exterior = rings.remove(0);
            polygons.push(Polygon {
                exterior,
                holes: rings,
            });
        }

        let TopologySummary {
            area_um2,
            perimeter_um,
            bounds_um,
            hole_count,
            segments,
        } = topology::validate_and_canonicalize(&mut polygons, limits.maximum_topology_candidates)?;
        let logical_digest = window_digest(&polygons)?;
        let translation_geometry = to_geo_multipolygon(&polygons);
        let boundary = RTree::bulk_load(segments);
        let component_count = polygons.len();
        Ok(Self {
            polygons,
            translation_geometry,
            boundary,
            geometry_digest: logical_digest,
            descriptor: ObservationWindowDescriptor {
                coordinate_frame_id: None,
                area_um2,
                perimeter_um,
                bounds_um,
                component_count,
                hole_count,
                ring_count,
                vertex_count,
                logical_digest,
            },
        })
    }

    /// Bind this physical GeoJSON domain to one installed `[X,Y]` micrometre frame.
    pub fn with_coordinate_frame(
        mut self,
        registry: &CoordinateRegistry,
        frame_id: CoordinateFrameId,
    ) -> Result<Self, ObservationWindowError> {
        let frame = registry.frame(&frame_id).ok_or_else(|| {
            ObservationWindowError::UnknownCoordinateFrame {
                frame: frame_id.clone(),
            }
        })?;
        if frame.axes() != [SpatialAxis::X, SpatialAxis::Y]
            || frame.unit() != CoordinateUnit::Micrometer
            || frame.space() != CoordinateSpace::Physical
        {
            return Err(ObservationWindowError::InvalidCoordinateFrame { frame: frame_id });
        }
        if let Some(existing) = &self.descriptor.coordinate_frame_id {
            if existing != &frame_id {
                return Err(ObservationWindowError::CoordinateFrameMismatch {
                    expected: existing.clone(),
                    observed: frame_id,
                });
            }
            return Ok(self);
        }
        self.descriptor.logical_digest = ContentDigest::from_framed([
            b"marklab-observation-window-2d-framed-v1".as_slice(),
            self.geometry_digest.as_bytes(),
            frame_id.as_str().as_bytes(),
        ]);
        self.descriptor.coordinate_frame_id = Some(frame_id);
        Ok(self)
    }

    /// Exact installed frame bound to this domain, if one was declared.
    pub fn coordinate_frame_id(&self) -> Option<&CoordinateFrameId> {
        self.descriptor.coordinate_frame_id.as_ref()
    }

    /// Whether a finite point lies inside the window or on any exterior/hole boundary.
    pub fn contains(&self, x_um: f64, y_um: f64) -> bool {
        x_um.is_finite()
            && y_um.is_finite()
            && topology::window_contains(
                &self.polygons,
                [canonical_zero(x_um), canonical_zero(y_um)],
            )
    }

    /// Minimum Euclidean distance to an exterior or hole boundary.
    pub fn boundary_distance_um(
        &self,
        x_um: f64,
        y_um: f64,
    ) -> Result<f64, ObservationWindowError> {
        if !x_um.is_finite() || !y_um.is_finite() {
            return Err(ObservationWindowError::NonFiniteQueryPoint);
        }
        let point = [canonical_zero(x_um), canonical_zero(y_um)];
        self.boundary
            .nearest_neighbor(point)
            .map(|segment| segment.distance_2_to(point).sqrt())
            .ok_or_else(|| ObservationWindowError::InvalidTopology {
                reason: "window has no boundary segments".into(),
            })
    }

    /// Signed minimum distance to an exterior or hole boundary.
    ///
    /// Distances are positive in the permitted window interior, zero on any
    /// boundary, and negative outside the window or inside a hole.
    pub fn signed_boundary_distance_um(
        &self,
        x_um: f64,
        y_um: f64,
    ) -> Result<f64, ObservationWindowError> {
        let distance = self.boundary_distance_um(x_um, y_um)?;
        if distance == 0.0 {
            return Ok(0.0);
        }
        if self.contains(x_um, y_um) {
            Ok(distance)
        } else {
            Ok(-distance)
        }
    }

    /// Exact area after subtracting holes.
    pub fn area_um2(&self) -> f64 {
        self.descriptor.area_um2
    }

    /// Total exterior-plus-hole boundary length.
    pub fn perimeter_um(&self) -> f64 {
        self.descriptor.perimeter_um
    }

    /// Canonical bounded descriptor.
    pub fn descriptor(&self) -> &ObservationWindowDescriptor {
        &self.descriptor
    }

    /// Axis-aligned physical bounds used by conditional-CSR proposal sampling.
    pub(crate) fn bounds_um(&self) -> [f64; 4] {
        self.descriptor.bounds_um
    }

    pub(crate) fn boundary_storage_bytes(&self) -> usize {
        self.boundary
            .size()
            .saturating_mul(std::mem::size_of::<BoundarySegment>())
            .saturating_mul(3)
    }

    pub(crate) fn component_areas_um2(&self) -> Vec<f64> {
        let mut areas = self
            .polygons
            .iter()
            .map(topology::polygon_area)
            .collect::<Vec<_>>();
        areas.sort_by(f64::total_cmp);
        areas
    }

    pub(crate) fn translation_segment_count(&self) -> usize {
        self.descriptor
            .vertex_count
            .saturating_sub(self.descriptor.ring_count)
    }

    pub(crate) fn translation_overlap_area_um2(
        &self,
        displacement_x_um: f64,
        displacement_y_um: f64,
        maximum_output_vertices: usize,
    ) -> Result<TranslationOverlap2D, ObservationWindowError> {
        if !displacement_x_um.is_finite() || !displacement_y_um.is_finite() {
            return Err(ObservationWindowError::NonFiniteTranslation);
        }
        let translated = translate_geo_multipolygon(
            &self.translation_geometry,
            displacement_x_um,
            displacement_y_um,
        )?;
        let intersection = self.translation_geometry.intersection(&translated);
        let output_vertices = intersection
            .0
            .iter()
            .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
            .map(|ring| ring.0.len())
            .try_fold(0_usize, |total, count| total.checked_add(count))
            .ok_or(ObservationWindowError::SizeOverflow)?;
        if output_vertices > maximum_output_vertices {
            return Err(
                ObservationWindowError::TranslationOutputVertexLimitExceeded {
                    observed: output_vertices,
                    maximum: maximum_output_vertices,
                },
            );
        }
        let area_um2 = intersection.unsigned_area();
        if !area_um2.is_finite() {
            return Err(ObservationWindowError::NonFiniteTranslationOverlap);
        }
        Ok(TranslationOverlap2D {
            area_um2: canonical_zero(area_um2),
            output_vertices,
        })
    }

    pub(crate) fn visible_circle_arc_fraction(
        &self,
        center_x_um: f64,
        center_y_um: f64,
        radius_um: f64,
        maximum_membership_queries: usize,
    ) -> Result<VisibleCircleArc2D, ObservationWindowError> {
        if !center_x_um.is_finite()
            || !center_y_um.is_finite()
            || !radius_um.is_finite()
            || radius_um <= 0.0
        {
            return Err(ObservationWindowError::InvalidVisibleCircle);
        }
        if !self.contains(center_x_um, center_y_um) {
            return Err(ObservationWindowError::VisibleCircleCenterOutsideWindow);
        }
        let mut angles = Vec::new();
        angles
            .try_reserve_exact(self.translation_segment_count().saturating_mul(2))
            .map_err(|_| ObservationWindowError::VisibleCircleAllocationFailed)?;
        for ring in self
            .polygons
            .iter()
            .flat_map(|polygon| std::iter::once(&polygon.exterior).chain(polygon.holes.iter()))
        {
            for segment in ring.windows(2) {
                circle_segment_intersection_angles(
                    [center_x_um, center_y_um],
                    radius_um,
                    segment[0],
                    segment[1],
                    &mut angles,
                )?;
            }
        }
        angles.sort_by(f64::total_cmp);
        deduplicate_angles(&mut angles);
        let membership_queries = angles.len().max(1);
        if membership_queries > maximum_membership_queries {
            return Err(
                ObservationWindowError::VisibleArcMembershipQueryLimitExceeded {
                    required: membership_queries,
                    maximum: maximum_membership_queries,
                },
            );
        }
        let full_turn = std::f64::consts::TAU;
        let visible_angle = if angles.is_empty() {
            let x = center_x_um + radius_um;
            if !x.is_finite() {
                return Err(ObservationWindowError::InvalidVisibleCircle);
            }
            if self.contains(x, center_y_um) {
                full_turn
            } else {
                0.0
            }
        } else {
            let mut visible = 0.0;
            for index in 0..angles.len() {
                let start = angles[index];
                let end = if index + 1 < angles.len() {
                    angles[index + 1]
                } else {
                    angles[0] + full_turn
                };
                let midpoint = start + (end - start) * 0.5;
                let x = center_x_um + radius_um * midpoint.cos();
                let y = center_y_um + radius_um * midpoint.sin();
                if !x.is_finite() || !y.is_finite() {
                    return Err(ObservationWindowError::InvalidVisibleCircle);
                }
                if self.contains(x, y) {
                    visible += end - start;
                }
            }
            visible
        };
        let fraction = visible_angle / full_turn;
        let tolerance = 256.0 * f64::EPSILON;
        if !fraction.is_finite() || fraction < -tolerance || fraction > 1.0 + tolerance {
            return Err(ObservationWindowError::NonFiniteVisibleArcFraction);
        }
        let fraction = canonical_zero(fraction.clamp(0.0, 1.0));
        Ok(VisibleCircleArc2D {
            fraction,
            boundary_intersection_angles: angles.len(),
            membership_queries,
        })
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

fn circle_segment_intersection_angles(
    center: Point,
    radius: f64,
    start: Point,
    end: Point,
    angles: &mut Vec<f64>,
) -> Result<(), ObservationWindowError> {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let relative = [start[0] - center[0], start[1] - center[1]];
    let a = direction[0] * direction[0] + direction[1] * direction[1];
    let b = 2.0 * (relative[0] * direction[0] + relative[1] * direction[1]);
    let c = relative[0] * relative[0] + relative[1] * relative[1] - radius * radius;
    let mut discriminant = b * b - 4.0 * a * c;
    if !a.is_finite() || a <= 0.0 || !discriminant.is_finite() {
        return Err(ObservationWindowError::InvalidVisibleCircle);
    }
    let tolerance = 64.0 * f64::EPSILON * (b * b + (4.0 * a * c).abs() + 1.0);
    if discriminant < -tolerance {
        return Ok(());
    }
    discriminant = discriminant.max(0.0);
    let root = discriminant.sqrt();
    let denominator = 2.0 * a;
    let parameter_tolerance = 64.0 * f64::EPSILON;
    let parameters = [(-b - root) / denominator, (-b + root) / denominator];
    for (index, parameter) in parameters.into_iter().enumerate() {
        if index == 1 && root == 0.0 {
            continue;
        }
        if parameter < -parameter_tolerance || parameter > 1.0 + parameter_tolerance {
            continue;
        }
        let parameter = parameter.clamp(0.0, 1.0);
        let x = start[0] + parameter * direction[0];
        let y = start[1] + parameter * direction[1];
        let mut angle = (y - center[1]).atan2(x - center[0]);
        if angle < 0.0 {
            angle += std::f64::consts::TAU;
        }
        if !angle.is_finite() {
            return Err(ObservationWindowError::InvalidVisibleCircle);
        }
        angles.push(canonical_zero(angle));
    }
    Ok(())
}

fn deduplicate_angles(angles: &mut Vec<f64>) {
    const ANGLE_TOLERANCE: f64 = 256.0 * f64::EPSILON;
    angles.dedup_by(|right, left| (*right - *left).abs() <= ANGLE_TOLERANCE);
    if angles.len() > 1
        && (angles[0] + std::f64::consts::TAU - angles[angles.len() - 1]).abs() <= ANGLE_TOLERANCE
    {
        angles.pop();
    }
}

fn to_geo_multipolygon(polygons: &[Polygon]) -> GeoMultiPolygon<f64> {
    GeoMultiPolygon::new(
        polygons
            .iter()
            .map(|polygon| {
                GeoPolygon::new(
                    to_geo_ring(&polygon.exterior),
                    polygon.holes.iter().map(|ring| to_geo_ring(ring)).collect(),
                )
            })
            .collect(),
    )
}

fn to_geo_ring(ring: &[Point]) -> LineString<f64> {
    LineString::new(
        ring.iter()
            .map(|point| Coord {
                x: point[0],
                y: point[1],
            })
            .collect(),
    )
}

fn translate_geo_multipolygon(
    geometry: &GeoMultiPolygon<f64>,
    displacement_x_um: f64,
    displacement_y_um: f64,
) -> Result<GeoMultiPolygon<f64>, ObservationWindowError> {
    geometry
        .0
        .iter()
        .map(|polygon| {
            Ok(GeoPolygon::new(
                translate_geo_ring(polygon.exterior(), displacement_x_um, displacement_y_um)?,
                polygon
                    .interiors()
                    .iter()
                    .map(|ring| translate_geo_ring(ring, displacement_x_um, displacement_y_um))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(GeoMultiPolygon::new)
}

fn translate_geo_ring(
    ring: &LineString<f64>,
    displacement_x_um: f64,
    displacement_y_um: f64,
) -> Result<LineString<f64>, ObservationWindowError> {
    ring.0
        .iter()
        .map(|coordinate| {
            let x = coordinate.x + displacement_x_um;
            let y = coordinate.y + displacement_y_um;
            if !x.is_finite() || !y.is_finite() {
                return Err(ObservationWindowError::TranslationCoordinateOverflow);
            }
            Ok(Coord { x, y })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(LineString::new)
}

fn extract_geometry(geojson: GeoJson) -> Result<Geometry, ObservationWindowError> {
    match geojson {
        GeoJson::Geometry(geometry) => Ok(geometry),
        GeoJson::Feature(feature) => {
            feature
                .geometry
                .ok_or_else(|| ObservationWindowError::InvalidGeoJson {
                    reason: "Feature must contain geometry".into(),
                })
        }
        GeoJson::FeatureCollection(collection) if collection.features.len() == 1 => collection
            .features
            .into_iter()
            .next()
            .and_then(|feature| feature.geometry)
            .ok_or_else(|| ObservationWindowError::InvalidGeoJson {
                reason: "one-feature FeatureCollection must contain geometry".into(),
            }),
        GeoJson::FeatureCollection(_) => Err(ObservationWindowError::InvalidGeoJson {
            reason: "FeatureCollection must contain exactly one feature".into(),
        }),
    }
}

fn parse_ring(raw: &[Vec<f64>]) -> Result<Vec<Point>, ObservationWindowError> {
    if raw.len() < 4 {
        return Err(ObservationWindowError::InvalidTopology {
            reason: "linear ring must contain at least four positions".into(),
        });
    }
    let mut ring = Vec::with_capacity(raw.len());
    for position in raw {
        if position.len() != 2 || !position[0].is_finite() || !position[1].is_finite() {
            return Err(ObservationWindowError::InvalidGeoJson {
                reason: "positions must contain exactly two finite coordinates".into(),
            });
        }
        ring.push([canonical_zero(position[0]), canonical_zero(position[1])]);
    }
    if ring.first() != ring.last() {
        return Err(ObservationWindowError::InvalidTopology {
            reason: "linear ring must be closed".into(),
        });
    }
    Ok(ring)
}

fn window_digest(polygons: &[Polygon]) -> Result<ContentDigest, ObservationWindowError> {
    let mut writer = ContentDigest::builder();
    write_digest_part(&mut writer, DIGEST_DOMAIN)?;
    write_digest_part(&mut writer, &(polygons.len() as u128).to_be_bytes())?;
    for polygon in polygons {
        write_ring_digest(&mut writer, b"exterior", &polygon.exterior)?;
        for hole in &polygon.holes {
            write_ring_digest(&mut writer, b"hole", hole)?;
        }
    }
    Ok(writer.finish().0)
}

fn write_ring_digest(
    writer: &mut ContentDigestWriter,
    role: &[u8],
    ring: &[Point],
) -> Result<(), ObservationWindowError> {
    write_digest_part(writer, role)?;
    write_digest_part(writer, &(ring.len() as u128).to_be_bytes())?;
    for [x, y] in ring {
        write_digest_part(writer, &x.to_bits().to_be_bytes())?;
        write_digest_part(writer, &y.to_bits().to_be_bytes())?;
    }
    Ok(())
}

fn write_digest_part(
    writer: &mut ContentDigestWriter,
    bytes: &[u8],
) -> Result<(), ObservationWindowError> {
    writer
        .write_all(&(bytes.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(bytes))
        .map_err(|_| ObservationWindowError::SizeOverflow)
}
