#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    categorical_mark_connection_cross_k, BinaryMarkDeclaration, CategoricalPairConfig,
    CategoricalPairLimits, CategoricalPairPointStatus, DeclaredScalarPatternInput,
    HistologicCompartmentMarkDeclaration, MarkTable, MeasurementStatus, MissingnessPolicy,
    ObservationWindow2D, ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

fn input_fixture(codes: [u32; 4]) -> (Fixture, marklab::Pattern, MarkTable, ObservationWindow2D) {
    let mut fixture = fixture();
    let mut pattern = fixture.pattern.clone();
    pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        Vec::from(codes).into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        &mut fixture,
        b"categorical-pair-binary-provenance",
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
    let category_provenance = publish_record(
        &mut fixture,
        b"categorical-pair-compartment-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(
            MeasurementStatus::ImportedPrediction,
            &["tumor", "stroma"],
        ),
    );
    let binary = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_provenance,
    )
    .expect("binary declaration");
    let category = HistologicCompartmentMarkDeclaration::new(
        vec!["tumor".into(), "stroma".into()],
        MeasurementStatus::ImportedPrediction,
        category_provenance,
    )
    .expect("category declaration");
    let table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary,
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::histologic_compartment(
                category,
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                pattern.categorical_strata["histologic_compartment"].clone(),
            )
            .expect("category column"),
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
    (fixture, pattern, table, window)
}

fn config(seed: u64) -> CategoricalPairConfig {
    CategoricalPairConfig::new(
        vec![0.5, 1.1],
        "tumor",
        "stroma",
        31,
        seed,
        0.05,
        CategoricalPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("configuration")
}

#[test]
fn typed_categorical_mark_connection_and_directed_cross_k_match_hand_oracles() {
    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");

    let result = categorical_mark_connection_cross_k(&input, &window, &config(20260827))
        .expect("categorical pair result");
    let replay = categorical_mark_connection_cross_k(&input, &window, &config(20260827))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.mark_id.as_str(), "histologic_compartment");
    assert_eq!(result.source_level, "tumor");
    assert_eq!(result.target_level, "stroma");
    assert_eq!(
        result.measurement_status,
        MeasurementStatus::ImportedPrediction
    );
    assert_eq!(result.source_count, 2);
    assert_eq!(result.target_count, 2);
    assert_eq!(result.geometry.directed_pair_count, 6);
    assert_eq!(result.geometry.geometry_build_count, 1);

    let empty = &result.curve[0];
    assert_eq!(empty.status, CategoricalPairPointStatus::NoPairsInShell);
    assert_eq!(empty.connection_probability, None);
    assert_eq!(empty.cross_k, Some(0.0));

    let radius = &result.curve[1];
    assert_eq!(radius.status, CategoricalPairPointStatus::Available);
    assert_eq!(radius.eligible_source_centers, 2);
    assert_eq!(radius.directed_source_target_pairs, 1);
    assert_eq!(radius.directed_pairs_in_shell, 6);
    assert_eq!(radius.source_target_pairs_in_shell, 1);
    assert_abs_diff_eq!(
        radius.connection_probability.expect("connection"),
        1.0 / 6.0
    );
    assert_abs_diff_eq!(radius.cross_k.expect("cross K"), 7.0);
    assert_abs_diff_eq!(
        radius.theoretical_cross_k,
        std::f64::consts::PI * 1.1_f64.powi(2)
    );
    assert_abs_diff_eq!(result.expected_random_label_connection, 1.0 / 3.0);
    assert_eq!(result.inference.permutations_completed, 31);
    assert!(result.inference.connection.is_some());
    assert!(result.inference.cross_k.is_some());
}

#[test]
fn same_levels_and_one_short_pair_work_are_rejected() {
    assert!(CategoricalPairConfig::new(
        vec![f64::MAX],
        "tumor",
        "stroma",
        31,
        7,
        0.05,
        CategoricalPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .is_err());
    assert!(CategoricalPairConfig::new(
        vec![1.0],
        "tumor",
        "tumor",
        31,
        7,
        0.05,
        CategoricalPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .is_err());

    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let unknown = CategoricalPairConfig::new(
        vec![1.1],
        "immune",
        "stroma",
        31,
        7,
        0.05,
        CategoricalPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("unknown-level config");
    assert!(matches!(
        categorical_mark_connection_cross_k(&input, &window, &unknown),
        Err(marklab::CategoricalPairError::UnknownLevel { level }) if level == "immune"
    ));
    let limited = CategoricalPairConfig::new(
        vec![1.1],
        "tumor",
        "stroma",
        31,
        7,
        0.05,
        CategoricalPairLimits::new(16, 16, 1, 31, 1 << 20).expect("limits"),
    )
    .expect("limited config");
    assert!(categorical_mark_connection_cross_k(&input, &window, &limited).is_err());

    let baseline = categorical_mark_connection_cross_k(&input, &window, &config(11))
        .expect("baseline resource estimate");
    let one_short = CategoricalPairConfig::new(
        vec![0.5, 1.1],
        "tumor",
        "stroma",
        31,
        11,
        0.05,
        CategoricalPairLimits::new(
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
        categorical_mark_connection_cross_k(&input, &window, &one_short),
        Err(marklab::CategoricalPairError::RetainedByteLimitExceeded { required, maximum })
            if required == baseline.geometry.estimated_storage_bytes && required == maximum + 1
    ));
}

#[test]
fn alternating_labels_increase_connection_and_cross_k_over_segregated_labels() {
    let run = |codes| {
        let (fixture, pattern, table, window) = input_fixture(codes);
        let input = DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        )
        .expect("declared input");
        categorical_mark_connection_cross_k(&input, &window, &config(59))
            .expect("categorical pair result")
    };
    let segregated = run([0, 0, 1, 1]);
    let alternating = run([0, 1, 0, 1]);
    assert!(
        alternating.curve[1]
            .connection_probability
            .expect("alternating connection")
            > segregated.curve[1]
                .connection_probability
                .expect("segregated connection")
    );
    assert!(
        alternating.curve[1].cross_k.expect("alternating cross K")
            > segregated.curve[1].cross_k.expect("segregated cross K")
    );
}
