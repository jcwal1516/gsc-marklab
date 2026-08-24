use approx::assert_abs_diff_eq;
use marklab::{
    analyze_classical_spatial_pattern, ClassicalNullModel, ClassicalRandomizationUnit,
    ClassicalSpatialConfig, ClassicalSpatialError, ClassicalSpatialLimits, ClassicalSpatialStatus,
    KlPointStatus, ObservationWindow2D, ObservationWindowError, ObservationWindowLimits, Pattern,
    PatternMeta,
};

fn window_limits() -> ObservationWindowLimits {
    ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits")
}

fn analysis_limits(maximum_pair_visits: usize) -> ClassicalSpatialLimits {
    ClassicalSpatialLimits::new(16, 16, maximum_pair_visits, 100_000, 1 << 20)
        .expect("analysis limits")
}

fn pattern(x: Vec<f64>, y: Vec<f64>) -> Pattern {
    Pattern::from_arrays(
        x.clone(),
        y,
        vec![0; x.len()],
        PatternMeta {
            case_id: "classical-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("classical-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn rectangle_window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        window_limits(),
    )
    .expect("rectangle")
}

#[test]
fn rectangle_border_kl_matches_hand_oracle_and_declares_csr_unit() {
    let window = rectangle_window();
    let pattern = pattern(vec![2.0, 4.0, 8.0], vec![2.0, 2.0, 8.0]);
    let config = ClassicalSpatialConfig::new(vec![0.1, 2.0], 19, 7, 0.05, analysis_limits(100_000))
        .expect("classical config");

    let result =
        analyze_classical_spatial_pattern(&pattern, &window, &config).expect("classical result");

    assert_eq!(result.status, ClassicalSpatialStatus::Available);
    assert_eq!(
        result.null_design.null_model,
        ClassicalNullModel::HomogeneousCsrConditionalOnCount
    );
    assert_eq!(
        result.null_design.randomization_unit,
        ClassicalRandomizationUnit::WholeLocationPattern
    );
    assert_eq!(result.null_design.simulations, 19);
    assert_eq!(result.null_design.seed, 7);
    assert_eq!(result.curve.len(), 2);
    assert_eq!(result.curve[0].status, KlPointStatus::Available);
    assert_eq!(result.curve[0].eligible_centers, 3);
    assert_eq!(result.curve[0].ordered_pairs, 0);
    assert_eq!(result.curve[0].k, Some(0.0));
    assert_eq!(result.curve[0].l, Some(0.0));
    assert_eq!(
        result.curve[0].k.expect("zero K").to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        result.curve[0].l.expect("zero L").to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(result.curve[1].status, KlPointStatus::Available);
    assert_eq!(result.curve[1].eligible_centers, 3);
    assert_eq!(result.curve[1].ordered_pairs, 2);
    assert_abs_diff_eq!(result.curve[1].k.expect("K"), 200.0 / 9.0, epsilon = 1e-12);
    assert_abs_diff_eq!(
        result.curve[1].l.expect("L"),
        ((200.0 / 9.0) / std::f64::consts::PI).sqrt(),
        epsilon = 1e-12
    );
    assert_eq!(result.window.area_um2, 100.0);
    assert_eq!(result.window.coordinate_unit, "micrometre");
    assert_eq!(
        result.window.coordinate_frame,
        "existing_pattern_physical_xy"
    );
    assert_eq!(result.window.membership_policy, "closed_window");
    assert_eq!(result.window.component_count, 1);
    assert_eq!(result.window.hole_count, 0);
    assert_eq!(result.geometry.point_count, 3);
    assert_eq!(
        result.geometry.boundary_distance_owner,
        "observation_window_2d"
    );
    assert_eq!(
        result.geometry.pair_traversal,
        "exact_streaming_ordered_pairs"
    );
    assert_eq!(result.configuration.radius_count, 2);
    assert_eq!(result.configuration.limits, analysis_limits(100_000));
    assert_eq!(result.configuration.logical_digest.len(), 64);
    assert!(result.inference.is_some());
}

#[test]
fn donut_window_has_closed_boundary_semantics_and_rejects_self_intersection() {
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[3,3],[7,3],[7,7],[3,7],[3,3]]
        ]]}"#,
        window_limits(),
    )
    .expect("donut");

    assert_abs_diff_eq!(window.area_um2(), 84.0, epsilon = 1e-12);
    assert_abs_diff_eq!(window.perimeter_um(), 56.0, epsilon = 1e-12);
    assert!(window.contains(2.0, 5.0));
    assert!(!window.contains(5.0, 5.0));
    assert!(window.contains(3.0, 5.0));
    assert_abs_diff_eq!(
        window.boundary_distance_um(2.0, 5.0).expect("distance"),
        1.0,
        epsilon = 1e-12
    );
    assert_eq!(window.descriptor().hole_count, 1);

    let error = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,4],[0,4],[4,0],[0,0]]]]}"#,
        window_limits(),
    )
    .expect_err("bow tie must fail");
    assert!(matches!(
        error,
        ObservationWindowError::InvalidTopology { .. }
    ));
}

#[test]
fn duplicate_points_and_one_short_pair_budget_fail_before_result() {
    let window = rectangle_window();
    let duplicate = pattern(vec![2.0, 2.0], vec![2.0, 2.0]);
    let config = ClassicalSpatialConfig::new(vec![1.0], 19, 11, 0.05, analysis_limits(100_000))
        .expect("config");

    assert!(matches!(
        analyze_classical_spatial_pattern(&duplicate, &window, &config),
        Err(ClassicalSpatialError::DuplicatePoint {
            first_row: 0,
            second_row: 1
        })
    ));

    let signed_zero_duplicate = pattern(vec![0.0, -0.0], vec![5.0, 5.0]);
    assert!(matches!(
        analyze_classical_spatial_pattern(&signed_zero_duplicate, &window, &config),
        Err(ClassicalSpatialError::DuplicatePoint {
            first_row: 0,
            second_row: 1
        })
    ));

    let pattern = pattern(vec![2.0, 4.0, 8.0], vec![2.0, 2.0, 8.0]);
    let one_short = ClassicalSpatialConfig::new(vec![2.0], 19, 11, 0.05, analysis_limits(1))
        .expect("one-short config");
    assert!(matches!(
        analyze_classical_spatial_pattern(&pattern, &window, &one_short),
        Err(ClassicalSpatialError::PairVisitLimitExceeded { maximum: 1, .. })
    ));
}

#[test]
fn invalid_radius_and_null_configurations_are_rejected_before_execution() {
    let limits = analysis_limits(100_000);
    for radii in [
        vec![],
        vec![0.0],
        vec![f64::INFINITY],
        vec![1.0, 1.0],
        vec![2.0, 1.0],
    ] {
        assert!(matches!(
            ClassicalSpatialConfig::new(radii, 19, 61, 0.05, limits),
            Err(ClassicalSpatialError::InvalidConfig { .. })
        ));
    }
    for (simulations, alpha) in [(0, 0.05), (19, 0.0), (19, 1.0), (9, 0.05)] {
        assert!(matches!(
            ClassicalSpatialConfig::new(vec![1.0], simulations, 61, alpha, limits),
            Err(ClassicalSpatialError::InvalidConfig { .. })
        ));
    }
}

#[test]
fn empty_and_singleton_patterns_return_typed_unavailable_results() {
    let window = rectangle_window();
    let config =
        ClassicalSpatialConfig::new(vec![0.5, 1.0], 19, 13, 0.05, analysis_limits(100_000))
            .expect("config");

    for input in [pattern(vec![], vec![]), pattern(vec![5.0], vec![5.0])] {
        let result =
            analyze_classical_spatial_pattern(&input, &window, &config).expect("typed result");
        assert_eq!(result.status, ClassicalSpatialStatus::InsufficientPoints);
        assert_eq!(result.geometry.point_count, input.len());
        assert_eq!(result.null_design.conditioned_point_count, input.len());
        assert_eq!(result.inference, None);
        assert_eq!(result.csr_candidate_draws, 0);
        assert!(result.curve.iter().all(|point| {
            point.status == KlPointStatus::NoEligibleCenters
                && point.k.is_none()
                && point.l.is_none()
        }));
    }
}

#[test]
fn exact_border_counts_match_a_brute_force_rectangle_oracle() {
    let window = rectangle_window();
    let x = vec![1.0, 2.0, 5.0, 7.0, 9.0];
    let y = vec![1.0, 6.0, 5.0, 8.0, 2.0];
    let radii = vec![0.5, 1.0, 2.5, 4.0];
    let input = pattern(x.clone(), y.clone());
    let config =
        ClassicalSpatialConfig::new(radii.clone(), 19, 17, 0.05, analysis_limits(1_000_000))
            .expect("config");

    let result =
        analyze_classical_spatial_pattern(&input, &window, &config).expect("classical result");

    for (curve_point, radius) in result.curve.iter().zip(radii) {
        let mut eligible = 0_usize;
        let mut ordered_pairs = 0_usize;
        for center in 0..x.len() {
            let border = x[center]
                .min(10.0 - x[center])
                .min(y[center])
                .min(10.0 - y[center]);
            if border < radius {
                continue;
            }
            eligible += 1;
            for neighbor in 0..x.len() {
                if center != neighbor
                    && (x[center] - x[neighbor]).hypot(y[center] - y[neighbor]) <= radius
                {
                    ordered_pairs += 1;
                }
            }
        }
        assert_eq!(curve_point.eligible_centers, eligible);
        assert_eq!(curve_point.ordered_pairs, ordered_pairs);
        if eligible == 0 {
            assert_eq!(curve_point.status, KlPointStatus::NoEligibleCenters);
        } else {
            let expected_k = 100.0 * ordered_pairs as f64 / (x.len() * eligible) as f64;
            assert_abs_diff_eq!(curve_point.k.expect("K"), expected_k, epsilon = 1e-12);
        }
    }
}

#[test]
fn point_row_order_does_not_change_the_unmarked_scientific_result() {
    let window = rectangle_window();
    let config =
        ClassicalSpatialConfig::new(vec![0.1, 2.0], 19, 59, 0.05, analysis_limits(100_000))
            .expect("config");
    let forward = pattern(vec![2.0, 4.0, 8.0], vec![2.0, 2.0, 8.0]);
    let reordered = pattern(vec![8.0, 2.0, 4.0], vec![8.0, 2.0, 2.0]);

    assert_eq!(
        analyze_classical_spatial_pattern(&forward, &window, &config).expect("forward"),
        analyze_classical_spatial_pattern(&reordered, &window, &config).expect("reordered")
    );
}

#[test]
fn conditional_csr_is_repeatable_and_honors_draw_and_memory_ceilings() {
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[1,1],[9,1],[9,9],[1,9],[1,1]]
        ]]}"#,
        window_limits(),
    )
    .expect("thin frame");
    let input = pattern(vec![0.5, 5.0, 9.5], vec![0.5, 0.5, 9.5]);
    let config = ClassicalSpatialConfig::new(
        vec![0.1],
        19,
        23,
        0.05,
        ClassicalSpatialLimits::new(16, 16, 1_000_000, 100_000, 1 << 20).expect("limits"),
    )
    .expect("config");

    let first = analyze_classical_spatial_pattern(&input, &window, &config).expect("first");
    let second = analyze_classical_spatial_pattern(&input, &window, &config).expect("second");
    assert_eq!(first, second);
    assert!(first.csr_candidate_draws >= input.len() * config.simulations());

    let draw_limited = ClassicalSpatialConfig::new(
        vec![0.1],
        19,
        23,
        0.05,
        ClassicalSpatialLimits::new(16, 16, 1_000_000, 1, 1 << 20).expect("limits"),
    )
    .expect("draw-limited config");
    assert!(matches!(
        analyze_classical_spatial_pattern(&input, &window, &draw_limited),
        Err(ClassicalSpatialError::CsrDrawLimitExceeded { maximum: 1 })
    ));

    let memory_limited = ClassicalSpatialConfig::new(
        vec![0.1],
        19,
        23,
        0.05,
        ClassicalSpatialLimits::new(16, 16, 1_000_000, 100_000, 1).expect("limits"),
    )
    .expect("memory-limited config");
    assert!(matches!(
        analyze_classical_spatial_pattern(&input, &window, &memory_limited),
        Err(ClassicalSpatialError::RetainedByteLimitExceeded { maximum: 1, .. })
    ));
}

#[test]
fn malformed_windows_and_outside_points_fail_at_their_boundaries() {
    let adjacent_overlap = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[2,0],[4,4],[0,4],[0,0]]]]}"#,
        window_limits(),
    );
    assert!(matches!(
        adjacent_overlap,
        Err(ObservationWindowError::InvalidTopology { .. })
    ));

    let too_many_vertices = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 4, 256).expect("limits"),
    );
    assert_eq!(
        too_many_vertices,
        Err(ObservationWindowError::VertexLimitExceeded {
            observed: 5,
            maximum: 4
        })
    );

    let outside = pattern(vec![5.0, 11.0], vec![5.0, 5.0]);
    let config = ClassicalSpatialConfig::new(vec![0.1], 19, 29, 0.05, analysis_limits(100_000))
        .expect("config");
    assert!(matches!(
        analyze_classical_spatial_pattern(&outside, &rectangle_window(), &config),
        Err(ClassicalSpatialError::PointOutsideWindow { row: 1 })
    ));
}

#[test]
fn concave_and_disconnected_windows_have_exact_geometry_and_canonical_identity() {
    let concave = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[4,1],[1,1],[1,4],[0,4],[0,0]]]]}"#,
        window_limits(),
    )
    .expect("concave window");
    assert_abs_diff_eq!(concave.area_um2(), 7.0, epsilon = 1e-12);
    assert_abs_diff_eq!(concave.perimeter_um(), 16.0, epsilon = 1e-12);
    assert!(concave.contains(0.5, 3.0));
    assert!(!concave.contains(2.0, 2.0));
    assert_abs_diff_eq!(
        concave
            .boundary_distance_um(0.5, 3.0)
            .expect("boundary distance"),
        0.5,
        epsilon = 1e-12
    );

    let forward = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[
            [[[0,0],[1,0],[1,1],[0,1],[0,0]]],
            [[[3,3],[4,3],[4,4],[3,4],[3,3]]]
        ]}"#,
        window_limits(),
    )
    .expect("forward components");
    let reordered_and_reoriented = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[
            [[[4,4],[4,3],[3,3],[3,4],[4,4]]],
            [[[1,1],[1,0],[0,0],[0,1],[1,1]]]
        ]}"#,
        window_limits(),
    )
    .expect("reordered components");
    assert_eq!(forward.area_um2(), 2.0);
    assert_eq!(forward.perimeter_um(), 8.0);
    assert_eq!(forward.descriptor().component_count, 2);
    assert!(forward.contains(0.5, 0.5));
    assert!(forward.contains(3.5, 3.5));
    assert!(!forward.contains(2.0, 2.0));
    assert_eq!(
        forward.descriptor().logical_digest,
        reordered_and_reoriented.descriptor().logical_digest
    );
}

#[test]
fn exact_boundary_and_one_short_point_limit_have_frozen_outcomes() {
    let window = rectangle_window();
    let input = pattern(vec![0.0, 5.0], vec![5.0, 5.0]);
    let config = ClassicalSpatialConfig::new(vec![0.1], 19, 53, 0.05, analysis_limits(100_000))
        .expect("config");
    let result =
        analyze_classical_spatial_pattern(&input, &window, &config).expect("boundary accepted");
    assert_eq!(result.curve[0].eligible_centers, 1);

    let limited = ClassicalSpatialConfig::new(
        vec![0.1],
        19,
        53,
        0.05,
        ClassicalSpatialLimits::new(1, 16, 100_000, 100_000, 1 << 20).expect("limits"),
    )
    .expect("limited config");
    assert!(matches!(
        analyze_classical_spatial_pattern(&input, &window, &limited),
        Err(ClassicalSpatialError::PointLimitExceeded {
            observed: 2,
            maximum: 1
        })
    ));
}

#[test]
fn window_byte_component_ring_and_touching_limits_are_explicit() {
    let rectangle =
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#;
    assert_eq!(
        ObservationWindow2D::from_geojson_str(
            rectangle,
            ObservationWindowLimits::new(rectangle.len() - 1, 4, 8, 64, 256).expect("limits"),
        ),
        Err(ObservationWindowError::InputByteLimitExceeded {
            observed: rectangle.len(),
            maximum: rectangle.len() - 1
        })
    );
    assert!(matches!(
        ObservationWindow2D::from_geojson_str(
            r#"{"type":"MultiPolygon","coordinates":[
                [[[0,0],[1,0],[1,1],[0,1],[0,0]]],
                [[[3,3],[4,3],[4,4],[3,4],[3,3]]]
            ]}"#,
            ObservationWindowLimits::new(4_096, 1, 8, 64, 256).expect("limits")
        ),
        Err(ObservationWindowError::ComponentLimitExceeded {
            observed: 2,
            maximum: 1
        })
    ));
    assert!(matches!(
        ObservationWindow2D::from_geojson_str(
            r#"{"type":"MultiPolygon","coordinates":[[
                [[0,0],[10,0],[10,10],[0,10],[0,0]],
                [[3,3],[7,3],[7,7],[3,7],[3,3]]
            ]]}"#,
            ObservationWindowLimits::new(4_096, 4, 1, 64, 256).expect("limits")
        ),
        Err(ObservationWindowError::RingLimitExceeded {
            observed: 2,
            maximum: 1
        })
    ));
    for invalid in [
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[0,3],[4,3],[4,7],[0,7],[0,3]]
        ]]}"#,
        r#"{"type":"MultiPolygon","coordinates":[
            [[[0,0],[2,0],[2,2],[0,2],[0,0]]],
            [[[2,0],[4,0],[4,2],[2,2],[2,0]]]
        ]}"#,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[2,0],[0,0]]]]}"#,
    ] {
        assert!(matches!(
            ObservationWindow2D::from_geojson_str(invalid, window_limits()),
            Err(ObservationWindowError::InvalidTopology { .. })
        ));
    }
}
