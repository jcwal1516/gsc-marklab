use crate::{
    data::PatternMeta,
    geom::spatial_index::SpatialIndex2D,
    multiscale_residual::{
        energy::relative_scale_energies_from_field,
        territories::{
            detect_residual_territories, neighborhood_radius_from_scale, standardized_residual,
            ResidualTerritoryCandidate, ResidualTerritoryPlan,
        },
    },
    periodogram::{
        fft2::fft2_power_spectrum,
        raster::{centered_mark_raster, RasterSpec},
        tapered::{hann_tapered_raster_periodogram, hann_weight},
    },
    spectra::anisotropy::anisotropy_from_weighted_modes,
    Pattern,
};
use approx::assert_abs_diff_eq;

fn meta() -> PatternMeta {
    PatternMeta {
        case_id: "field".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
        slide_id: None,
        section_id: None,
        stain_batch: None,
        block_id: None,
        region_id: None,
    }
}

#[test]
fn anisotropy_uses_weighted_low_k_power_tensor() {
    let readout =
        anisotropy_from_weighted_modes(&[(1.0, 0.0, 2.0), (0.0, 1.0, 1.0)]).expect("anisotropy");

    assert_abs_diff_eq!(readout.index, 2.0, epsilon = 1e-12);
    assert_abs_diff_eq!(readout.theta_deg.unwrap(), 0.0, epsilon = 1e-12);
}

#[test]
fn anisotropy_is_one_for_isotropic_power() {
    let readout =
        anisotropy_from_weighted_modes(&[(1.0, 0.0, 1.0), (0.0, 1.0, 1.0)]).expect("anisotropy");

    assert_abs_diff_eq!(readout.index, 1.0, epsilon = 1e-12);
}

#[test]
fn anisotropy_returns_finite_large_index_for_one_directional_power() {
    let readout = anisotropy_from_weighted_modes(&[(1.0, 0.0, 2.0)]).expect("anisotropy");

    assert!(readout.index.is_finite());
    assert!(readout.index > 1.0e12);
}

#[test]
fn hann_taper_has_zero_endpoints_and_unit_center_for_odd_lengths() {
    assert_abs_diff_eq!(hann_weight(0, 5), 0.0, epsilon = 1e-12);
    assert_abs_diff_eq!(hann_weight(2, 5), 1.0, epsilon = 1e-12);
    assert_abs_diff_eq!(hann_weight(4, 5), 0.0, epsilon = 1e-12);
}

#[test]
fn fft2_power_spectrum_places_constant_field_power_at_dc() {
    let power = fft2_power_spectrum(&[1.0, 1.0, 1.0, 1.0], 2, 2).expect("fft power");

    assert_eq!(power.len(), 4);
    assert_abs_diff_eq!(power[0], 16.0, epsilon = 1e-9);
    assert_abs_diff_eq!(power[1], 0.0, epsilon = 1e-9);
    assert_abs_diff_eq!(power[2], 0.0, epsilon = 1e-9);
    assert_abs_diff_eq!(power[3], 0.0, epsilon = 1e-9);
}

#[test]
fn fft2_power_spectrum_rejects_bad_shapes() {
    assert!(fft2_power_spectrum(&[1.0, 2.0, 3.0], 2, 2).is_none());
    assert!(fft2_power_spectrum(&[], 0, 2).is_none());
}

#[test]
fn centered_mark_raster_accumulates_centered_labels_into_cells() {
    let pattern =
        Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 0.0], vec![1, 0], meta()).expect("pattern");
    let (spec, raster) = centered_mark_raster(&pattern, 1.0).expect("raster");

    assert_eq!(
        spec,
        RasterSpec {
            width: 2,
            height: 1,
            cell_size_um: 1.0
        }
    );
    assert_eq!(raster, vec![0.5, -0.5]);
}

#[test]
fn hann_tapered_raster_periodogram_reports_finite_low_k_summary() {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    for row in 0..4 {
        for col in 0..4 {
            x.push(col as f64);
            y.push(row as f64);
            marks.push(u8::from(row < 2 && col < 2));
        }
    }
    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.window.d_nn_mean_um = 1.0;

    let summary = hann_tapered_raster_periodogram(&pattern, 1.0, 2).expect("periodogram");

    assert_eq!(summary.raster_width, 4);
    assert_eq!(summary.raster_height, 4);
    assert!(summary.n_modes > 0);
    assert!(summary.n_radial_shells > 0);
    assert!(summary.n_modes > summary.n_radial_shells);
    assert!(summary.low_k_power.is_finite());
    assert!(summary.normalized_low_k_power.is_finite());
}

#[test]
fn tapered_periodogram_groups_all_modes_in_each_radial_shell() {
    let mut x_um = Vec::new();
    let mut y_um = Vec::new();
    let mut marks = Vec::new();
    for y in 0..5 {
        for x in 0..5 {
            x_um.push(x as f64);
            y_um.push(y as f64);
            marks.push(u8::from((x + 2 * y) % 5 <= 1));
        }
    }
    let pattern = Pattern::from_arrays(x_um, y_um, marks, meta()).expect("pattern");
    let summary = hann_tapered_raster_periodogram(&pattern, 1.0, 1).expect("periodogram");

    let (spec, mut raster) = centered_mark_raster(&pattern, 1.0).expect("raster");
    for y in 0..spec.height {
        for x in 0..spec.width {
            raster[y * spec.width + x] *=
                (hann_weight(x, spec.width) * hann_weight(y, spec.height)) as f32;
        }
    }
    let power = fft2_power_spectrum(&raster, spec.width, spec.height).expect("power");
    let shell_width = 1.0 / spec.width.max(spec.height) as f64;
    let mut shells = std::collections::BTreeMap::<usize, (f64, usize)>::new();
    for y in 0..spec.height {
        for x in 0..spec.width {
            if x == 0 && y == 0 {
                continue;
            }
            let fx = x.min(spec.width - x) as f64 / spec.width as f64;
            let fy = y.min(spec.height - y) as f64 / spec.height as f64;
            let shell = (fx.hypot(fy) / shell_width).floor() as usize;
            let entry = shells.entry(shell).or_default();
            entry.0 += power[y * spec.width + x];
            entry.1 += 1;
        }
    }
    let (sum, count) = shells
        .into_values()
        .find(|(_, count)| *count > 0)
        .expect("first nonempty radial shell");

    assert_abs_diff_eq!(summary.low_k_power, sum / count as f64, epsilon = 1e-12);
}

#[test]
fn standardized_residual_uses_binomial_local_variance() {
    let z = standardized_residual(0.75, 0.5, 25.0);

    assert_abs_diff_eq!(z, 2.5, epsilon = 1e-12);
}

#[test]
fn analysis_scale_converts_to_candidate_territory_radius() {
    assert_abs_diff_eq!(
        neighborhood_radius_from_scale(10.0),
        10.0 * 2.0_f64.sqrt(),
        epsilon = 1e-12
    );
}

#[test]
fn residual_territory_detector_keeps_separated_local_maxima() {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    for row in 0..10 {
        for col in 0..10 {
            x.push(col as f64);
            y.push(row as f64);
            marks.push(u8::from((row <= 1 && col <= 1) || (row >= 8 && col >= 8)));
        }
    }
    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.l_eff_um = 10.0;

    let territories = detect_residual_territories(&pattern, 2.0);

    assert!(territories.len() >= 2);
    assert!(territories
        .iter()
        .any(|territory| territory.center_x_um < 3.0 && territory.center_y_um < 3.0));
    assert!(territories
        .iter()
        .any(|territory| territory.center_x_um > 6.0 && territory.center_y_um > 6.0));
    assert!(territories
        .iter()
        .all(|territory| territory.residual_score >= 2.0));
}

#[test]
fn residual_territory_plan_matches_bruteforce() {
    let mut x = Vec::new();
    let mut y = Vec::new();
    for row in 0..6 {
        for col in 0..7 {
            x.push(col as f64 * 1.25 + row as f64 * 0.03);
            y.push(row as f64 * 1.1);
        }
    }
    let first_marks = (0..x.len())
        .map(|index| u8::from(index % 5 <= 1))
        .collect::<Vec<_>>();
    let mut pattern = Pattern::from_arrays(x, y, first_marks.clone(), meta()).expect("pattern");
    pattern.component_id = Some(
        (0..pattern.len())
            .map(|index| (index / 14) as u32)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    pattern.window.d_nn_mean_um = 1.1;
    pattern.window.l_eff_um = 24.0;
    let plan = ResidualTerritoryPlan::new(&pattern).expect("territory plan");
    let mark_assignments = [
        first_marks,
        (0..pattern.len())
            .map(|index| u8::from((index * 3 + 1) % 7 <= 2))
            .collect(),
        (0..pattern.len())
            .map(|index| u8::from(index % 11 == 0 || index % 13 == 0))
            .collect(),
    ];

    for marks in mark_assignments {
        let indexed = plan
            .detect_for_marks(&pattern, &marks, -0.5)
            .expect("indexed territories");
        let brute = brute_force_residual_territories(&pattern, &marks, -0.5);

        assert_eq!(indexed, brute);
    }
}

#[test]
fn residual_territory_plan_rejects_storage_over_budget() {
    let mut pattern = Pattern::from_arrays(
        vec![0.0, 0.1, 0.2, 0.3],
        vec![0.0; 4],
        vec![1, 0, 1, 0],
        meta(),
    )
    .expect("pattern");
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.l_eff_um = 8.0;
    let index = SpatialIndex2D::new(&pattern.x_um, &pattern.y_um).expect("index");

    let error = ResidualTerritoryPlan::new_with_index(&pattern, &index, 3.0, 64)
        .expect_err("territory plan should exceed budget");

    assert!(error.to_string().contains("residual territory plan"));
    assert!(error
        .to_string()
        .contains("remaining geometry memory budget"));
}

fn brute_force_residual_territories(
    pattern: &Pattern,
    marks: &[u8],
    min_z: f64,
) -> Vec<ResidualTerritoryCandidate> {
    if pattern.is_empty()
        || marks.len() != pattern.len()
        || marks.iter().all(|mark| *mark == 0)
        || marks.iter().all(|mark| *mark == 1)
    {
        return Vec::new();
    }

    let p_hat = marks.iter().filter(|mark| **mark == 1).count() as f64 / marks.len() as f64;
    let d_nn = pattern.window.d_nn_mean_um.max(1.0);
    let block_mean_scale = (pattern.window.l_eff_um.max(d_nn) / 8.0).max(d_nn);
    let mut scales = vec![d_nn, d_nn * 2.0, block_mean_scale];
    scales.sort_by(f64::total_cmp);
    scales.dedup_by(|left, right| (*left - *right).abs() < 1.0e-9);

    let mut candidates = Vec::new();
    for scale_um in scales {
        let radius_um = neighborhood_radius_from_scale(scale_um);
        let radius2 = radius_um * radius_um;
        for index in 0..pattern.len() {
            if marks[index] != 1 {
                continue;
            }
            let center_x = pattern.x_um[index];
            let center_y = pattern.y_um[index];
            let mut n_eff = 0usize;
            let mut marked = 0usize;
            let mut marked_x = 0.0;
            let mut marked_y = 0.0;
            let mut component_id = None;
            for (cell_index, mark) in marks.iter().copied().enumerate() {
                let dx = pattern.x_um[cell_index] - center_x;
                let dy = pattern.y_um[cell_index] - center_y;
                if dx * dx + dy * dy <= radius2 {
                    n_eff += 1;
                    if mark == 1 {
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
                continue;
            }
            let z = standardized_residual(marked as f64 / n_eff as f64, p_hat, n_eff as f64);
            if z >= min_z {
                candidates.push(ResidualTerritoryCandidate {
                    center_x_um: marked_x / marked as f64,
                    center_y_um: marked_y / marked as f64,
                    radius_um,
                    analysis_scale_um: scale_um,
                    residual_score: z,
                    supporting_marked_cells: marked,
                    component_id,
                });
            }
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
    selected
}

#[test]
fn relative_scale_energy_emphasizes_local_differences_for_checkerboard() {
    let energies = relative_scale_energies_from_field(&[1.0, -1.0, -1.0, 1.0], 2, 2)
        .expect("relative scale energies");

    assert!(energies.local_difference > energies.block_mean);
    assert_abs_diff_eq!(
        energies.local_difference + energies.residual + energies.block_mean,
        1.0,
        epsilon = 1e-6
    );
}

#[test]
fn relative_scale_energy_emphasizes_block_means_for_broad_gradient() {
    let energies = relative_scale_energies_from_field(
        &[
            -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
        ],
        4,
        4,
    )
    .expect("relative scale energies");

    assert!(energies.block_mean > energies.local_difference);
}
