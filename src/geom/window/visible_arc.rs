use crate::common::finite::canonical_zero;

use super::{topology::Point, ObservationWindow2D, ObservationWindowError, VisibleCircleArc2D};

impl ObservationWindow2D {
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
