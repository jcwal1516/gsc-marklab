use approx::assert_abs_diff_eq;
use marklab::{
    analyze_inhomogeneous_pair_correlation, analyze_inhomogeneous_spatial_pattern,
    InhomogeneousPairCorrelationConfig, InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    ObservationWindow2D, ObservationWindowLimits, PairCorrelationKernel,
    PairCorrelationPointStatus, Pattern, PatternMeta,
};

const PYTHON_ORACLE: &str =
    include_str!("fixtures/inhomogeneous_pair_correlation/python_rectangle_oracle.json");

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![2.0, 3.0, 7.0, 9.0],
        vec![2.0, 2.0, 7.0, 7.0],
        vec![0; 4],
        PatternMeta {
            case_id: "inhomogeneous-g-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("inhomogeneous-g-slide".into()),
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

fn intensity_config(seed: u64) -> InhomogeneousSpatialConfig {
    intensity_config_with_limits(
        seed,
        InhomogeneousSpatialLimits::new(16, 16, 1_000, 1_000_000, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    )
}

fn intensity_config_with_limits(
    seed: u64,
    limits: InhomogeneousSpatialLimits,
) -> InhomogeneousSpatialConfig {
    InhomogeneousSpatialConfig::new(vec![1.0], 2.0, [10, 10], 19, seed, 0.05, 1e-12, limits)
        .expect("intensity configuration")
}

#[test]
fn persisted_leave_one_out_pilot_flows_into_inhomogeneous_g() {
    let pattern = pattern();
    let window = window();
    let base = intensity_config(20260827);
    let kl =
        analyze_inhomogeneous_spatial_pattern(&pattern, &window, &base).expect("inhomogeneous K/L");
    let config = InhomogeneousPairCorrelationConfig::new(base, 0.5).expect("g config");
    let result = analyze_inhomogeneous_pair_correlation(&pattern, &window, &config)
        .expect("inhomogeneous g");
    let replay = analyze_inhomogeneous_pair_correlation(
        &pattern,
        &window,
        &InhomogeneousPairCorrelationConfig::new(intensity_config(20260827), 0.5)
            .expect("replay config"),
    )
    .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.intensity, kl.intensity);
    assert_eq!(result.kernel, PairCorrelationKernel::Epanechnikov);
    assert_eq!(result.pair_bandwidth_um, 0.5);
    assert_eq!(result.intensity_bandwidth_um, 2.0);

    let point = &result.curve[0];
    assert_eq!(point.status, PairCorrelationPointStatus::Available);
    assert_eq!(point.eligible_centers, 3);
    assert_eq!(point.directed_pairs_in_support, 2);
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("oracle JSON");
    assert_abs_diff_eq!(
        point.inverse_intensity_kernel_sum,
        oracle["inverse_intensity_kernel_sum"]
            .as_f64()
            .expect("kernel sum"),
        epsilon = 1e-10
    );
    assert_abs_diff_eq!(
        point.eligible_center_inverse_intensity_sum,
        oracle["eligible_center_inverse_intensity_sum"]
            .as_f64()
            .expect("center sum"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.g.expect("inhomogeneous g"),
        oracle["g"].as_f64().expect("oracle g"),
        epsilon = 1e-12
    );
    assert_eq!(point.theoretical_g, 1.0);
    assert_eq!(
        result.inference.null_model,
        "fixed_gridded_inhomogeneous_binomial"
    );
    assert_eq!(result.inference.simulations_completed, 19);
}

#[test]
fn zero_kernel_endpoints_and_actual_resource_ceilings_are_explicit() {
    assert!(InhomogeneousPairCorrelationConfig::new(intensity_config(31), 1.0).is_err());
    let baseline = analyze_inhomogeneous_pair_correlation(
        &pattern(),
        &window(),
        &InhomogeneousPairCorrelationConfig::new(intensity_config(31), 0.5)
            .expect("baseline config"),
    )
    .expect("baseline");
    let limits = |pairs, bytes| {
        InhomogeneousSpatialLimits::new(16, 16, 1_000, 1_000_000, pairs, 1_000_000, bytes)
            .expect("limits")
    };
    assert!(matches!(
        analyze_inhomogeneous_pair_correlation(
            &pattern(),
            &window(),
            &InhomogeneousPairCorrelationConfig::new(
                intensity_config_with_limits(
                    31,
                    limits(1_000_000, baseline.estimated_storage_bytes - 1)
                ),
                0.5
            )
            .expect("one-short memory config")
        ),
        Err(marklab::InhomogeneousSpatialError::RetainedByteLimitExceeded {
            required,
            maximum
        }) if required == maximum + 1
    ));
    assert!(matches!(
        analyze_inhomogeneous_pair_correlation(
            &pattern(),
            &window(),
            &InhomogeneousPairCorrelationConfig::new(
                intensity_config_with_limits(31, limits(baseline.total_pair_visits - 1, 1 << 20)),
                0.5
            )
            .expect("one-short pair config")
        ),
        Err(marklab::InhomogeneousSpatialError::PairVisitLimitExceeded { .. })
    ));

    let endpoint_pattern = Pattern::from_arrays(
        vec![2.0, 3.5, 7.0, 9.0],
        vec![2.0, 2.0, 7.0, 7.0],
        vec![0; 4],
        pattern().meta,
    )
    .expect("endpoint pattern");
    let endpoint = analyze_inhomogeneous_pair_correlation(
        &endpoint_pattern,
        &window(),
        &InhomogeneousPairCorrelationConfig::new(intensity_config(31), 0.5)
            .expect("endpoint config"),
    )
    .expect("endpoint result");
    assert_eq!(
        endpoint.curve[0].status,
        PairCorrelationPointStatus::NoPairsInKernelSupport
    );
    assert_eq!(endpoint.curve[0].g, None);
}
