#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateTerritory {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub scale_um: f64,
    pub z_or_power: f64,
    pub supporting_cells: usize,
    pub component_id: Option<u32>,
    pub qc_overlap_fraction: f64,
}

use crate::{
    data::Pattern,
    wavelet::{dog::territory_radius_from_scale, residual_field::standardized_residual},
};

pub fn detect_residual_territories(pattern: &Pattern, min_z: f64) -> Vec<CandidateTerritory> {
    if pattern.is_empty() || pattern.n_marked() == 0 || pattern.n_unmarked() == 0 {
        return Vec::new();
    }

    let scales = territory_scales(pattern);
    let mut candidates = Vec::new();
    for scale_um in scales {
        let radius_um = territory_radius_from_scale(scale_um);
        for index in 0..pattern.len() {
            if pattern.mark[index] != 1 {
                continue;
            }
            if let Some(candidate) = candidate_at(pattern, index, scale_um, radius_um, min_z) {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(|left, right| {
        right
            .z_or_power
            .total_cmp(&left.z_or_power)
            .then_with(|| right.supporting_cells.cmp(&left.supporting_cells))
    });

    let mut selected: Vec<CandidateTerritory> = Vec::new();
    for candidate in candidates {
        let overlaps_existing = selected.iter().any(|existing| {
            let dx = existing.center_x_um - candidate.center_x_um;
            let dy = existing.center_y_um - candidate.center_y_um;
            let distance = (dx * dx + dy * dy).sqrt();
            distance <= existing.radius_um.min(candidate.radius_um)
        });
        if !overlaps_existing {
            selected.push(candidate);
        }
    }

    selected
}

fn territory_scales(pattern: &Pattern) -> Vec<f64> {
    let d_nn = pattern.window.d_nn_mean_um.max(1.0);
    let coarse = (pattern.window.l_eff_um.max(d_nn) / 8.0).max(d_nn);
    let mut scales = vec![d_nn, d_nn * 2.0, coarse];
    scales.sort_by(f64::total_cmp);
    scales.dedup_by(|left, right| (*left - *right).abs() < 1.0e-9);
    scales
}

fn candidate_at(
    pattern: &Pattern,
    index: usize,
    scale_um: f64,
    radius_um: f64,
    min_z: f64,
) -> Option<CandidateTerritory> {
    let center_x = pattern.x_um[index];
    let center_y = pattern.y_um[index];
    let radius2 = radius_um * radius_um;
    let mut n_eff = 0usize;
    let mut marked = 0usize;
    let mut marked_x = 0.0;
    let mut marked_y = 0.0;
    let mut component_id = None;

    for cell_index in 0..pattern.len() {
        let dx = pattern.x_um[cell_index] - center_x;
        let dy = pattern.y_um[cell_index] - center_y;
        if dx * dx + dy * dy <= radius2 {
            n_eff += 1;
            if pattern.mark[cell_index] == 1 {
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
    }

    if marked == 0 || n_eff == 0 {
        return None;
    }

    let local_p = marked as f64 / n_eff as f64;
    let z = standardized_residual(local_p, pattern.p_hat(), n_eff as f64);
    if z < min_z {
        return None;
    }

    Some(CandidateTerritory {
        center_x_um: marked_x / marked as f64,
        center_y_um: marked_y / marked as f64,
        radius_um,
        scale_um,
        z_or_power: z,
        supporting_cells: marked,
        component_id,
        qc_overlap_fraction: 0.0,
    })
}
