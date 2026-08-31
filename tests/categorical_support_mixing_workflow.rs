#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use marklab::{
    categorical_support_mixing, execute_algorithm_with_store, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, CategoricalSupportMixingAnalysisNode,
    CategoricalSupportMixingConfig, CategoricalSupportMixingError, CategoricalSupportMixingLimits,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits,
    HistologicCompartmentMarkDeclaration, LocalScheduler, MarkTable, MeasurementStatus,
    MissingnessPolicy, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, ProbabilityMarkDeclaration, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, WorkflowGraph,
};
use support::{
    binary_metadata, cell_id_text_bytes, fixture, histologic_compartment_metadata,
    probability_metadata, publish_record, Fixture, MARK_SCHEMA,
};

fn table(fixture: &mut Fixture) -> MarkTable {
    fixture.pattern.x_um = vec![0.0, 1.0, 2.5, 4.0].into_boxed_slice();
    fixture.pattern.y_um = vec![0.0; 4].into_boxed_slice();
    let codes = vec![0, 1, 1, 2];
    let supports = vec![1.0, 0.5, 0.25, 0.75];
    fixture.pattern.mark_prob = Some(supports.clone().into_boxed_slice());
    fixture.pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        codes.clone().into_boxed_slice(),
    );
    fixture.pattern.categorical_stratum_levels.insert(
        "histologic_compartment".into(),
        vec![
            "neoplastic".into(),
            "inflammatory".into(),
            "connective".into(),
        ]
        .into_boxed_slice(),
    );
    let binary = publish_record(
        fixture,
        b"support-binary",
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
    let categorical = publish_record(
        fixture,
        b"support-categorical",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(
            MeasurementStatus::MorphologyPrediction,
            &["neoplastic", "inflammatory", "connective"],
        ),
    );
    let support_score = publish_record(
        fixture,
        b"support-score",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata(
            "winning_type_pixel_support",
            MeasurementStatus::MorphologyPrediction,
        ),
    );
    MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::independent(
                    ScalarMarkId::new("mmr_loss").expect("binary ID"),
                    "MMR loss",
                    MeasurementStatus::Measured,
                    binary,
                )
                .expect("binary declaration"),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::probability(
                ProbabilityMarkDeclaration::new(
                    ScalarMarkId::new("winning_type_pixel_support").expect("support ID"),
                    MeasurementStatus::MorphologyPrediction,
                    support_score,
                )
                .expect("support declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                supports,
            )
            .expect("support column"),
            ScalarMarkColumn::histologic_compartment(
                HistologicCompartmentMarkDeclaration::new(
                    vec![
                        "neoplastic".into(),
                        "inflammatory".into(),
                        "connective".into(),
                    ],
                    MeasurementStatus::MorphologyPrediction,
                    categorical,
                )
                .expect("categorical declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                codes,
            )
            .expect("categorical column"),
        ],
        4,
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("mark table")
}

#[test]
fn support_weighting_retains_hard_multiclass_geometry_without_posterior_claims() {
    let mut fixture = fixture();
    let table = table(&mut fixture);
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[5,-1],[5,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("input");
    let categorical = ScalarMarkId::new("histologic_compartment").expect("categorical ID");
    let support_mark = ScalarMarkId::new("winning_type_pixel_support").expect("support ID");
    let config = CategoricalSupportMixingConfig::new(
        1.6,
        CategoricalSupportMixingLimits::new(4, 3, 6, 1 << 20).expect("limits"),
    )
    .expect("config");
    let result = categorical_support_mixing(&input, &window, &categorical, &support_mark, &config)
        .expect("support mixing");

    assert_eq!(result.directed_pair_visits, 6);
    assert!((result.total_support_weighted_pair_mass - 1.625).abs() < 1e-12);
    assert!((result.total_support_retention_fraction - 1.625 / 6.0).abs() < 1e-12);
    assert_eq!(result.matrix.len(), 9);
    assert_eq!(result.matrix[1].hard_directed_pair_count, 1);
    assert!((result.matrix[1].support_weighted_pair_mass - 0.5).abs() < 1e-12);
    assert_eq!(result.matrix[4].hard_directed_pair_count, 2);
    assert!((result.matrix[4].support_weighted_pair_mass - 0.25).abs() < 1e-12);
    assert_eq!(
        result.support_semantics,
        "winner_type_pixel_support_sensitivity_weighting_conditional_on_fixed_hard_labels_not_class_posterior"
    );
    assert!(matches!(
        categorical_support_mixing(
            &input,
            &window,
            &categorical,
            &support_mark,
            &CategoricalSupportMixingConfig::new(
                1.6,
                CategoricalSupportMixingLimits::new(4, 3, 5, 1 << 20).expect("limits"),
            )
            .expect("one-short config"),
        ),
        Err(CategoricalSupportMixingError::PairVisitLimitExceeded {
            observed: 6,
            maximum: 5,
        })
    ));

    let node = CategoricalSupportMixingAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("categorical-support-mixing").expect("node ID"),
        &input,
        &window,
        &categorical,
        &support_mark,
        &config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let root = tempfile::tempdir().expect("durable root");
    let mut durable = DurableProject::open_or_create(
        root.path(),
        DurableProjectLimits::new(64 << 10, 1 << 20, 64, 64 << 10, 1 << 20)
            .expect("durable limits"),
    )
    .expect("durable project");
    let first = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_support_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("miss");
    let second = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_support_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("hit");
    assert_eq!(first.cache_status, CacheStatus::Miss);
    assert_eq!(second.cache_status, CacheStatus::Hit);
    assert_eq!(first.output, second.output);
    assert_eq!(durable.execution_count(), 1);
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.1.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"categorical-support-mixing-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}
