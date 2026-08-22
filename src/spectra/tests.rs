use crate::{
    data::PatternMeta,
    spectra::{
        kgrid::{resolvable_k_modes, KBand},
        pair_correlation::pair_correlation,
        structure_factor::centered_structure_factor,
    },
    Pattern,
};
use approx::assert_abs_diff_eq;

fn meta() -> PatternMeta {
    PatternMeta {
        case_id: "case_001".into(),
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
fn centered_structure_factor_is_zero_at_zero_frequency_for_balanced_labels() {
    let pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        meta(),
    )
    .expect("pattern");

    assert_abs_diff_eq!(
        centered_structure_factor(&pattern, 0.0, 0.0).expect("zero-frequency power"),
        0.0,
        epsilon = 1e-12
    );
}

#[test]
fn structure_factor_matches_two_point_closed_form() {
    let pattern =
        Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 0.0], vec![1, 0], meta()).expect("pattern");

    assert_abs_diff_eq!(
        centered_structure_factor(&pattern, 0.0, 0.0).expect("s0"),
        0.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        centered_structure_factor(&pattern, std::f64::consts::PI, 0.0).expect("spi"),
        0.5,
        epsilon = 1e-12
    );
}

#[test]
fn kband_uses_window_diameter_and_mean_nearest_neighbor() {
    let band = KBand::from_window(100.0, 5.0).expect("band");

    assert_abs_diff_eq!(
        band.k_min,
        2.0 * std::f64::consts::PI / 100.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        band.k_max,
        2.0 * std::f64::consts::PI / 5.0,
        epsilon = 1e-12
    );
    assert!(KBand::from_window(0.0, 5.0).is_none());
    assert!(KBand::from_window(100.0, 0.0).is_none());
}

#[test]
fn resolvable_k_modes_cover_2d_grid_and_radial_shells() {
    let band = KBand::from_window(40.0, 10.0).expect("band");
    let modes = resolvable_k_modes(band, 3);

    assert!(modes.iter().any(|mode| mode.kx > 0.0 && mode.ky == 0.0));
    assert!(modes.iter().any(|mode| mode.kx == 0.0 && mode.ky > 0.0));
    assert!(modes.iter().any(|mode| mode.kx > 0.0 && mode.ky > 0.0));
    assert!(modes.iter().all(|mode| mode.k >= band.k_min));
    assert!(modes.iter().all(|mode| mode.k <= band.k_max));
    assert!(modes.iter().all(|mode| mode.shell_index < 3));
    assert!(modes.len() > 3);
}

#[test]
fn pair_correlation_averages_centered_mark_products_by_distance_bin() {
    let pattern =
        Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 0.0], vec![1, 0], meta()).expect("pattern");

    let bins = pair_correlation(&pattern, 1.0, 2.0).expect("pair correlation");

    assert_eq!(bins.len(), 2);
    assert_eq!(bins[1].count, 1);
    assert_abs_diff_eq!(bins[1].value, -0.25, epsilon = 1e-12);
}

#[test]
#[ignore = "Phase 0 reproduction: COR-05 empty-bin availability is fixed in Phase 2"]
fn remediation_pair_correlation_does_not_report_empty_bins_as_observed_zero() {
    let pattern =
        Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 0.0], vec![1, 0], meta()).expect("pattern");

    let bins = pair_correlation(&pattern, 1.0, 2.0).expect("pair correlation");

    assert!(
        bins.iter().all(|bin| bin.count > 0),
        "empty bins must be omitted or carry typed unavailability: {bins:?}"
    );
}
