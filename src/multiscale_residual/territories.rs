use crate::{
    data::Pattern,
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multiscale_residual::{
        residual_field::standardized_residual, scale_radius::neighborhood_radius_from_scale,
    },
};

#[cfg(test)]
thread_local! {
    static PLAN_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_plan_build_call_count() {
    PLAN_BUILD_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn plan_build_call_count() -> usize {
    PLAN_BUILD_CALLS.get()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResidualTerritoryCandidate {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub analysis_scale_um: f64,
    pub residual_score: f64,
    pub supporting_marked_cells: usize,
    pub component_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
struct ScaleNeighborhoods {
    scale_um: f64,
    radius_um: f64,
    offsets: Box<[usize]>,
    neighbors: Box<[usize]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResidualTerritoryPlan {
    point_count: usize,
    scales: Box<[ScaleNeighborhoods]>,
}

impl ResidualTerritoryPlan {
    #[cfg(test)]
    pub(crate) fn new(pattern: &Pattern) -> Result<Self> {
        let index = SpatialIndex2D::new(&pattern.x_um, &pattern.y_um)?;
        Self::build(pattern, &index, None)
    }

    pub(crate) fn new_with_index(
        pattern: &Pattern,
        index: &SpatialIndex2D,
        max_scale_um: f64,
    ) -> Result<Self> {
        if !max_scale_um.is_finite() || max_scale_um < 0.0 {
            return Err(MarklabError::Geometry(
                "maximum residual territory scale must be finite and non-negative".into(),
            ));
        }
        Self::build(pattern, index, Some(max_scale_um))
    }

    fn build(pattern: &Pattern, index: &SpatialIndex2D, max_scale_um: Option<f64>) -> Result<Self> {
        #[cfg(test)]
        PLAN_BUILD_CALLS.set(PLAN_BUILD_CALLS.get() + 1);
        if index.len() != pattern.len() {
            return Err(MarklabError::Geometry(format!(
                "residual territory plan has {} points but spatial index has {}",
                pattern.len(),
                index.len()
            )));
        }

        let mut scales = Vec::new();
        for scale_um in territory_scales(pattern)
            .into_iter()
            .filter(|scale_um| max_scale_um.is_none_or(|maximum| *scale_um <= maximum))
        {
            let radius_um = neighborhood_radius_from_scale(scale_um);
            if !scale_um.is_finite() || !radius_um.is_finite() || radius_um < 0.0 {
                return Err(MarklabError::Geometry(
                    "residual territory scales must be finite and non-negative".into(),
                ));
            }
            let mut offsets = Vec::with_capacity(pattern.len().saturating_add(1));
            let mut neighbors = Vec::new();
            offsets.push(0);
            for center in 0..pattern.len() {
                index.visit_within_radius(center, radius_um, |neighbor| {
                    neighbors.push(neighbor.index);
                })?;
                neighbors.push(center);
                let start = offsets.last().copied().unwrap_or(0);
                neighbors[start..].sort_unstable();
                offsets.push(neighbors.len());
            }
            scales.push(ScaleNeighborhoods {
                scale_um,
                radius_um,
                offsets: offsets.into_boxed_slice(),
                neighbors: neighbors.into_boxed_slice(),
            });
        }

        Ok(Self {
            point_count: pattern.len(),
            scales: scales.into_boxed_slice(),
        })
    }

    pub(crate) fn detect_for_marks(
        &self,
        pattern: &Pattern,
        marks: &[u8],
        min_z: f64,
    ) -> Result<Vec<ResidualTerritoryCandidate>> {
        if pattern.len() != self.point_count || marks.len() != self.point_count {
            return Err(MarklabError::Compute(format!(
                "residual territory plan expects {} points but pattern and marks have {} and {}",
                self.point_count,
                pattern.len(),
                marks.len()
            )));
        }
        if marks.iter().any(|mark| *mark != 0 && *mark != 1) {
            return Err(MarklabError::Compute(
                "residual territory marks must be binary".into(),
            ));
        }
        if !min_z.is_finite() {
            return Err(MarklabError::Compute(
                "residual territory threshold must be finite".into(),
            ));
        }
        let n_marked = marks.iter().filter(|mark| **mark == 1).count();
        if marks.is_empty() || n_marked == 0 || n_marked == marks.len() {
            return Ok(Vec::new());
        }

        let p_hat = n_marked as f64 / marks.len() as f64;
        let mut candidates = Vec::new();
        for scale in &self.scales {
            for center in 0..self.point_count {
                if marks[center] != 1 {
                    continue;
                }
                let Some(candidate) = candidate_at(pattern, marks, center, scale, p_hat, min_z)
                else {
                    continue;
                };
                candidates.push(candidate);
            }
        }

        candidates.sort_by(|left, right| {
            right
                .residual_score
                .total_cmp(&left.residual_score)
                .then_with(|| {
                    right
                        .supporting_marked_cells
                        .cmp(&left.supporting_marked_cells)
                })
        });

        let mut selected: Vec<ResidualTerritoryCandidate> = Vec::new();
        for candidate in candidates {
            let overlaps_existing = selected.iter().any(|existing| {
                let distance = (existing.center_x_um - candidate.center_x_um)
                    .hypot(existing.center_y_um - candidate.center_y_um);
                distance <= existing.radius_um.min(candidate.radius_um)
            });
            if !overlaps_existing {
                selected.push(candidate);
            }
        }
        Ok(selected)
    }
}

#[cfg(test)]
pub fn detect_residual_territories(
    pattern: &Pattern,
    min_z: f64,
) -> Vec<ResidualTerritoryCandidate> {
    ResidualTerritoryPlan::new(pattern)
        .and_then(|plan| plan.detect_for_marks(pattern, &pattern.mark, min_z))
        .unwrap_or_default()
}

fn territory_scales(pattern: &Pattern) -> Vec<f64> {
    let d_nn = pattern.window.d_nn_mean_um.max(1.0);
    let block_mean_scale = (pattern.window.l_eff_um.max(d_nn) / 8.0).max(d_nn);
    let mut scales = vec![d_nn, d_nn * 2.0, block_mean_scale];
    scales.sort_by(f64::total_cmp);
    scales.dedup_by(|left, right| (*left - *right).abs() < 1.0e-9);
    scales
}

fn candidate_at(
    pattern: &Pattern,
    marks: &[u8],
    center: usize,
    scale: &ScaleNeighborhoods,
    p_hat: f64,
    min_z: f64,
) -> Option<ResidualTerritoryCandidate> {
    let mut n_eff = 0usize;
    let mut marked = 0usize;
    let mut marked_x = 0.0;
    let mut marked_y = 0.0;
    let mut component_id = None;

    let start = *scale.offsets.get(center)?;
    let end = *scale.offsets.get(center + 1)?;
    for &cell_index in scale.neighbors.get(start..end)? {
        n_eff += 1;
        if marks[cell_index] == 1 {
            marked += 1;
            marked_x += pattern.x_um[cell_index];
            marked_y += pattern.y_um[cell_index];
            if component_id.is_none() {
                component_id = pattern
                    .component_id
                    .as_deref()
                    .and_then(|values| values.get(cell_index).copied());
            }
        }
    }

    if marked == 0 || n_eff == 0 {
        return None;
    }

    let local_p = marked as f64 / n_eff as f64;
    let z = standardized_residual(local_p, p_hat, n_eff as f64);
    if z < min_z {
        return None;
    }

    Some(ResidualTerritoryCandidate {
        center_x_um: marked_x / marked as f64,
        center_y_um: marked_y / marked as f64,
        radius_um: scale.radius_um,
        analysis_scale_um: scale.scale_um,
        residual_score: z,
        supporting_marked_cells: marked,
        component_id,
    })
}
