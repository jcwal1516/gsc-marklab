use crate::{
    data::PatternMeta,
    spectra::{
        kgrid::{resolvable_k_modes, KBand},
        mark_pair_covariance::{
            mark_pair_covariance, MarkPairCovarianceBin, MarkPairCovariancePlan,
        },
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
fn mark_pair_covariance_averages_centered_mark_products_by_distance_bin() {
    let pattern =
        Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 0.0], vec![1, 0], meta()).expect("pattern");

    let bins = mark_pair_covariance(&pattern, 1.0, 2.0).expect("mark-pair covariance");

    assert_eq!(bins.len(), 2);
    assert_eq!(bins[1].count, 1);
    assert_abs_diff_eq!(bins[1].value.expect("observed bin"), -0.25, epsilon = 1e-12);
}

#[test]
fn remediation_mark_pair_covariance_does_not_report_empty_bins_as_observed_zero() {
    let pattern =
        Pattern::from_arrays(vec![0.0, 1.0], vec![0.0, 0.0], vec![1, 0], meta()).expect("pattern");

    let bins = mark_pair_covariance(&pattern, 1.0, 2.0).expect("mark-pair covariance");

    assert_eq!(bins[0].count, 0);
    assert_eq!(bins[0].value, None);
    assert_eq!(bins[1].count, 1);
    assert_eq!(bins[1].value, Some(-0.25));
}

#[test]
fn pair_plan_matches_bruteforce() {
    let x = (0..53)
        .map(|index| ((index * 17) % 31) as f64 - 11.0)
        .collect::<Vec<_>>();
    let y = (0..53)
        .map(|index| ((index * 29) % 37) as f64 - 13.0)
        .collect::<Vec<_>>();
    let marks = (0..53).map(|index| u8::from(index % 4 == 0)).collect();
    let pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    let plan = MarkPairCovariancePlan::new(&pattern, 1.75, 9.0).expect("pair plan");

    for marks in [
        pattern.mark.to_vec(),
        pattern.mark.iter().copied().rev().collect(),
        (0..pattern.len())
            .map(|index| u8::from(index % 3 == 0))
            .collect(),
    ] {
        assert_eq!(
            plan.evaluate(&marks).expect("planned covariance"),
            brute_force_mark_pair_covariance(&pattern, &marks, 1.75, 9.0)
                .expect("brute-force covariance")
        );
    }
}

fn brute_force_mark_pair_covariance(
    pattern: &Pattern,
    marks: &[u8],
    bin_width_um: f64,
    max_r_um: f64,
) -> Option<Vec<MarkPairCovarianceBin>> {
    if pattern.len() < 2 || marks.len() != pattern.len() {
        return None;
    }
    let n_bins = (max_r_um / bin_width_um).ceil() as usize;
    let mut sums = vec![0.0; n_bins];
    let mut counts = vec![0usize; n_bins];
    let p_hat = marks.iter().filter(|mark| **mark == 1).count() as f64 / marks.len() as f64;
    let centered = marks
        .iter()
        .map(|mark| f64::from(*mark) - p_hat)
        .collect::<Vec<_>>();

    for source in 0..pattern.len() {
        for target in (source + 1)..pattern.len() {
            let distance = (pattern.x_um[target] - pattern.x_um[source])
                .hypot(pattern.y_um[target] - pattern.y_um[source]);
            if distance >= max_r_um {
                continue;
            }
            let bin = (distance / bin_width_um).floor() as usize;
            sums[bin] += centered[source] * centered[target];
            counts[bin] += 1;
        }
    }

    Some(
        sums.into_iter()
            .zip(counts)
            .enumerate()
            .map(|(index, (sum, count))| MarkPairCovarianceBin {
                r_min_um: index as f64 * bin_width_um,
                r_max_um: (index + 1) as f64 * bin_width_um,
                value: (count > 0).then_some(sum / count as f64),
                count,
            })
            .collect(),
    )
}
