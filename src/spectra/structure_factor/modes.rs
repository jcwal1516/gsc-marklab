use crate::{
    common::stats::min_max_ignoring_nonfinite,
    data::Pattern,
    geom::spatial_index::mean_nearest_neighbor_distance,
    spectra::kgrid::{resolvable_k_modes, KBand, KMode},
};

pub fn resolvable_modes_for_pattern(pattern: &Pattern, n_shells: usize) -> Option<Vec<KMode>> {
    let band = resolvable_band(pattern)?;
    let modes = resolvable_k_modes(band, n_shells);
    (!modes.is_empty()).then_some(modes)
}

fn effective_length_um(pattern: &Pattern) -> Option<f64> {
    if pattern.window.l_eff_um.is_finite() && pattern.window.l_eff_um > 0.0 {
        return Some(pattern.window.l_eff_um);
    }
    let (min_x, max_x) = min_max_ignoring_nonfinite(&pattern.x_um)?;
    let (min_y, max_y) = min_max_ignoring_nonfinite(&pattern.y_um)?;
    let span = (max_x - min_x).max(max_y - min_y);
    (span > 0.0).then_some(span)
}

fn resolvable_band(pattern: &Pattern) -> Option<KBand> {
    let l_eff_um = effective_length_um(pattern)?;
    let d_nn_mean_um =
        if pattern.window.d_nn_mean_um.is_finite() && pattern.window.d_nn_mean_um > 0.0 {
            pattern.window.d_nn_mean_um
        } else {
            mean_nearest_neighbor_distance(&pattern.x_um, &pattern.y_um)?
        };
    KBand::from_window(l_eff_um, d_nn_mean_um)
}
