use crate::common::finite::canonical_zero;

use super::{
    topology::{self, BoundarySegment},
    ObservationWindow2D, ObservationWindowDescriptor, ObservationWindowError,
};

impl ObservationWindow2D {
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
}
