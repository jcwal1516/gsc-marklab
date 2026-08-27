use approx::assert_abs_diff_eq;
use marklab::{
    homogeneous_pair_correlation, ClassicalSpatialLimits, HomogeneousPairCorrelationConfig,
    ObservationWindow2D, ObservationWindowLimits, PairCorrelationKernel,
    PairCorrelationPointStatus, Pattern, PatternMeta,
};

const PYTHON_ORACLE: &str = include_str!("fixtures/pair_correlation/python_line_oracle.json");

fn pattern(x: Vec<f64>, y: Vec<f64>) -> Pattern {
    Pattern::from_arrays(
        x.clone(),
        y,
        vec![0; x.len()],
        PatternMeta {
            case_id: "pair-correlation-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("pair-correlation-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn config(seed: u64) -> HomogeneousPairCorrelationConfig {
    config_with_bytes(seed, 1 << 20)
}

fn config_with_bytes(seed: u64, maximum_retained_bytes: usize) -> HomogeneousPairCorrelationConfig {
    HomogeneousPairCorrelationConfig::new(
        vec![1.0, 2.0],
        0.5,
        19,
        seed,
        0.05,
        ClassicalSpatialLimits::new(16, 16, 100_000, 100_000, maximum_retained_bytes)
            .expect("limits"),
    )
    .expect("configuration")
}

#[test]
fn epanechnikov_border_g_matches_a_direct_hand_oracle() {
    let pattern = pattern(vec![3.0, 4.0, 7.0], vec![5.0, 5.0, 5.0]);
    let result = homogeneous_pair_correlation(&pattern, &window(), &config(20260827))
        .expect("pair correlation");
    let replay = homogeneous_pair_correlation(&pattern, &window(), &config(20260827))
        .expect("deterministic replay");

    assert_eq!(result, replay);
    assert_eq!(result.kernel, PairCorrelationKernel::Epanechnikov);
    assert_eq!(result.bandwidth_um, 0.5);
    assert_eq!(
        result.edge_correction,
        "standard_border_radius_plus_bandwidth"
    );
    assert_eq!(result.geometry_build_count, 1);

    let at_one = &result.curve[0];
    assert_eq!(at_one.status, PairCorrelationPointStatus::Available);
    assert_eq!(at_one.eligible_centers, 3);
    assert_eq!(at_one.directed_pairs_in_support, 2);
    assert_abs_diff_eq!(at_one.kernel_weight_sum, 3.0, epsilon = 1e-12);
    assert_abs_diff_eq!(
        at_one.g.expect("available g"),
        100.0 * 3.0 / (2.0 * std::f64::consts::PI * 1.0 * 3.0 * 3.0),
        epsilon = 1e-12
    );
    assert_eq!(at_one.theoretical_g, 1.0);

    let at_two = &result.curve[1];
    assert_eq!(
        at_two.status,
        PairCorrelationPointStatus::NoPairsInKernelSupport
    );
    assert_eq!(at_two.g, None);
}

#[test]
fn bandwidth_support_and_finite_output_are_validated_before_geometry() {
    let limits = ClassicalSpatialLimits::new(16, 16, 100_000, 100_000, 1 << 20).expect("limits");
    for (radii, bandwidth) in [
        (vec![0.5], 0.5),
        (vec![1.0], 0.0),
        (vec![1.0], f64::INFINITY),
        (vec![f64::MAX], 0.5),
    ] {
        assert!(
            HomogeneousPairCorrelationConfig::new(radii, bandwidth, 19, 7, 0.05, limits).is_err()
        );
    }
}

#[test]
fn direct_python_pair_loop_agrees_with_the_rust_kernel_estimator() {
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("oracle JSON");
    let pattern = pattern(vec![3.0, 4.0, 7.0], vec![5.0, 5.0, 5.0]);
    let result = homogeneous_pair_correlation(&pattern, &window(), &config(20260827))
        .expect("pair correlation");
    let point = &result.curve[0];
    assert_eq!(
        point.eligible_centers,
        oracle["eligible_centers"].as_u64().expect("centers") as usize
    );
    assert_eq!(
        point.directed_pairs_in_support,
        oracle["directed_pairs_in_support"].as_u64().expect("pairs") as usize
    );
    assert_abs_diff_eq!(
        point.kernel_weight_sum,
        oracle["kernel_weight_sum"].as_f64().expect("weight"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.g.expect("g"),
        oracle["g"].as_f64().expect("oracle g"),
        epsilon = 1e-12
    );
}

#[test]
fn on_radius_pairs_exceed_off_radius_pairs_and_one_short_memory_fails() {
    let exact = pattern(vec![3.0, 4.0, 7.0], vec![5.0, 5.0, 5.0]);
    let shifted = pattern(vec![3.0, 4.25, 7.0], vec![5.0, 5.0, 5.0]);
    let exact_result = homogeneous_pair_correlation(&exact, &window(), &config(71))
        .expect("exact-distance result");
    let shifted_result = homogeneous_pair_correlation(&shifted, &window(), &config(71))
        .expect("shifted-distance result");
    assert!(
        exact_result.curve[0].g.expect("exact g") > shifted_result.curve[0].g.expect("shifted g")
    );

    let one_short = config_with_bytes(71, exact_result.estimated_storage_bytes - 1);
    assert!(matches!(
        homogeneous_pair_correlation(&exact, &window(), &one_short),
        Err(marklab::HomogeneousPairCorrelationError::RetainedByteLimitExceeded {
            required,
            maximum
        }) if required == maximum + 1
    ));
}
