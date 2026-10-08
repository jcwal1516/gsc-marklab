#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use marklab::{
    execute_algorithm_with_store, top1_pair_mixing_bounds, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, HistologicCompartmentMarkDeclaration, LocalScheduler, MarkTable,
    MeasurementStatus, MissingnessPolicy, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, ProbabilityMarkDeclaration, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, Top1PairMixingBoundsAnalysisNode,
    Top1PairMixingBoundsConfig, Top1PairMixingBoundsError, Top1PairMixingBoundsLimits,
    WorkflowGraph,
};
use support::{
    binary_metadata, cell_id_text_bytes, fixture, histologic_compartment_metadata,
    probability_metadata, publish_record, Fixture, MARK_SCHEMA,
};

fn table(fixture: &mut Fixture) -> MarkTable {
    fixture.pattern.x_um = vec![0.0, 1.0, 2.5, 4.0].into_boxed_slice();
    fixture.pattern.y_um = vec![0.0; 4].into_boxed_slice();
    let codes = vec![0, 1, 1, 2];
    let probabilities = vec![0.6, 0.5, 0.7, 0.4];
    fixture.pattern.mark_prob = Some(probabilities.clone().into_boxed_slice());
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
        b"top1-binary",
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
        b"top1-categorical",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(
            MeasurementStatus::MorphologyPrediction,
            &["neoplastic", "inflammatory", "connective"],
        ),
    );
    let probability = publish_record(
        fixture,
        b"top1-confidence",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata(
            "cellvit_winning_class_confidence",
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
                    ScalarMarkId::new("cellvit_winning_class_confidence").expect("probability ID"),
                    MeasurementStatus::MorphologyPrediction,
                    probability,
                )
                .expect("probability declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                probabilities,
            )
            .expect("probability column"),
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
fn top1_confidence_produces_conservative_multiclass_pair_bounds() {
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
    let categorical_mark_id = ScalarMarkId::new("histologic_compartment").expect("class mark");
    let confidence_mark_id =
        ScalarMarkId::new("cellvit_winning_class_confidence").expect("confidence mark");
    let config = Top1PairMixingBoundsConfig::new(
        1.6,
        3,
        Top1PairMixingBoundsLimits::new(4, 3, 6, 54, 1 << 20).expect("limits"),
    )
    .expect("config");
    let result = top1_pair_mixing_bounds(
        &input,
        &window,
        &categorical_mark_id,
        &confidence_mark_id,
        &config,
    )
    .expect("bounds");

    assert_eq!(
        result.class_ids,
        ["neoplastic", "inflammatory", "connective"]
    );
    assert_eq!(result.directed_pair_visits, 6);
    assert_eq!(result.bound_product_evaluations, 54);
    assert_eq!(result.matrix.len(), 9);
    for cell in &result.matrix {
        assert!(cell.lower_expected_pair_mass <= cell.upper_expected_pair_mass);
        assert!(cell.lower_pair_fraction <= cell.upper_pair_fraction);
    }
    let neoplastic_to_inflammatory = &result.matrix[1];
    assert!((neoplastic_to_inflammatory.lower_expected_pair_mass - 0.44).abs() < 1e-6);
    assert!((neoplastic_to_inflammatory.upper_expected_pair_mass - 1.4).abs() < 1e-6);
    assert!(matches!(
        top1_pair_mixing_bounds(
            &input,
            &window,
            &categorical_mark_id,
            &confidence_mark_id,
            &Top1PairMixingBoundsConfig::new(
                1.6,
                3,
                Top1PairMixingBoundsLimits::new(4, 3, 6, 53, 1 << 20).expect("limits"),
            )
            .expect("one-short config"),
        ),
        Err(Top1PairMixingBoundsError::BoundProductLimitExceeded {
            observed: 54,
            maximum: 53
        })
    ));

    let node = Top1PairMixingBoundsAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("top1-pair-bounds").expect("node ID"),
        &input,
        &window,
        &categorical_mark_id,
        &confidence_mark_id,
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
        ArtifactSchema::new("marklab.top1_pair_mixing_bounds", 1).expect("schema"),
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
        ArtifactSchema::new("marklab.top1_pair_mixing_bounds", 1).expect("schema"),
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
            b"top1-pair-mixing-bounds-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}
