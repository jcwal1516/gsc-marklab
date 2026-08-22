use crate::{
    comparison::{
        curves::max_abs_standardized_difference, margin_assessment::curve_margin_assessment,
        pooled_bin_difference::pooled_bin_difference_diagnostic,
    },
    geom::spatial_index::SpatialIndex2D,
    multimodal::{
        cells::{CellSection, FusedCell},
        labels::PrimaryLabelEncoding,
    },
    neighborhood::{
        cross_curves::{cross_interaction_curve, cross_interaction_curves_with_index},
        enrichment::LabelPair,
    },
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
fn cross_curve_empty_geometric_bins_are_unavailable() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 1.0, 0.0, "lymphocyte"),
    ];

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "lymphocyte",
        2.0,
        6.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("curve");

    assert_eq!(curve.points[0].value, Some(1.0));
    assert!(curve.points[0].inference_eligible);
    for empty in &curve.points[1..] {
        assert_eq!(empty.value, None);
        assert!(!empty.inference_eligible);
        assert_eq!(empty.lower_global_envelope, None);
        assert_eq!(empty.upper_global_envelope, None);
    }
    let json = serde_json::to_string(&curve).expect("serialize sparse cross curve");
    let roundtrip = serde_json::from_str(&json).expect("deserialize sparse cross curve");
    assert_eq!(curve, roundtrip);
}

#[test]
fn cross_interaction_plan_matches_bruteforce() {
    let cells = (0..24)
        .map(|index| {
            cell(
                &format!("cell-{index}"),
                (index % 6) as f64,
                (index / 6) as f64,
                if index % 3 == 0 {
                    "mmr_abnormal"
                } else {
                    "lymphocyte"
                },
            )
        })
        .collect::<Vec<_>>();
    let bin_width_um = 0.75;
    let max_r_um = 3.5;

    let curve = cross_interaction_curve(
        &cells,
        "mmr_abnormal",
        "lymphocyte",
        bin_width_um,
        max_r_um,
        TEST_PERMUTATIONS,
        TEST_SEED,
    )
    .expect("planned curve");
    let mut brute_counts = vec![0usize; curve.points.len()];
    let mut brute_geometric_counts = vec![0usize; curve.points.len()];
    for source in 0..cells.len() {
        for target in (source + 1)..cells.len() {
            let distance = (cells[target].x_um_registered - cells[source].x_um_registered)
                .hypot(cells[target].y_um_registered - cells[source].y_um_registered);
            if distance >= max_r_um {
                continue;
            }
            let bin = (distance / bin_width_um).floor() as usize;
            brute_geometric_counts[bin] += 1;
            let left = cells[source].cell_type.as_deref();
            let right = cells[target].cell_type.as_deref();
            if matches!(
                (left, right),
                (Some("mmr_abnormal"), Some("lymphocyte"))
                    | (Some("lymphocyte"), Some("mmr_abnormal"))
            ) {
                brute_counts[bin] += 1;
            }
        }
    }

    assert_eq!(
        curve
            .points
            .iter()
            .map(|point| point.count)
            .collect::<Vec<_>>(),
        brute_counts
    );
    assert_eq!(
        curve
            .points
            .iter()
            .map(|point| point.inference_eligible)
            .collect::<Vec<_>>(),
        brute_geometric_counts
            .iter()
            .map(|count| *count > 0)
            .collect::<Vec<_>>()
    );
}

#[test]
fn cross_curve_plan_rejects_output_sensitive_storage_over_budget() {
    let cells = (0..32)
        .map(|index| {
            cell(
                &format!("cell-{index}"),
                index as f64 * 0.01,
                0.0,
                if index % 2 == 0 {
                    "mmr_abnormal"
                } else {
                    "lymphocyte"
                },
            )
        })
        .collect::<Vec<_>>();
    let labels = PrimaryLabelEncoding::new(&cells).expect("labels");
    let index = SpatialIndex2D::from_points(
        cells
            .iter()
            .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
    )
    .expect("index");

    let error = cross_interaction_curves_with_index(
        &cells,
        &index,
        &labels,
        &[LabelPair::new("mmr_abnormal", "lymphocyte")],
        1.0,
        5.0,
        TEST_PERMUTATIONS,
        TEST_SEED,
        0.05,
        512,
    )
    .expect_err("cross-interaction plan must honor its storage budget");

    assert!(error.to_string().contains("cross-interaction"));
    assert!(error.to_string().contains("512 bytes"));
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
        if point.inference_eligible {
            matches!(
                (point.lower_global_envelope, point.upper_global_envelope),
                (Some(lower), Some(upper)) if lower <= upper
            )
        } else {
            point.value.is_none()
                && point.lower_global_envelope.is_none()
                && point.upper_global_envelope.is_none()
        }
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
fn margin_assessment_requires_margin_and_reports_result() {
    let a = [1.0, 1.1, 0.9];
    let b = [1.02, 1.05, 0.91];
    let result =
        curve_margin_assessment("small_diff", &a, &b, Some(0.2)).expect("margin assessment");
    assert_eq!(result.within_margin, Some(true));
    assert_eq!(result.margin, Some(0.2));

    let no_margin = curve_margin_assessment("no_margin", &a, &b, None).expect("diagnostic");
    assert_eq!(no_margin.within_margin, None);
    assert!(no_margin.interpretation.contains("unavailable"));
}

#[test]
fn comparison_apis_reject_non_finite_curve_values() {
    let a = [1.0, f64::NAN];
    let b = [1.0, 2.0];

    assert!(max_abs_standardized_difference(&a, &b).is_err());
    assert!(pooled_bin_difference_diagnostic("nan", &a, &b, 19, 123).is_err());
    assert!(curve_margin_assessment("nan", &a, &b, Some(0.1)).is_err());
}

#[test]
fn pooled_bin_difference_rejects_zero_permutations() {
    let a = [1.0, 1.1];
    let b = [1.0, 1.2];

    assert!(pooled_bin_difference_diagnostic("zero", &a, &b, 0, 123).is_err());
}

#[test]
fn margin_assessment_rejects_invalid_margins_and_accepts_zero_margin() {
    let a = [1.0, 1.1];
    let b = [1.0, 1.1];

    assert!(curve_margin_assessment("negative", &a, &b, Some(-0.1)).is_err());
    assert!(curve_margin_assessment("nan", &a, &b, Some(f64::NAN)).is_err());
    let exact =
        curve_margin_assessment("exact", &a, &b, Some(0.0)).expect("exact margin assessment");
    assert_eq!(exact.within_margin, Some(true));
}

#[test]
fn pooled_bin_difference_is_deterministic_for_same_seed() {
    let a = [1.0, 1.0, 1.0];
    let b = [2.0, 1.0, 0.0];

    let first = pooled_bin_difference_diagnostic("changed", &a, &b, 19, 123).expect("first");
    let second = pooled_bin_difference_diagnostic("changed", &a, &b, 19, 123).expect("second");

    assert_eq!(first, second);
}

#[test]
fn pooled_bin_difference_reports_nonzero_statistic() {
    let a = [1.0, 1.0, 1.0];
    let b = [2.0, 1.0, 0.0];
    let result = pooled_bin_difference_diagnostic("changed", &a, &b, 19, 123).expect("difference");
    assert!(result.statistic.is_some_and(|statistic| statistic > 0.0));
    assert!(result.pooled_bin_p_value.is_some());
}
