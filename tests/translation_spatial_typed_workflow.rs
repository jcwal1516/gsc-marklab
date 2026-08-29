use approx::assert_abs_diff_eq;
use marklab::{
    analyze_translation_spatial_pattern, ObservationWindow2D, ObservationWindowLimits, Pattern,
    PatternMeta, TranslationSpatialConfig, TranslationSpatialError, TranslationSpatialLimits,
    TranslationSpatialStatus,
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
            case_id: "translation-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("translation-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn config(radius_um: f64) -> TranslationSpatialConfig {
    config_with_limits(
        radius_um,
        TranslationSpatialLimits::new(16, 8, 1_000, 1_000, 100_000, 1_000, 100_000, 1 << 20)
            .expect("translation limits"),
    )
}

fn config_with_limits(
    radius_um: f64,
    limits: TranslationSpatialLimits,
) -> TranslationSpatialConfig {
    TranslationSpatialConfig::new(vec![radius_um], 19, 20260829, 0.05, limits)
        .expect("translation config")
}

fn limits(
    maximum_pair_visits: usize,
    maximum_overlap_evaluations: usize,
    maximum_overlap_candidate_work: usize,
    maximum_overlap_output_vertices: usize,
    maximum_retained_bytes: usize,
) -> TranslationSpatialLimits {
    TranslationSpatialLimits::new(
        16,
        8,
        maximum_pair_visits,
        maximum_overlap_evaluations,
        maximum_overlap_candidate_work,
        maximum_overlap_output_vertices,
        100_000,
        maximum_retained_bytes,
    )
    .expect("limits")
}

#[test]
fn rectangle_and_concave_polygon_match_independent_translation_overlap_oracles() {
    let rectangle =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#);
    let rectangle_result = analyze_translation_spatial_pattern(
        &pattern([2.0, 3.0], [2.0, 2.0]),
        &rectangle,
        &config(1.0),
    )
    .expect("rectangle translation K/L");

    assert_eq!(rectangle_result.status, TranslationSpatialStatus::Available);
    assert_eq!(rectangle_result.correction, "translation");
    assert_eq!(rectangle_result.curve[0].ordered_pairs, 2);
    assert_eq!(rectangle_result.curve[0].overlap_evaluations, 1);
    assert_abs_diff_eq!(
        rectangle_result.curve[0].translation_overlap_area_sum_um2,
        90.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        rectangle_result.curve[0].k.expect("rectangle K"),
        10_000.0 / 90.0,
        epsilon = 1e-12
    );

    // W is a seven-square-unit L. W intersect (W + [1, 0]) is the
    // three-square-unit horizontal rectangle [1,4] x [0,1].
    let concave = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[4,1],[1,1],[1,4],[0,4],[0,0]]]]}"#,
    );
    let concave_result = analyze_translation_spatial_pattern(
        &pattern([0.5, 1.5], [0.5, 0.5]),
        &concave,
        &config(1.0),
    )
    .expect("concave translation K/L");

    assert_eq!(concave_result.curve[0].ordered_pairs, 2);
    assert_eq!(concave_result.curve[0].overlap_evaluations, 1);
    assert_abs_diff_eq!(
        concave_result.curve[0].translation_overlap_area_sum_um2,
        3.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        concave_result.curve[0].k.expect("concave K"),
        49.0 / 3.0,
        epsilon = 1e-12
    );
    assert_eq!(
        concave_result.geometry.overlap_owner,
        "observation_window_2d_exact_boolean_intersection"
    );
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
fn holed_multipolygon_agrees_with_the_static_geos_intersection_oracle() {
    let oracle: GeosTranslationOracle = serde_json::from_str(include_str!(
        "fixtures/translation_overlap/geos_holed_multipolygon_oracle.json"
    ))
    .expect("GEOS oracle");
    assert_eq!(oracle.geos_version, "3.14.1");
    let window = window(&oracle.window_geojson);
    assert_abs_diff_eq!(window.area_um2(), oracle.window_area_um2, epsilon = 1e-12);
    let [dx, dy] = oracle.displacement_um;
    let result = analyze_translation_spatial_pattern(
        &pattern([1.0, 1.0 + dx], [2.0, 2.0 + dy]),
        &window,
        &config(dx.hypot(dy)),
    )
    .expect("GEOS translation K/L");

    assert_abs_diff_eq!(
        result.curve[0].translation_overlap_area_sum_um2,
        oracle.translation_overlap_area_um2,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        result.curve[0].k.expect("K"),
        oracle.window_area_um2 * oracle.window_area_um2 / oracle.translation_overlap_area_um2,
        epsilon = 1e-12
    );
}

#[test]
fn exact_work_memory_and_zero_measure_overlap_boundaries_fail_explicitly() {
    let window =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#);
    let points = pattern([2.0, 3.0], [2.0, 2.0]);

    let exact = analyze_translation_spatial_pattern(
        &points,
        &window,
        &config_with_limits(20.0, limits(20, 20, 320, 24, 1 << 20)),
    )
    .expect("exact all-pair work");
    assert_eq!(exact.geometry.total_pair_visits, 20);
    assert_eq!(exact.geometry.total_overlap_evaluations, 20);
    assert_eq!(exact.geometry.total_overlap_candidate_work, 320);

    assert!(matches!(
        analyze_translation_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(19, 20, 320, 24, 1 << 20)),
        ),
        Err(TranslationSpatialError::PairVisitLimitExceeded {
            observed: 20,
            maximum: 19
        })
    ));
    assert!(matches!(
        analyze_translation_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 19, 320, 24, 1 << 20)),
        ),
        Err(TranslationSpatialError::OverlapEvaluationLimitExceeded {
            observed: 20,
            maximum: 19
        })
    ));
    assert!(matches!(
        analyze_translation_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 20, 319, 24, 1 << 20)),
        ),
        Err(TranslationSpatialError::OverlapCandidateWorkLimitExceeded {
            required: 320,
            maximum: 319
        })
    ));
    assert!(matches!(
        analyze_translation_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 20, 320, 23, 1 << 20)),
        ),
        Err(TranslationSpatialError::InvalidConfig { .. })
    ));
    let required = exact.geometry.estimated_storage_bytes;
    assert!(matches!(
        analyze_translation_spatial_pattern(
            &points,
            &window,
            &config_with_limits(20.0, limits(20, 20, 320, 24, required - 1)),
        ),
        Err(TranslationSpatialError::RetainedByteLimitExceeded {
            required: observed,
            maximum
        }) if observed == required && maximum + 1 == required
    ));

    let boundary_pair = pattern([0.0, 10.0], [0.0, 10.0]);
    assert!(matches!(
        analyze_translation_spatial_pattern(
            &boundary_pair,
            &window,
            &config_with_limits(15.0, limits(1_000, 1_000, 100_000, 24, 1 << 20)),
        ),
        Err(TranslationSpatialError::NonPositiveOverlap { left: 0, right: 1 })
    ));
}
