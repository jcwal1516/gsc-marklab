use crate::{ObservationWindow2D, TranslationSpatialError, TranslationSpatialLimits};

/// Shared exact polygon-translation work accounting for the immediate K/L and g callers.
pub(crate) struct TranslationOverlapBudget {
    limits: TranslationSpatialLimits,
    candidate_work_per_overlap: usize,
    pub(crate) pair_visits: usize,
    pub(crate) overlap_evaluations: usize,
    pub(crate) overlap_candidate_work: usize,
    pub(crate) maximum_output_vertices: usize,
}

impl TranslationOverlapBudget {
    pub(crate) fn new(
        limits: TranslationSpatialLimits,
        window: &ObservationWindow2D,
    ) -> Result<Self, TranslationSpatialError> {
        let segments = window.translation_segment_count();
        let candidate_work_per_overlap = segments
            .checked_mul(segments)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        Ok(Self {
            limits,
            candidate_work_per_overlap,
            pair_visits: 0,
            overlap_evaluations: 0,
            overlap_candidate_work: 0,
            maximum_output_vertices: 0,
        })
    }

    pub(crate) fn charge_pair(&mut self) -> Result<(), TranslationSpatialError> {
        self.pair_visits = self
            .pair_visits
            .checked_add(1)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if self.pair_visits > self.limits.maximum_pair_visits {
            return Err(TranslationSpatialError::PairVisitLimitExceeded {
                observed: self.pair_visits,
                maximum: self.limits.maximum_pair_visits,
            });
        }
        Ok(())
    }

    pub(crate) fn overlap(
        &mut self,
        window: &ObservationWindow2D,
        displacement_x: f64,
        displacement_y: f64,
        left: usize,
        right: usize,
    ) -> Result<f64, TranslationSpatialError> {
        self.overlap_evaluations = self
            .overlap_evaluations
            .checked_add(1)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if self.overlap_evaluations > self.limits.maximum_overlap_evaluations {
            return Err(TranslationSpatialError::OverlapEvaluationLimitExceeded {
                observed: self.overlap_evaluations,
                maximum: self.limits.maximum_overlap_evaluations,
            });
        }
        self.overlap_candidate_work = self
            .overlap_candidate_work
            .checked_add(self.candidate_work_per_overlap)
            .ok_or(TranslationSpatialError::SizeOverflow)?;
        if self.overlap_candidate_work > self.limits.maximum_overlap_candidate_work {
            return Err(TranslationSpatialError::OverlapCandidateWorkLimitExceeded {
                required: self.overlap_candidate_work,
                maximum: self.limits.maximum_overlap_candidate_work,
            });
        }
        let overlap = window.translation_overlap_area_um2(
            displacement_x,
            displacement_y,
            self.limits.maximum_overlap_output_vertices,
        )?;
        self.maximum_output_vertices = self.maximum_output_vertices.max(overlap.output_vertices);
        if overlap.area_um2 <= 0.0 {
            return Err(TranslationSpatialError::NonPositiveOverlap { left, right });
        }
        Ok(overlap.area_um2)
    }
}

pub(crate) fn preflight_overlap_complexity(
    window: &ObservationWindow2D,
    limits: TranslationSpatialLimits,
) -> Result<(), TranslationSpatialError> {
    let segments = window.translation_segment_count();
    let maximum_output = segments
        .checked_mul(segments)
        .and_then(|value| value.checked_add(segments.checked_mul(2)?))
        .ok_or(TranslationSpatialError::SizeOverflow)?;
    if maximum_output > limits.maximum_overlap_output_vertices {
        return Err(TranslationSpatialError::InvalidConfig {
            reason: format!(
                "translation Boolean output bound {maximum_output} exceeds maximum {}",
                limits.maximum_overlap_output_vertices
            ),
        });
    }
    Ok(())
}
