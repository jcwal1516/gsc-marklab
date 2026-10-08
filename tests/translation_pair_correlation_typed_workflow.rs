use approx::assert_abs_diff_eq;
use marklab::{
    analyze_translation_pair_correlation, ObservationWindow2D, ObservationWindowLimits,
    PairCorrelationPointStatus, Pattern, PatternMeta, TranslationPairCorrelationConfig,
    TranslationSpatialError, TranslationSpatialLimits,
};

fn pattern(x: Vec<f64>, y: Vec<f64>) -> Pattern {
    Pattern::from_arrays(
        x.clone(),
        y,
        vec![0; x.len()],
        PatternMeta {
            case_id: "translation-g-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("translation-g-slide".into()),
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

fn limits() -> TranslationSpatialLimits {
    bounded_limits(1_000, 1_000, 16_000, 64, 1 << 20)
}

fn bounded_limits(
    maximum_pair_visits: usize,
    maximum_overlap_evaluations: usize,
    maximum_overlap_candidate_work: usize,
    maximum_overlap_output_vertices: usize,
    maximum_retained_bytes: usize,
) -> TranslationSpatialLimits {
    TranslationSpatialLimits::new(
        16,
        16,
        maximum_pair_visits,
        maximum_overlap_evaluations,
        maximum_overlap_candidate_work,
        maximum_overlap_output_vertices,
        100_000,
        maximum_retained_bytes,
    )
    .expect("limits")
}

fn config_with_limits(
    radii_um: Vec<f64>,
    bandwidth_um: f64,
    limits: TranslationSpatialLimits,
) -> TranslationPairCorrelationConfig {
    TranslationPairCorrelationConfig::new(radii_um, bandwidth_um, 19, 20260829, 0.05, limits)
        .expect("config")
}

#[test]
fn rectangle_translation_g_matches_the_exact_overlap_hand_oracle() {
    let pattern = pattern(vec![1.0, 2.0], vec![5.0, 5.0]);
    let config =
        TranslationPairCorrelationConfig::new(vec![1.0], 0.5, 19, 20260829, 0.05, limits())
            .expect("config");

    let result = analyze_translation_pair_correlation(&pattern, &rectangle(), &config)
        .expect("translation-corrected g");

    assert_eq!(result.correction, "translation");
    assert_eq!(result.bandwidth_um, 0.5);
    assert_eq!(result.curve.len(), 1);
    let point = &result.curve[0];
    assert_eq!(point.ordered_pairs_in_support, 2);
    assert_eq!(point.overlap_evaluations, 1);
    assert_abs_diff_eq!(point.translation_overlap_area_sum_um2, 90.0);
    assert_abs_diff_eq!(point.translation_weighted_kernel_sum, 10.0 / 3.0);
    assert_abs_diff_eq!(
        point.g.expect("available g"),
        250.0 / (3.0 * std::f64::consts::PI),
        epsilon = 1e-12
    );
    assert_eq!(point.theoretical_g, 1.0);
}

#[derive(serde::Deserialize)]
struct GeosTranslationOracle {
    geos_version: String,
    window_geojson: String,
    displacement_um: [f64; 2],
    window_area_um2: f64,
    translation_overlap_area_um2: f64,
}

#[test]
fn holed_multipolygon_g_agrees_with_the_static_geos_overlap_oracle() {
    let oracle: GeosTranslationOracle = serde_json::from_str(include_str!(
        "fixtures/translation_overlap/geos_holed_multipolygon_oracle.json"
    ))
    .expect("GEOS oracle");
    assert_eq!(oracle.geos_version, "3.14.1");
    let window = window(&oracle.window_geojson);
    let [dx, dy] = oracle.displacement_um;
    let radius = dx.hypot(dy);
    let result = analyze_translation_pair_correlation(
        &pattern(vec![1.0, 1.0 + dx], vec![2.0, 2.0 + dy]),
        &window,
        &config_with_limits(
            vec![radius],
            0.5,
            bounded_limits(1_000, 1_000, 100_000, 256, 1 << 20),
        ),
    )
    .expect("GEOS translation g");

    let point = &result.curve[0];
    assert_abs_diff_eq!(
        point.translation_overlap_area_sum_um2,
        oracle.translation_overlap_area_um2,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.g.expect("g"),
        3.0 * oracle.window_area_um2 * oracle.window_area_um2
            / (4.0 * std::f64::consts::PI * radius * oracle.translation_overlap_area_um2),
        epsilon = 1e-12
    );
}

#[test]
fn compact_support_and_exact_work_memory_boundaries_are_explicit() {
    let window = rectangle();
    let points = pattern(vec![1.0, 2.0], vec![5.0, 5.0]);
    let endpoint = analyze_translation_pair_correlation(
        &pattern(vec![1.0, 2.5], vec![5.0, 5.0]),
        &window,
        &config_with_limits(vec![1.0], 0.5, limits()),
    )
    .expect("kernel endpoint");
    assert_eq!(
        endpoint.curve[0].status,
        PairCorrelationPointStatus::NoPairsInKernelSupport
    );
    assert_eq!(endpoint.curve[0].g, None);
    assert_eq!(endpoint.geometry.observed_overlap_evaluations, 0);

    let all_support =
        config_with_limits(vec![20.0], 19.999, bounded_limits(20, 20, 320, 24, 1 << 20));
    let exact = analyze_translation_pair_correlation(&points, &window, &all_support)
        .expect("exact all-pair work");
    assert_eq!(exact.geometry.total_pair_visits, 20);
    assert_eq!(exact.geometry.total_overlap_evaluations, 20);
    assert_eq!(exact.geometry.total_overlap_candidate_work, 320);

    assert!(matches!(
        analyze_translation_pair_correlation(
            &points,
            &window,
            &config_with_limits(vec![20.0], 19.999, bounded_limits(19, 20, 320, 24, 1 << 20),),
        ),
        Err(TranslationSpatialError::PairVisitLimitExceeded {
            observed: 20,
            maximum: 19
        })
    ));
    assert!(matches!(
        analyze_translation_pair_correlation(
            &points,
            &window,
            &config_with_limits(vec![20.0], 19.999, bounded_limits(20, 19, 320, 24, 1 << 20),),
        ),
        Err(TranslationSpatialError::OverlapEvaluationLimitExceeded {
            observed: 20,
            maximum: 19
        })
    ));
    assert!(matches!(
        analyze_translation_pair_correlation(
            &points,
            &window,
            &config_with_limits(vec![20.0], 19.999, bounded_limits(20, 20, 319, 24, 1 << 20),),
        ),
        Err(TranslationSpatialError::OverlapCandidateWorkLimitExceeded {
            required: 320,
            maximum: 319
        })
    ));
    assert!(matches!(
        analyze_translation_pair_correlation(
            &points,
            &window,
            &config_with_limits(vec![20.0], 19.999, bounded_limits(20, 20, 320, 23, 1 << 20),),
        ),
        Err(TranslationSpatialError::InvalidConfig { .. })
    ));
    let required = exact.geometry.estimated_storage_bytes;
    assert!(matches!(
        analyze_translation_pair_correlation(
            &points,
            &window,
            &config_with_limits(
                vec![20.0],
                19.999,
                bounded_limits(20, 20, 320, 24, required - 1),
            ),
        ),
        Err(TranslationSpatialError::RetainedByteLimitExceeded {
            required: observed,
            maximum
        }) if observed == required && maximum + 1 == required
    ));
}
