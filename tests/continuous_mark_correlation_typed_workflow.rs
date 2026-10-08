#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    continuous_mark_correlation, BinaryMarkDeclaration, ContinuousMarkCorrelationConfig,
    ContinuousMarkCorrelationError, ContinuousMarkCorrelationLimits,
    ContinuousMarkCorrelationPointStatus, DeclaredScalarPatternInput, MarkTable, MeasurementStatus,
    MissingnessPolicy, NucleusAreaUm2MarkDeclaration, ObservationWindow2D, ObservationWindowLimits,
    ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

fn input_fixture(
    areas: [f32; 4],
) -> (
    Fixture,
    marklab::Pattern,
    MarkTable,
    ObservationWindow2D,
    ScalarMarkId,
) {
    let mut fixture = fixture();
    let mut pattern = fixture.pattern.clone();
    pattern.nucleus_area_um2 = Some(Vec::from(areas).into_boxed_slice());
    let binary_provenance = publish_record(
        &mut fixture,
        b"continuous-correlation-binary-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    );
    let continuous_provenance = publish_record(
        &mut fixture,
        b"continuous-correlation-area-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    let mark_id = ScalarMarkId::new("nucleus_area_um2").expect("continuous ID");
    let table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::independent(
                    ScalarMarkId::new("mmr_loss").expect("binary ID"),
                    "MMR loss",
                    MeasurementStatus::Measured,
                    binary_provenance,
                )
                .expect("binary declaration"),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::continuous(
                NucleusAreaUm2MarkDeclaration::new(
                    MeasurementStatus::Measured,
                    continuous_provenance,
                )
                .expect("continuous declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::SquareMicrometer,
                MissingnessPolicy::NotPermitted,
                areas,
            )
            .expect("continuous column"),
        ],
        fixture.cell_ids.len(),
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("mark table");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-2,-2],[5,-2],[5,2],[-2,2],[-2,-2]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    (fixture, pattern, table, window, mark_id)
}

fn config(mark_id: ScalarMarkId, seed: u64) -> ContinuousMarkCorrelationConfig {
    ContinuousMarkCorrelationConfig::new(
        mark_id,
        vec![0.5, 1.1],
        31,
        seed,
        0.05,
        ContinuousMarkCorrelationLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("configuration")
}

#[test]
fn normalized_global_mean_product_matches_hand_oracle() {
    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 2.0, 3.0, 4.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");

    let result = continuous_mark_correlation(&input, &window, &config(mark_id.clone(), 20260827))
        .expect("continuous correlation");
    let replay = continuous_mark_correlation(&input, &window, &config(mark_id, 20260827))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.mark_id.as_str(), "nucleus_area_um2");
    assert_eq!(result.measurement_status, MeasurementStatus::Measured);
    assert_eq!(result.unit, "square_micrometer");
    assert_eq!(result.normalization, "global_arithmetic_mean_product");
    assert_abs_diff_eq!(result.global_mark_mean, 2.5);
    assert_abs_diff_eq!(result.global_mark_population_variance, 1.25);
    assert_abs_diff_eq!(result.expected_random_label_correlation, 14.0 / 15.0);
    assert_eq!(result.geometry.directed_pair_count, 6);
    assert_eq!(result.geometry.geometry_build_count, 1);

    let empty = &result.curve[0];
    assert_eq!(
        empty.status,
        ContinuousMarkCorrelationPointStatus::NoPairsInShell
    );
    assert_eq!(empty.correlation, None);

    let radius = &result.curve[1];
    assert_eq!(
        radius.status,
        ContinuousMarkCorrelationPointStatus::Available
    );
    assert_eq!(radius.directed_pairs_in_shell, 6);
    assert_abs_diff_eq!(radius.mark_product_sum_in_shell, 40.0);
    assert_abs_diff_eq!(radius.correlation.expect("correlation"), 16.0 / 15.0);
    assert!(radius.inference_eligible);
    assert_eq!(result.inference.permutations_completed, 31);
    assert!(result.inference.correlation.is_some());
}

#[test]
fn independent_python_direct_pair_fixture_matches_every_curve_value() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/continuous_mark_correlation/python_line_oracle.json"
    ))
    .expect("Python oracle JSON");
    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 2.0, 3.0, 4.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let result = continuous_mark_correlation(&input, &window, &config(mark_id, 20260827))
        .expect("continuous correlation");
    assert_abs_diff_eq!(
        result.global_mark_mean,
        oracle["global_mark_mean"].as_f64().expect("mean")
    );
    assert_abs_diff_eq!(
        result.global_mark_population_variance,
        oracle["global_mark_population_variance"]
            .as_f64()
            .expect("variance")
    );
    assert_abs_diff_eq!(
        result.expected_random_label_correlation,
        oracle["expected_random_label_correlation"]
            .as_f64()
            .expect("expectation")
    );
    for (observed, expected) in result
        .curve
        .iter()
        .zip(oracle["curve"].as_array().expect("curve"))
    {
        assert_eq!(
            observed.directed_pairs_in_shell as u64,
            expected["directed_pairs"].as_u64().expect("pair count")
        );
        assert_abs_diff_eq!(
            observed.mark_product_sum_in_shell,
            expected["mark_product_sum"].as_f64().expect("product sum")
        );
        match (observed.correlation, expected["correlation"].as_f64()) {
            (Some(observed), Some(expected)) => assert_abs_diff_eq!(observed, expected),
            (None, None) => {}
            values => panic!("correlation availability differs: {values:?}"),
        }
    }
}

#[test]
fn constant_marks_missing_identity_and_pair_work_are_rejected() {
    let (fixture, pattern, table, window, mark_id) = input_fixture([2.0, 2.0, 2.0, 2.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    assert!(matches!(
        continuous_mark_correlation(&input, &window, &config(mark_id, 7)),
        Err(ContinuousMarkCorrelationError::ZeroMarkVariance)
    ));
    let missing = config(
        ScalarMarkId::new("different_continuous").expect("different ID"),
        7,
    );
    assert!(matches!(
        continuous_mark_correlation(&input, &window, &missing),
        Err(ContinuousMarkCorrelationError::MissingContinuousMark)
    ));

    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 2.0, 3.0, 4.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let limited = ContinuousMarkCorrelationConfig::new(
        mark_id,
        vec![1.1],
        31,
        7,
        0.05,
        ContinuousMarkCorrelationLimits::new(16, 16, 1, 31, 1 << 20).expect("limits"),
    )
    .expect("limited config");
    assert!(matches!(
        continuous_mark_correlation(&input, &window, &limited),
        Err(ContinuousMarkCorrelationError::DirectedPairLimitExceeded { maximum: 1 })
    ));

    let baseline = continuous_mark_correlation(
        &input,
        &window,
        &config(
            ScalarMarkId::new("nucleus_area_um2").expect("continuous ID"),
            9,
        ),
    )
    .expect("baseline resource estimate");
    let one_short = ContinuousMarkCorrelationConfig::new(
        ScalarMarkId::new("nucleus_area_um2").expect("continuous ID"),
        vec![0.5, 1.1],
        31,
        9,
        0.05,
        ContinuousMarkCorrelationLimits::new(
            16,
            16,
            64,
            64 * 31,
            baseline.geometry.estimated_storage_bytes - 1,
        )
        .expect("one-short limits"),
    )
    .expect("one-short config");
    assert!(matches!(
        continuous_mark_correlation(&input, &window, &one_short),
        Err(ContinuousMarkCorrelationError::RetainedByteLimitExceeded { required, maximum })
            if required == baseline.geometry.estimated_storage_bytes && required == maximum + 1
    ));
}

#[test]
fn adjacent_similar_marks_increase_correlation_over_alternating_marks() {
    let run = |areas| {
        let (fixture, pattern, table, window, mark_id) = input_fixture(areas);
        let input = DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        )
        .expect("declared input");
        continuous_mark_correlation(&input, &window, &config(mark_id, 59))
            .expect("continuous correlation")
    };
    let adjacent = run([1.0, 2.0, 3.0, 4.0]);
    let alternating = run([1.0, 4.0, 2.0, 3.0]);
    assert!(
        adjacent.curve[1].correlation.expect("adjacent correlation")
            > alternating.curve[1]
                .correlation
                .expect("alternating correlation")
    );
}

#[test]
fn dominant_positive_mark_retains_nonzero_random_label_mass() {
    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0e30, 1.0, 1.0, 1.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let result = continuous_mark_correlation(&input, &window, &config(mark_id, 71))
        .expect("dominant-mark correlation");
    assert!(result.expected_random_label_correlation.is_finite());
    assert!(result.expected_random_label_correlation > 0.0);
    assert!(result.curve[1]
        .correlation
        .expect("available correlation")
        .is_finite());
}
