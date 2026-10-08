use geo::{
    algorithm::buffer::{BufferStyle, LineJoin},
    Area, BooleanOps, Buffer,
};

use super::{ObservationWindow2D, ObservationWindowError};

pub(crate) const PROFILE_BUFFER_ANGLE: f64 = 0.05;

impl ObservationWindow2D {
    /// Area of an annotation offset clipped to this tissue window. Round arcs are polygonal.
    pub(crate) fn clipped_annotation_offset_area(
        &self,
        annotation: &Self,
        distance_um: f64,
        maximum_vertices: usize,
    ) -> Result<f64, ObservationWindowError> {
        let offset = if distance_um == 0.0 {
            annotation.translation_geometry.clone()
        } else {
            annotation.translation_geometry.buffer_with_style(
                BufferStyle::new(distance_um).line_join(LineJoin::Round(PROFILE_BUFFER_ANGLE)),
            )
        };
        let count = |geometry: &geo::MultiPolygon<f64>| {
            geometry
                .0
                .iter()
                .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
                .map(|ring| ring.0.len())
                .sum::<usize>()
        };
        let observed = count(&offset);
        if observed > maximum_vertices {
            return Err(ObservationWindowError::VertexLimitExceeded {
                observed,
                maximum: maximum_vertices,
            });
        }
        let clipped = self.translation_geometry.intersection(&offset);
        let observed = count(&clipped);
        if observed > maximum_vertices {
            return Err(ObservationWindowError::VertexLimitExceeded {
                observed,
                maximum: maximum_vertices,
            });
        }
        let area = clipped.unsigned_area();
        if !area.is_finite() || area < 0.0 || area > self.area_um2() * (1.0 + 1e-10) {
            return Err(ObservationWindowError::InvalidTopology {
                reason: "nonfinite or inconsistent clipped annotation offset area".into(),
            });
        }
        Ok(area.min(self.area_um2()))
    }
}
