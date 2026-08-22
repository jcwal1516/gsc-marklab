use crate::{
    comparison::{
        curves::max_abs_standardized_difference, difference::curve_difference_test,
        equivalence::curve_equivalence_test,
    },
    multimodal::cell_table::{CellSection, FusedCell},
    neighborhood::cross_curves::cross_interaction_curve,
};

const TEST_PERMUTATIONS: usize = 99;
const TEST_SEED: u64 = 0x6d6d_7273_7061_6365;

fn cell(id: &str, x: f64, y: f64, label: &str) -> FusedCell {
    FusedCell {
        source_section: CellSection::He,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: None,
        mmr_probability: None,
        cell_type: Some(label.into()),
        cell_type_probability: Some(1.0),
        same_section: false,
        registration_error_um: Some(4.0),
        timepoint: "post".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn ihc_cell(id: &str, x: f64, y: f64, mmr_mark: u8) -> FusedCell {
    FusedCell {
        source_section: CellSection::Ihc,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: Some(mmr_mark),
        mmr_probability: Some(1.0),
        cell_type: None,
        cell_type_probability: None,
        same_section: false,
        registration_error_um: Some(4.0),
        timepoint: "post".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn ihc_probability_cell(id: &str, x: f64, y: f64, mmr_probability: f64) -> FusedCell {
    FusedCell {
        source_section: CellSection::Ihc,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: None,
        mmr_probability: Some(mmr_probability),
        cell_type: None,
        cell_type_probability: None,
        same_section: false,
        registration_error_um: Some(4.0),
        timepoint: "post".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

#[test]
fn cross_curve_counts_label_pairs_by_distance_bin() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 5.0, 0.0, "lymphocyte"),
        cell("c", 25.0, 0.0, "lymphocyte"),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "lymphocyte",
        10.0,
        30.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.label_a, "mmr_abnormal");
    assert_eq!(curve.label_b, "lymphocyte");
    assert_eq!(curve.points.len(), 3);
    assert_eq!(curve.points[0].count, 1);
    assert_eq!(curve.points[2].count, 1);
}

#[test]
fn cross_curve_uses_half_open_bins_and_excludes_max_distance() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 10.0, 0.0, "lymphocyte"),
        cell("c", 22.0, 0.0, "lymphocyte"),
        cell("d", 25.0, 0.0, "lymphocyte"),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "lymphocyte",
        10.0,
        25.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.points.len(), 3);
    assert_eq!(curve.points[0].count, 0);
    assert_eq!(curve.points[1].count, 1);
    assert_eq!(curve.points[2].r_min_um, 20.0);
    assert_eq!(curve.points[2].r_max_um, 25.0);
    assert_eq!(curve.points[2].count, 1);
}

#[test]
fn cross_curve_counts_same_label_pairs_once() {
    let cells = vec![
        cell("a", 0.0, 0.0, "tumor"),
        cell("b", 1.0, 0.0, "tumor"),
        cell("c", 2.0, 0.0, "tumor"),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "tumor",
        "tumor",
        10.0,
        10.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.points.len(), 1);
    assert_eq!(curve.points[0].count, 3);
}

#[test]
fn cross_curve_allows_reversed_label_order() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 5.0, 0.0, "lymphocyte"),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "lymphocyte",
        "mmr_abnormal",
        10.0,
        20.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.label_a, "lymphocyte");
    assert_eq!(curve.label_b, "mmr_abnormal");
    assert_eq!(curve.points[0].count, 1);
}

#[test]
fn cross_curve_maps_ihc_mmr_marks_to_primary_labels() {
    let cells = vec![ihc_cell("a", 0.0, 0.0, 1), ihc_cell("b", 5.0, 0.0, 0)];

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "mmr_retained",
        10.0,
        20.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.points[0].count, 1);
}

#[test]
fn cross_curve_maps_probability_only_ihc_labels() {
    let cells = vec![
        ihc_probability_cell("a", 0.0, 0.0, 0.8),
        ihc_probability_cell("b", 5.0, 0.0, 0.2),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "mmr_retained",
        10.0,
        20.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.points[0].count, 1);
}

#[test]
fn cross_curve_reports_global_permutation_diagnostic() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 5.0, 0.0, "lymphocyte"),
        cell("c", 25.0, 0.0, "lymphocyte"),
        cell("d", 50.0, 0.0, "stroma"),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "lymphocyte",
        10.0,
        30.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert!(curve.p_global.is_some());
    assert!(curve.points.iter().all(|point| {
        matches!(
            (point.lower_global_envelope, point.upper_global_envelope),
            (Some(lower), Some(upper)) if lower <= upper
        )
    }));
}

#[test]
fn cross_curve_rejects_blank_labels() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor")];

    assert!(cross_interaction_curve(
        &cells,
        " ",
        "tumor",
        10.0,
        20.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .is_err());
    assert!(cross_interaction_curve(
        &cells,
        "tumor",
        "\t",
        10.0,
        20.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .is_err());
}

#[test]
fn cross_curve_rejects_non_finite_registered_coordinates() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", f64::NAN, 0.0, "lymphocyte"),
    ];

    assert!(cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "lymphocyte",
        10.0,
        20.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .is_err());
}

#[test]
fn cross_curve_rejects_invalid_bin_width_and_max_distance() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor")];

    for (bin_width, max_distance) in [
        (0.0, 20.0),
        (10.0, 0.0),
        (f64::NAN, 20.0),
        (10.0, f64::INFINITY),
    ] {
        assert!(cross_interaction_curve(
            &cells,
            "tumor",
            "tumor",
            bin_width,
            max_distance,
            TEST_PERMUTATIONS,
            TEST_SEED,
        )
        .is_err());
    }
}

#[test]
fn equivalence_requires_margin_and_reports_result() {
    let a = [1.0, 1.1, 0.9];
    let b = [1.02, 1.05, 0.91];
    let result = curve_equivalence_test("small_diff", &a, &b, Some(0.2)).expect("equivalence");
    assert_eq!(result.equivalent, Some(true));
    assert_eq!(result.equivalence_margin, Some(0.2));

    let no_margin = curve_equivalence_test("no_margin", &a, &b, None).expect("diagnostic");
    assert_eq!(no_margin.equivalent, None);
    assert!(no_margin.interpretation.contains("non-confirmatory"));
}

#[test]
fn comparison_apis_reject_non_finite_curve_values() {
    let a = [1.0, f64::NAN];
    let b = [1.0, 2.0];

    assert!(max_abs_standardized_difference(&a, &b).is_err());
    assert!(curve_difference_test("nan", &a, &b, 19, 123).is_err());
    assert!(curve_equivalence_test("nan", &a, &b, Some(0.1)).is_err());
}

#[test]
fn difference_test_rejects_zero_permutations() {
    let a = [1.0, 1.1];
    let b = [1.0, 1.2];

    assert!(curve_difference_test("zero", &a, &b, 0, 123).is_err());
}

#[test]
fn equivalence_rejects_invalid_margins_and_accepts_zero_margin() {
    let a = [1.0, 1.1];
    let b = [1.0, 1.1];

    assert!(curve_equivalence_test("negative", &a, &b, Some(-0.1)).is_err());
    assert!(curve_equivalence_test("nan", &a, &b, Some(f64::NAN)).is_err());
    let exact = curve_equivalence_test("exact", &a, &b, Some(0.0)).expect("exact equivalence");
    assert_eq!(exact.equivalent, Some(true));
}

#[test]
fn difference_test_is_deterministic_for_same_seed() {
    let a = [1.0, 1.0, 1.0];
    let b = [2.0, 1.0, 0.0];

    let first = curve_difference_test("changed", &a, &b, 19, 123).expect("first");
    let second = curve_difference_test("changed", &a, &b, 19, 123).expect("second");

    assert_eq!(first, second);
}

#[test]
fn difference_test_reports_nonzero_statistic() {
    let a = [1.0, 1.0, 1.0];
    let b = [2.0, 1.0, 0.0];
    let result = curve_difference_test("changed", &a, &b, 19, 123).expect("difference");
    assert!(result.statistic.is_some_and(|statistic| statistic > 0.0));
    assert!(result.p_difference.is_some());
}
