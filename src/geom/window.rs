use std::io::Write;

use geojson::{GeoJson, Geometry, Value};
use marklab_data::{
    CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit, SpatialAxis,
};
use marklab_workflow::{ContentDigest, ContentDigestWriter};
use rstar::RTree;
use thiserror::Error;

mod topology;
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
        let boundary = RTree::bulk_load(segments);
        let component_count = polygons.len();
        Ok(Self {
            polygons,
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
    /// Checked count or byte arithmetic overflowed.
    #[error("observation-window size arithmetic overflow")]
    SizeOverflow,
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

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}
