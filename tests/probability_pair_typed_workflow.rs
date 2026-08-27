#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    probability_mark_connection, BinaryMarkDeclaration, DeclaredScalarPatternInput, MarkTable,
    MeasurementStatus, MissingnessPolicy, ObservationWindow2D, ObservationWindowLimits,
    ProbabilityMarkDeclaration, ProbabilityPairConfig, ProbabilityPairError, ProbabilityPairLimits,
    ProbabilityPairPointStatus, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

fn input_fixture(
    probabilities: [f32; 4],
) -> (
    Fixture,
    marklab::Pattern,
    MarkTable,
    ObservationWindow2D,
    ScalarMarkId,
) {
    let mut fixture = fixture();
    let mut pattern = fixture.pattern.clone();
    pattern.mark_prob = Some(Vec::from(probabilities).into_boxed_slice());
    let binary_provenance = publish_record(
        &mut fixture,
        b"probability-pair-binary-provenance",
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
    let probability_mark_id = ScalarMarkId::new("tumor_probability").expect("probability ID");
    let probability_provenance = publish_record(
        &mut fixture,
        b"probability-pair-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata(
            probability_mark_id.as_str(),
            MeasurementStatus::ImportedPrediction,
        ),
    );
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
            ScalarMarkColumn::probability(
                ProbabilityMarkDeclaration::new(
                    probability_mark_id.clone(),
                    MeasurementStatus::ImportedPrediction,
                    probability_provenance,
                )
                .expect("probability declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                probabilities,
            )
            .expect("probability column"),
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
    (fixture, pattern, table, window, probability_mark_id)
}

fn config(mark_id: ScalarMarkId, seed: u64) -> ProbabilityPairConfig {
    ProbabilityPairConfig::new(
        mark_id,
        vec![0.5, 1.1],
        31,
        seed,
        0.05,
        ProbabilityPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("configuration")
}

#[test]
fn expected_positive_positive_connection_matches_hand_oracle() {
    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 0.5, 0.0, 1.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");

    let result = probability_mark_connection(&input, &window, &config(mark_id.clone(), 20260827))
        .expect("probability pair result");
    let replay = probability_mark_connection(&input, &window, &config(mark_id, 20260827))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.mark_id.as_str(), "tumor_probability");
    assert_eq!(
        result.measurement_status,
        MeasurementStatus::ImportedPrediction
    );
    assert_eq!(
        result.normalization,
        "expected_positive_positive_pairs_per_eligible_directed_pair"
    );
    assert_abs_diff_eq!(result.effective_positive_mass, 2.5);
    assert_abs_diff_eq!(result.effective_negative_mass, 1.5);
    assert_abs_diff_eq!(result.expected_random_label_connection, 1.0 / 3.0);
    assert_eq!(result.geometry.directed_pair_count, 6);
    assert_eq!(result.geometry.geometry_build_count, 1);

    let empty = &result.curve[0];
    assert_eq!(empty.status, ProbabilityPairPointStatus::NoPairsInShell);
    assert_eq!(empty.connection_probability, None);

    let radius = &result.curve[1];
    assert_eq!(radius.status, ProbabilityPairPointStatus::Available);
    assert_eq!(radius.directed_pairs_in_shell, 6);
    assert_abs_diff_eq!(radius.expected_positive_pairs_in_shell, 1.0);
    assert_abs_diff_eq!(
        radius.connection_probability.expect("connection"),
        1.0 / 6.0
    );
    assert!(radius.connection_inference_eligible);
    assert_eq!(result.inference.permutations_completed, 31);
    assert!(result.inference.connection.is_some());
}

#[test]
fn insufficient_effective_pair_mass_and_pair_work_are_rejected() {
    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 0.0, 0.0, 0.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    assert!(matches!(
        probability_mark_connection(&input, &window, &config(mark_id.clone(), 7)),
        Err(ProbabilityPairError::InsufficientEffectivePositivePairMass)
    ));
    let missing = config(
        ScalarMarkId::new("different_probability").expect("different ID"),
        7,
    );
    assert!(matches!(
        probability_mark_connection(&input, &window, &missing),
        Err(ProbabilityPairError::MissingProbabilityMark)
    ));

    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 1.0, 1.0, 1.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    assert!(matches!(
        probability_mark_connection(&input, &window, &config(mark_id, 7)),
        Err(ProbabilityPairError::NoEffectiveNegativeMass)
    ));

    let (fixture, pattern, table, window, mark_id) = input_fixture([1.0, 1.0, 0.0, 0.0]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let limited = ProbabilityPairConfig::new(
        mark_id,
        vec![1.1],
        31,
        7,
        0.05,
        ProbabilityPairLimits::new(16, 16, 1, 31, 1 << 20).expect("limits"),
    )
    .expect("limited config");
    assert!(matches!(
        probability_mark_connection(&input, &window, &limited),
        Err(ProbabilityPairError::DirectedPairLimitExceeded { maximum: 1 })
    ));
}

#[test]
fn clustered_probability_rows_increase_connection_over_alternating_rows() {
    let run = |probabilities| {
        let (fixture, pattern, table, window, mark_id) = input_fixture(probabilities);
        let input = DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        )
        .expect("declared input");
        probability_mark_connection(&input, &window, &config(mark_id, 59))
            .expect("probability pair result")
    };
    let clustered = run([1.0, 1.0, 0.0, 0.0]);
    let alternating = run([1.0, 0.0, 1.0, 0.0]);
    assert!(
        clustered.curve[1]
            .connection_probability
            .expect("clustered connection")
            > alternating.curve[1]
                .connection_probability
                .expect("alternating connection")
    );
}
