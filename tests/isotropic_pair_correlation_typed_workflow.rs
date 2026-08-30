use approx::assert_abs_diff_eq;
use marklab::{
    analyze_isotropic_pair_correlation, IsotropicPairCorrelationConfig, IsotropicSpatialError,
    IsotropicSpatialLimits, ObservationWindow2D, ObservationWindowLimits,
    PairCorrelationPointStatus, Pattern, PatternMeta,
};

fn pattern(x: Vec<f64>, y: Vec<f64>) -> Pattern {
    Pattern::from_arrays(
        x.clone(),
        y,
        vec![0; x.len()],
        PatternMeta {
            case_id: "isotropic-g-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("isotropic-g-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn rectangle() -> ObservationWindow2D {
    window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#)
}

fn window(geojson: &str) -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        geojson,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn limits() -> IsotropicSpatialLimits {
    bounded_limits(1_000, 2_000, 200_000, 100_000, 1 << 20)
}

fn bounded_limits(
    pair: usize,
    arcs: usize,
    segments: usize,
    queries: usize,
    bytes: usize,
) -> IsotropicSpatialLimits {
    IsotropicSpatialLimits::new(16, 16, pair, arcs, segments, queries, 100_000, bytes)
        .expect("limits")
}

fn config_with_limits(
    radius: f64,
    bandwidth: f64,
    limits: IsotropicSpatialLimits,
) -> IsotropicPairCorrelationConfig {
    IsotropicPairCorrelationConfig::new(vec![radius], bandwidth, 19, 20260829, 0.05, limits)
        .expect("config")
}

#[test]
fn hole_fractions_and_normalized_g_match_the_analytic_oracle() {
    let donut = window(
        r#"{"type":"MultiPolygon","coordinates":[[
        [[0,0],[10,0],[10,10],[0,10],[0,0]],
        [[4,4],[4,6],[6,6],[6,4],[4,4]]]]}"#,
    );
    let result = analyze_isotropic_pair_correlation(
        &pattern(vec![3.0, 3.0], vec![5.0, 7.0]),
        &donut,
        &config_with_limits(2.0, 0.5, limits()),
    )
    .expect("donut g");
    let point = &result.curve[0];
    assert_abs_diff_eq!(point.visible_arc_fraction_sum, 7.0 / 4.0, epsilon = 1e-12);
    assert_abs_diff_eq!(
        point.inverse_visible_arc_weighted_kernel_sum,
        189.0 / 55.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.g.unwrap(),
        2_268.0 / (55.0 * std::f64::consts::PI),
        epsilon = 1e-12
    );
}

#[test]
fn compact_support_and_exact_arc_work_memory_boundaries_are_explicit() {
    let window = rectangle();
    let endpoint = analyze_isotropic_pair_correlation(
        &pattern(vec![1.0, 2.5], vec![5.0, 5.0]),
        &window,
        &config_with_limits(1.0, 0.5, limits()),
    )
    .expect("endpoint");
    assert_eq!(
        endpoint.curve[0].status,
        PairCorrelationPointStatus::NoPairsInKernelSupport
    );
    assert_eq!(endpoint.geometry.observed_visible_arc_evaluations, 0);

    let points = pattern(vec![2.0, 3.0], vec![2.0, 2.0]);
    let exact = analyze_isotropic_pair_correlation(
        &points,
        &window,
        &config_with_limits(20.0, 19.999, bounded_limits(20, 40, 160, 1_000, 1 << 20)),
    )
    .expect("exact work");
    assert_eq!(exact.geometry.total_pair_visits, 20);
    assert_eq!(exact.geometry.total_visible_arc_evaluations, 40);
    assert_eq!(exact.geometry.total_arc_segment_tests, 160);
    let queries = exact.geometry.total_arc_membership_queries;
    assert!(matches!(
        analyze_isotropic_pair_correlation(
            &points,
            &window,
            &config_with_limits(20.0, 19.999, bounded_limits(19, 40, 160, 1_000, 1 << 20))
        ),
        Err(IsotropicSpatialError::PairVisitLimitExceeded {
            observed: 20,
            maximum: 19
        })
    ));
    assert!(matches!(
        analyze_isotropic_pair_correlation(
            &points,
            &window,
            &config_with_limits(20.0, 19.999, bounded_limits(20, 39, 160, 1_000, 1 << 20))
        ),
        Err(IsotropicSpatialError::VisibleArcEvaluationLimitExceeded {
            observed: 40,
            maximum: 39
        })
    ));
    assert!(matches!(
        analyze_isotropic_pair_correlation(
            &points,
            &window,
            &config_with_limits(20.0, 19.999, bounded_limits(20, 40, 159, 1_000, 1 << 20))
        ),
        Err(IsotropicSpatialError::ArcSegmentTestLimitExceeded {
            required: 160,
            maximum: 159
        })
    ));
    assert!(
        matches!(analyze_isotropic_pair_correlation(&points, &window,
        &config_with_limits(20.0, 19.999, bounded_limits(20, 40, 160, queries - 1, 1 << 20))),
        Err(IsotropicSpatialError::ArcMembershipQueryLimitExceeded { maximum }) if maximum + 1 == queries)
    );
    let required = exact.geometry.estimated_storage_bytes;
    assert!(
        matches!(analyze_isotropic_pair_correlation(&points, &window,
        &config_with_limits(20.0, 19.999, bounded_limits(20, 40, 160, 1_000, required - 1))),
        Err(IsotropicSpatialError::RetainedByteLimitExceeded { required: observed, maximum })
            if observed == required && maximum + 1 == required)
    );
}

#[test]
fn rectangle_isotropic_g_matches_visible_arc_hand_oracle() {
    let config = IsotropicPairCorrelationConfig::new(vec![2.0], 0.5, 19, 20260829, 0.05, limits())
        .expect("config");
    let result = analyze_isotropic_pair_correlation(
        &pattern(vec![1.0, 3.0], vec![5.0, 5.0]),
        &rectangle(),
        &config,
    )
    .expect("isotropic g");

    assert_eq!(result.correction, "isotropic");
    let point = &result.curve[0];
    assert_eq!(point.directed_pairs_in_support, 2);
    assert_eq!(point.visible_arc_evaluations, 2);
    assert_abs_diff_eq!(point.visible_arc_fraction_sum, 5.0 / 3.0, epsilon = 1e-12);
    assert_abs_diff_eq!(
        point.inverse_visible_arc_weighted_kernel_sum,
        15.0 / 4.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.g.expect("g"),
        375.0 / (8.0 * std::f64::consts::PI),
        epsilon = 1e-12
    );
}
