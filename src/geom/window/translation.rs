use geo::{
    Area, BooleanOps, Coord, LineString, MultiPolygon as GeoMultiPolygon, Polygon as GeoPolygon,
};

use crate::common::finite::canonical_zero;

use super::{ObservationWindow2D, ObservationWindowError, TranslationOverlap2D};

impl ObservationWindow2D {
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
