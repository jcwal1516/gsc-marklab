use geo::{Area, BooleanOps, Coord, LineString, MultiPolygon, Polygon, Rect};
use rstar::AABB;

use super::{ObservationWindow2D, ObservationWindowError};

/// Exact intersection of a study tile with the window.
pub(crate) enum StudyTile {
    /// The closed tile meets no boundary segment and lies outside the window.
    Outside,
    /// The closed tile meets no boundary segment and lies inside the window.
    Inside {
        area: f64,
        geometry: MultiPolygon<f64>,
    },
    /// Some boundary segment's envelope meets the tile; it needs clipping.
    Boundary,
}

impl ObservationWindow2D {
    /// Exact polygon intersection for study tiles and bounded polygonal scan displays.
    pub(crate) fn clip_study_polygon(
        &self,
        polygon: &Polygon<f64>,
        maximum_vertices: usize,
    ) -> Result<(f64, MultiPolygon<f64>), ObservationWindowError> {
        let clipped = self
            .translation_geometry
            .intersection(&MultiPolygon(vec![polygon.clone()]));
        let vertices = clipped
            .0
            .iter()
            .flat_map(|p| std::iter::once(p.exterior()).chain(p.interiors()))
            .map(|r| r.0.len())
            .sum();
        if vertices > maximum_vertices {
            return Err(ObservationWindowError::VertexLimitExceeded {
                observed: vertices,
                maximum: maximum_vertices,
            });
        }
        let area = clipped.unsigned_area();
        if !area.is_finite() || area < 0.0 || area > self.area_um2() * (1.0 + 1e-10) {
            return Err(ObservationWindowError::InvalidTopology {
                reason: "invalid clipped study area".into(),
            });
        }
        Ok((area, clipped))
    }

    /// Classify a closed tile without clipping when no boundary can cross it.
    ///
    /// A closed rectangle that meets no boundary segment is connected and
    /// disjoint from the boundary, so it lies wholly inside or wholly outside
    /// the window; its center decides which. Any segment envelope that meets
    /// the tile, including at an edge or corner, conservatively requires the
    /// exact clip. Inside tiles use the clipper's counterclockwise ring order,
    /// starting at the upper-left corner, with exact corner coordinates; the
    /// clipper itself can quantize slide-scale coordinates by nanometers.
    pub(crate) fn classify_study_tile(&self, tile: &Rect<f64>) -> StudyTile {
        let (min, max) = (tile.min(), tile.max());
        if self
            .boundary
            .locate_in_envelope_intersecting(AABB::from_corners([min.x, min.y], [max.x, max.y]))
            .next()
            .is_some()
        {
            return StudyTile::Boundary;
        }
        if !self.contains(0.5 * (min.x + max.x), 0.5 * (min.y + max.y)) {
            return StudyTile::Outside;
        }
        let ring = [
            (min.x, max.y),
            (min.x, min.y),
            (max.x, min.y),
            (max.x, max.y),
            (min.x, max.y),
        ];
        let geometry = MultiPolygon(vec![Polygon::new(
            LineString(ring.into_iter().map(|(x, y)| Coord { x, y }).collect()),
            Vec::new(),
        )]);
        StudyTile::Inside {
            area: geometry.unsigned_area(),
            geometry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclipped_tiles_match_the_exact_clip() {
        // A concave L-shaped window with a hole; 7.5-um tiles from a shifted
        // origin touch edges, corners, and the hole in every arrangement.
        let window = ObservationWindow2D::from_geojson_str(
            r#"{"type":"MultiPolygon","coordinates":[[
                [[0,0],[100,0],[100,40],[40,40],[40,100],[0,100],[0,0]],
                [[10,10],[25,10],[25,25],[10,25],[10,10]]
            ]]}"#,
            crate::ObservationWindowLimits::default(),
        )
        .expect("window");
        let mut counts = [0_usize; 3];
        for row in 0..15 {
            for column in 0..15 {
                let (x, y) = (-4.0 + column as f64 * 7.5, -4.0 + row as f64 * 7.5);
                let tile = Rect::new(
                    Coord { x, y },
                    Coord {
                        x: x + 7.5,
                        y: y + 7.5,
                    },
                );
                let (area, clipped) = window
                    .clip_study_polygon(&tile.to_polygon(), 1_000)
                    .expect("exact clip");
                match window.classify_study_tile(&tile) {
                    StudyTile::Outside => {
                        counts[0] += 1;
                        assert_eq!(area, 0.0, "{tile:?}");
                    }
                    StudyTile::Inside {
                        area: inside_area,
                        geometry,
                    } => {
                        counts[1] += 1;
                        assert_eq!(inside_area.to_bits(), area.to_bits(), "{tile:?}");
                        assert_eq!(geometry, clipped, "{tile:?}");
                    }
                    StudyTile::Boundary => counts[2] += 1,
                }
            }
        }
        assert!(counts.iter().all(|count| *count > 0), "{counts:?}");
    }
}
