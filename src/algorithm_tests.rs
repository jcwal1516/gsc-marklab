use crate::{
    data::PatternMeta,
    periodogram::{
        bartlett::marked_bartlett_periodogram,
        fft2::fft2_power_spectrum,
        raster::{centered_mark_raster, RasterSpec},
        taper::hann_weight,
    },
    spectra::anisotropy::anisotropy_from_weighted_modes,
    wavelet::{
        dog::territory_radius_from_scale, modwt::variance_fractions_from_field,
        residual_field::standardized_residual, territories::detect_residual_territories,
    },
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
fn marked_bartlett_periodogram_reports_finite_low_k_summary() {
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

    let summary = marked_bartlett_periodogram(&pattern, 1.0, 2).expect("periodogram");

    assert_eq!(summary.raster_width, 4);
    assert_eq!(summary.raster_height, 4);
    assert!(summary.n_modes > 0);
    assert!(summary.low_k_power.is_finite());
    assert!(summary.normalized_low_k_power.is_finite());
}

#[test]
fn standardized_residual_uses_binomial_local_variance() {
    let z = standardized_residual(0.75, 0.5, 25.0);

    assert_abs_diff_eq!(z, 2.5, epsilon = 1e-12);
}

#[test]
fn dog_scale_converts_to_candidate_territory_radius() {
    assert_abs_diff_eq!(
        territory_radius_from_scale(10.0),
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
        .all(|territory| territory.z_or_power >= 2.0));
}

#[test]
fn variance_fractions_identify_fine_checkerboard_structure() {
    let fractions =
        variance_fractions_from_field(&[1.0, -1.0, -1.0, 1.0], 2, 2).expect("fractions");

    assert!(fractions.fine > fractions.coarse);
    assert_abs_diff_eq!(
        fractions.fine + fractions.intermediate + fractions.coarse,
        1.0,
        epsilon = 1e-6
    );
}

#[test]
fn variance_fractions_identify_coarse_gradient_structure() {
    let fractions = variance_fractions_from_field(
        &[
            -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
        ],
        4,
        4,
    )
    .expect("fractions");

    assert!(fractions.coarse > fractions.fine);
}
