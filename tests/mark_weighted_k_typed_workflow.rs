#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    continuous_mark_weighted_k, BinaryMarkDeclaration, DeclaredScalarPatternInput, MarkTable,
    MarkWeightedKConfig, MarkWeightedKError, MarkWeightedKLimits, MarkWeightedKPointStatus,
    MeasurementStatus, MissingnessPolicy, NucleusAreaUm2MarkDeclaration, ObservationWindow2D,
    ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
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
        b"weighted-k-binary-provenance",
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
        b"weighted-k-area-provenance",
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

fn config(mark_id: ScalarMarkId, seed: u64) -> MarkWeightedKConfig {
    MarkWeightedKConfig::new(
        mark_id,
        vec![0.5, 1.1],
        31,
        seed,
        0.05,
        MarkWeightedKLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("configuration")
}

#[test]
fn cumulative_weighted_and_unweighted_k_match_hand_oracles() {
    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 2.0, 3.0, 4.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let result = continuous_mark_weighted_k(&input, &window, &config(mark_id.clone(), 20260827))
        .expect("weighted K");
    let replay = continuous_mark_weighted_k(&input, &window, &config(mark_id, 20260827))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(
        result.weight_function,
        "product_over_global_arithmetic_mean_squared"
    );
    assert_eq!(result.edge_correction, "standard_border_reduced_sample");
    assert_abs_diff_eq!(result.global_mark_mean, 2.5);
    assert_abs_diff_eq!(result.expected_random_label_weight, 14.0 / 15.0);
    assert_eq!(result.geometry.directed_pair_count, 6);
    assert_eq!(result.geometry.geometry_build_count, 1);

    let first = &result.curve[0];
    assert_eq!(first.status, MarkWeightedKPointStatus::Available);
    assert_eq!(first.eligible_centers, 4);
    assert_eq!(first.directed_pairs, 0);
    assert_eq!(first.weighted_k, Some(0.0));
    assert_eq!(first.unweighted_k, Some(0.0));

    let radius = &result.curve[1];
    assert_eq!(radius.status, MarkWeightedKPointStatus::Available);
    assert_eq!(radius.eligible_centers, 4);
    assert_eq!(radius.directed_pairs, 6);
    assert_abs_diff_eq!(radius.mark_product_sum, 40.0);
    assert_abs_diff_eq!(radius.normalized_weight_sum, 6.4);
    assert_abs_diff_eq!(radius.unweighted_k.expect("unweighted K"), 10.5);
    assert_abs_diff_eq!(
        radius.weighted_k.expect("weighted K"),
        11.2,
        epsilon = 1.0e-12
    );
    assert_abs_diff_eq!(
        radius
            .expected_random_label_weighted_k
            .expect("expected weighted K"),
        9.8,
        epsilon = 1.0e-12
    );
    assert_abs_diff_eq!(radius.theoretical_k, std::f64::consts::PI * 1.1_f64.powi(2));
    assert!(radius.inference_eligible);
    assert_eq!(result.inference.permutations_completed, 31);
    assert!(result.inference.weighted_k.is_some());
}

#[test]
fn independent_python_cumulative_pair_fixture_matches() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/mark_weighted_k/python_line_oracle.json"
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
    let result = continuous_mark_weighted_k(&input, &window, &config(mark_id, 20260827))
        .expect("weighted K");
    for (observed, expected) in result
        .curve
        .iter()
        .zip(oracle["curve"].as_array().expect("curve"))
    {
        assert_eq!(
            observed.eligible_centers as u64,
            expected["eligible_centers"].as_u64().expect("eligible")
        );
        assert_eq!(
            observed.directed_pairs as u64,
            expected["directed_pairs"].as_u64().expect("pairs")
        );
        assert_abs_diff_eq!(
            observed.mark_product_sum,
            expected["mark_product_sum"].as_f64().expect("product")
        );
        assert_abs_diff_eq!(
            observed.weighted_k.expect("weighted"),
            expected["weighted_k"].as_f64().expect("weighted"),
            epsilon = 1.0e-12
        );
        assert_abs_diff_eq!(
            observed.unweighted_k.expect("unweighted"),
            expected["unweighted_k"].as_f64().expect("unweighted"),
            epsilon = 1.0e-12
        );
    }
}

#[test]
fn constant_marks_missing_identity_and_one_short_resources_fail() {
    assert!(MarkWeightedKConfig::new(
        ScalarMarkId::new("nucleus_area_um2").expect("ID"),
        vec![f64::MAX],
        31,
        7,
        0.05,
        MarkWeightedKLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .is_err());
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
        continuous_mark_weighted_k(&input, &window, &config(mark_id, 7)),
        Err(MarkWeightedKError::ZeroMarkVariance)
    ));
    let missing = config(ScalarMarkId::new("different_continuous").expect("ID"), 7);
    assert!(matches!(
        continuous_mark_weighted_k(&input, &window, &missing),
        Err(MarkWeightedKError::MissingContinuousMark)
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
    let baseline = continuous_mark_weighted_k(&input, &window, &config(mark_id.clone(), 11))
        .expect("baseline estimate");
    let one_short = MarkWeightedKConfig::new(
        mark_id,
        vec![0.5, 1.1],
        31,
        11,
        0.05,
        MarkWeightedKLimits::new(
            16,
            16,
            64,
            64 * 31,
            baseline.geometry.estimated_storage_bytes - 1,
        )
        .expect("limits"),
    )
    .expect("config");
    assert!(matches!(
        continuous_mark_weighted_k(&input, &window, &one_short),
        Err(MarkWeightedKError::RetainedByteLimitExceeded { required, maximum })
            if required == baseline.geometry.estimated_storage_bytes && required == maximum + 1
    ));
}

#[test]
fn adjacent_similar_marks_raise_weighted_k_without_changing_unweighted_k() {
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
        continuous_mark_weighted_k(&input, &window, &config(mark_id, 59)).expect("weighted K")
    };
    let adjacent = run([1.0, 2.0, 3.0, 4.0]);
    let alternating = run([1.0, 4.0, 2.0, 3.0]);
    assert!(adjacent.curve[1].weighted_k > alternating.curve[1].weighted_k);
    assert_eq!(
        adjacent.curve[1].unweighted_k,
        alternating.curve[1].unweighted_k
    );
}
