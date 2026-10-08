use crate::{
    geom::window::ObservationWindowError, IsotropicSpatialError, IsotropicSpatialLimits,
    ObservationWindow2D,
};

/// Shared exact visible-arc work accounting for the immediate K/L and g callers.
pub(crate) struct IsotropicArcBudget {
    limits: IsotropicSpatialLimits,
    segment_count: usize,
    pub(crate) pair_visits: usize,
    pub(crate) visible_arc_evaluations: usize,
    pub(crate) arc_segment_tests: usize,
    pub(crate) arc_membership_queries: usize,
    pub(crate) maximum_intersection_angles: usize,
}

impl IsotropicArcBudget {
    pub(crate) fn new(limits: IsotropicSpatialLimits, window: &ObservationWindow2D) -> Self {
        Self {
            limits,
            segment_count: window.translation_segment_count(),
            pair_visits: 0,
            visible_arc_evaluations: 0,
            arc_segment_tests: 0,
            arc_membership_queries: 0,
            maximum_intersection_angles: 0,
        }
    }

    pub(crate) fn charge_pair(&mut self) -> Result<(), IsotropicSpatialError> {
        self.pair_visits = self
            .pair_visits
            .checked_add(1)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if self.pair_visits > self.limits.maximum_pair_visits {
            return Err(IsotropicSpatialError::PairVisitLimitExceeded {
                observed: self.pair_visits,
                maximum: self.limits.maximum_pair_visits,
            });
        }
        Ok(())
    }

    pub(crate) fn visible_fraction(
        &mut self,
        window: &ObservationWindow2D,
        center_x: f64,
        center_y: f64,
        radius: f64,
        center: usize,
        neighbor: usize,
    ) -> Result<f64, IsotropicSpatialError> {
        self.visible_arc_evaluations = self
            .visible_arc_evaluations
            .checked_add(1)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if self.visible_arc_evaluations > self.limits.maximum_visible_arc_evaluations {
            return Err(IsotropicSpatialError::VisibleArcEvaluationLimitExceeded {
                observed: self.visible_arc_evaluations,
                maximum: self.limits.maximum_visible_arc_evaluations,
            });
        }
        self.arc_segment_tests = self
            .arc_segment_tests
            .checked_add(self.segment_count)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        if self.arc_segment_tests > self.limits.maximum_arc_segment_tests {
            return Err(IsotropicSpatialError::ArcSegmentTestLimitExceeded {
                required: self.arc_segment_tests,
                maximum: self.limits.maximum_arc_segment_tests,
            });
        }
        let remaining_queries = self
            .limits
            .maximum_arc_membership_queries
            .saturating_sub(self.arc_membership_queries);
        let arc = window
            .visible_circle_arc_fraction(center_x, center_y, radius, remaining_queries)
            .map_err(|error| match error {
                ObservationWindowError::VisibleArcMembershipQueryLimitExceeded { .. } => {
                    IsotropicSpatialError::ArcMembershipQueryLimitExceeded {
                        maximum: self.limits.maximum_arc_membership_queries,
                    }
                }
                other => IsotropicSpatialError::Window(other),
            })?;
        self.arc_membership_queries = self
            .arc_membership_queries
            .checked_add(arc.membership_queries)
            .ok_or(IsotropicSpatialError::SizeOverflow)?;
        self.maximum_intersection_angles = self
            .maximum_intersection_angles
            .max(arc.boundary_intersection_angles);
        if arc.fraction <= 0.0 {
            return Err(IsotropicSpatialError::NonPositiveVisibleArc { center, neighbor });
        }
        Ok(arc.fraction)
    }
}
