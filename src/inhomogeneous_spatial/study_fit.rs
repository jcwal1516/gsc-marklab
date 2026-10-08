use super::{
    analysis::Counters,
    intensity::{build_grid, fit_event_intensity, Grid},
    InhomogeneousSpatialConfig, InhomogeneousSpatialError,
};
use crate::{ObservationWindow2D, Pattern};

/// Only the concrete study caller enables refitting; legacy public analyses use default.
#[derive(Default)]
pub(crate) struct StudyFitPolicy {
    pub refit: bool,
    pub bandwidth_candidates_um: Vec<f64>,
}

pub(super) fn select_config(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    base: &InhomogeneousSpatialConfig,
    policy: &StudyFitPolicy,
    counters: &mut Counters,
) -> Result<InhomogeneousSpatialConfig, InhomogeneousSpatialError> {
    if policy.bandwidth_candidates_um.is_empty() {
        return Ok(base.clone());
    }
    select_on_grid(pattern, &build_grid(window, base)?, base, policy, counters)
}

fn select_on_grid(
    pattern: &Pattern,
    grid: &Grid,
    base: &InhomogeneousSpatialConfig,
    policy: &StudyFitPolicy,
    counters: &mut Counters,
) -> Result<InhomogeneousSpatialConfig, InhomogeneousSpatialError> {
    let mut selected = base.clone();
    let mut best = f64::NEG_INFINITY;
    for bandwidth in &policy.bandwidth_candidates_um {
        let mut candidate = base.clone();
        candidate.bandwidth_um = *bandwidth;
        let score = super::bandwidth::score_candidate(pattern, grid, &candidate, counters)?;
        if score.mean_log_leave_one_out_intensity > best {
            best = score.mean_log_leave_one_out_intensity;
            selected = candidate;
        }
    }
    Ok(selected)
}

/// Refit event intensities for one simulated pattern on the observed grid.
///
/// The grid depends only on the window and integration lattice, which every
/// candidate bandwidth shares, so it is built once by the observed fit.
pub(super) fn refit_intensities(
    x: &[f64],
    y: &[f64],
    source: &Pattern,
    grid: &Grid,
    base: &InhomogeneousSpatialConfig,
    policy: &StudyFitPolicy,
    counters: &mut Counters,
) -> Result<Vec<f64>, InhomogeneousSpatialError> {
    let mut simulated = Pattern::from_arrays(
        x.to_vec(),
        y.to_vec(),
        vec![0; x.len()],
        source.meta.clone(),
    )
    .map_err(super::analysis::dependency)?;
    simulated.cell_ids = source.cell_ids.clone();
    let selected = if policy.bandwidth_candidates_um.is_empty() {
        base.clone()
    } else {
        select_on_grid(&simulated, grid, base, policy, counters)?
    };
    Ok(fit_event_intensity(&simulated, grid, &selected, counters)?.observed_intensities)
}
