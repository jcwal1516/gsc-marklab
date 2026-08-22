use crate::{
    data::Pattern,
    geom::{
        length_scales::analysis_effective_length_um, spatial_index::mean_nearest_neighbor_distance,
    },
    spectra::kgrid::{resolvable_k_modes, KBand, KMode},
};

pub fn resolvable_modes_for_pattern(pattern: &Pattern, n_shells: usize) -> Option<Vec<KMode>> {
    let band = resolvable_band(pattern)?;
    let modes = resolvable_k_modes(band, n_shells);
    (!modes.is_empty()).then_some(modes)
}

fn resolvable_band(pattern: &Pattern) -> Option<KBand> {
    let analysis_effective_length_um = analysis_effective_length_um(
        pattern.window.analysis_effective_length_um,
        &pattern.x_um,
        &pattern.y_um,
    )?;
    let d_nn_mean_um =
        if pattern.window.d_nn_mean_um.is_finite() && pattern.window.d_nn_mean_um > 0.0 {
            pattern.window.d_nn_mean_um
        } else {
            mean_nearest_neighbor_distance(&pattern.x_um, &pattern.y_um)?
        };
    KBand::from_window(analysis_effective_length_um, d_nn_mean_um)
}
