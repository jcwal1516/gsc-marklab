use approx::assert_abs_diff_eq;
use marklab::{
    analyze_isotropic_spatial_pattern, IsotropicSpatialConfig, IsotropicSpatialError,
    IsotropicSpatialLimits, IsotropicSpatialStatus, ObservationWindow2D, ObservationWindowLimits,
    Pattern, PatternMeta,
};

fn window(geojson: &str) -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        geojson,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn pattern(x: [f64; 2], y: [f64; 2]) -> Pattern {
    Pattern::from_arrays(
        x.to_vec(),
        y.to_vec(),
        vec![0, 0],
        PatternMeta {
            case_id: "isotropic-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("isotropic-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn config(radius_um: f64) -> IsotropicSpatialConfig {
    config_with_limits(
        radius_um,
        IsotropicSpatialLimits::new(16, 8, 1_000, 2_000, 200_000, 100_000, 100_000, 1 << 20)
            .expect("isotropic limits"),
    )
}

fn config_with_limits(radius_um: f64, limits: IsotropicSpatialLimits) -> IsotropicSpatialConfig {
    IsotropicSpatialConfig::new(vec![radius_um], 19, 20260829, 0.05, limits)
        .expect("isotropic config")
}

fn limits(
    maximum_pair_visits: usize,
    maximum_visible_arc_evaluations: usize,
    maximum_arc_segment_tests: usize,
    maximum_arc_membership_queries: usize,
    maximum_retained_bytes: usize,
) -> IsotropicSpatialLimits {
    IsotropicSpatialLimits::new(
        16,
        8,
        maximum_pair_visits,
        maximum_visible_arc_evaluations,
        maximum_arc_segment_tests,
        maximum_arc_membership_queries,
        100_000,
        maximum_retained_bytes,
    )
    .expect("limits")
}

#[test]
fn rectangle_and_hole_visible_arc_fractions_match_analytic_oracles() {
    let rectangle =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#);
    let rectangle_result = analyze_isotropic_spatial_pattern(
        &pattern([1.0, 3.0], [5.0, 5.0]),
        &rectangle,
        &config(2.0),
    )
    .expect("rectangle isotropic K/L");

    assert_eq!(rectangle_result.status, IsotropicSpatialStatus::Available);
    assert_eq!(rectangle_result.correction, "isotropic");
    assert_eq!(rectangle_result.curve[0].directed_pairs, 2);
    assert_eq!(rectangle_result.curve[0].visible_arc_evaluations, 2);
    assert_abs_diff_eq!(
        rectangle_result.curve[0].visible_arc_fraction_sum,
        5.0 / 3.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        rectangle_result.curve[0].inverse_visible_arc_fraction_sum,
        5.0 / 2.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        rectangle_result.curve[0].k.expect("rectangle K"),
        125.0,
        epsilon = 1e-12
    );

    let donut = window(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[4,4],[4,6],[6,6],[6,4],[4,4]]
        ]]}"#,
    );
    let donut_result =
        analyze_isotropic_spatial_pattern(&pattern([3.0, 3.0], [5.0, 7.0]), &donut, &config(2.0))
            .expect("donut isotropic K/L");

    // Fractions are 5/6 from (3,5) and 11/12 from (3,7).
    assert_abs_diff_eq!(
        donut_result.curve[0].visible_arc_fraction_sum,
        7.0 / 4.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        donut_result.curve[0].inverse_visible_arc_fraction_sum,
        126.0 / 55.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        donut_result.curve[0].k.expect("donut K"),
        6_048.0 / 55.0,
        epsilon = 1e-12
    );
    assert_eq!(
        donut_result.geometry.visible_arc_owner,
        "observation_window_2d_segment_circle_partition"
    );
}

#[test]
fn boundary_centered_quarter_and_half_circles_have_exact_directed_weights() {
    let rectangle =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#);
    let result = analyze_isotropic_spatial_pattern(
        &pattern([0.0, 1.0], [0.0, 0.0]),
        &rectangle,
        &config(1.0),
    )
    .expect("boundary-centered isotropic K/L");

    assert_abs_diff_eq!(
        result.curve[0].visible_arc_fraction_sum,
        3.0 / 4.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        result.curve[0].inverse_visible_arc_fraction_sum,
        6.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        result.curve[0].k.expect("boundary K"),
        300.0,
        epsilon = 1e-12
    );
}

fn dense_visible_fraction(window: &ObservationWindow2D, center: [f64; 2], radius: f64) -> f64 {
    let samples = 1_000_000_usize;
    let visible = (0..samples)
        .filter(|index| {
            let angle = std::f64::consts::TAU * (*index as f64 + 0.5) / samples as f64;
            window.contains(
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            )
        })
        .count();
    visible as f64 / samples as f64
}

#[test]
fn concave_window_matches_an_independent_dense_angular_oracle() {
    let concave = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[4,1],[1,1],[1,4],[0,4],[0,0]]]]}"#,
    );
    let result =
        analyze_isotropic_spatial_pattern(&pattern([0.5, 1.5], [0.5, 0.5]), &concave, &config(1.0))
            .expect("concave isotropic K/L");
    let left = dense_visible_fraction(&concave, [0.5, 0.5], 1.0);
    let right = dense_visible_fraction(&concave, [1.5, 0.5], 1.0);

    assert_abs_diff_eq!(
        result.curve[0].visible_arc_fraction_sum,
        left + right,
        epsilon = 2e-5
    );
    assert_abs_diff_eq!(
        result.curve[0].inverse_visible_arc_fraction_sum,
        1.0 / left + 1.0 / right,
        epsilon = 5e-5
    );
}

#[test]
fn exact_arc_work_memory_and_zero_visible_measure_boundaries_fail_explicitly() {
    let window =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#);
    let points = pattern([2.0, 3.0], [2.0, 2.0]);
    let exact = analyze_isotropic_spatial_pattern(
        &points,
        &window,
        &config_with_limits(20.0, limits(20, 40, 160, 1_000, 1 << 20)),
    )
    .expect("exact all-pair arc work");
    assert_eq!(exact.geometry.total_pair_visits, 20);
    assert_eq!(exact.geometry.total_visible_arc_evaluations, 40);
    assert_eq!(exact.geometry.total_arc_segment_tests, 160);
    let membership_queries = exact.geometry.total_arc_membership_queries;

    assert!(matches!(
        analyze_isotropic_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(19, 40, 160, 1_000, 1 << 20)),
        ),
        Err(IsotropicSpatialError::PairVisitLimitExceeded {
            observed: 20,
            maximum: 19
        })
    ));
    assert!(matches!(
        analyze_isotropic_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 39, 160, 1_000, 1 << 20)),
        ),
        Err(IsotropicSpatialError::VisibleArcEvaluationLimitExceeded {
            observed: 40,
            maximum: 39
        })
    ));
    assert!(matches!(
        analyze_isotropic_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 40, 159, 1_000, 1 << 20)),
        ),
        Err(IsotropicSpatialError::ArcSegmentTestLimitExceeded {
            required: 160,
            maximum: 159
        })
    ));
    assert!(matches!(
        analyze_isotropic_spatial_pattern(
            &points,
            &window,
            &config_with_limits(
                20.0,
                limits(20, 40, 160, membership_queries - 1, 1 << 20),
            ),
        ),
        Err(IsotropicSpatialError::ArcMembershipQueryLimitExceeded { maximum })
            if maximum + 1 == membership_queries
    ));
    let required = exact.geometry.estimated_storage_bytes;
    assert!(matches!(
        analyze_isotropic_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 40, 160, 1_000, required - 1)),
        ),
        Err(IsotropicSpatialError::RetainedByteLimitExceeded {
            required: observed,
            maximum
        }) if observed == required && maximum + 1 == required
    ));

    let tangent_only = pattern([0.0, 10.0], [0.0, 10.0]);
    assert!(matches!(
        analyze_isotropic_spatial_pattern(
            &tangent_only,
            &window,
            &config_with_limits(15.0, limits(1_000, 2_000, 200_000, 100_000, 1 << 20)),
        ),
        Err(IsotropicSpatialError::NonPositiveVisibleArc {
            center: 0,
            neighbor: 1
        })
    ));
}
