use std::io::Write;

use geo::{Coord, LineString, MultiPolygon as GeoMultiPolygon, Polygon as GeoPolygon};
use geojson::{GeoJson, Geometry, Value};
use marklab_data::{
    CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit, SpatialAxis,
};
use marklab_workflow::{ContentDigest, ContentDigestWriter};
use rstar::RTree;

use crate::common::finite::canonical_zero;

use super::{
    topology::{self, Point, Polygon, TopologySummary},
    ObservationWindow2D, ObservationWindowDescriptor, ObservationWindowError,
    ObservationWindowLimits,
};

const DIGEST_DOMAIN: &[u8] = b"marklab-observation-window-2d-v1";

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
