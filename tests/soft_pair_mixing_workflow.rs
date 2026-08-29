#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use std::collections::BTreeMap;

use approx::assert_abs_diff_eq;
use marklab::{
    execute_algorithm_with_store, soft_pair_mixing, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, LocalScheduler, MarkTable, MeasurementStatus, MissingnessPolicy,
    NativeRuntimeProvenance, NodeId, ObservationWindow2D, ObservationWindowLimits,
    ProbabilitySimplexMarkDeclaration, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality,
    ScalarMarkUnit, SchedulerLimits, SoftPairMixingAnalysisNode, SoftPairMixingConfig,
    SoftPairMixingError, SoftPairMixingLimits, WorkflowGraph,
};
use support::{binary_metadata, fixture, publish_record, MARK_SCHEMA};

fn metadata() -> BTreeMap<String, String> {
    let levels = ["a", "b", "c"];
    let digest = marklab::ContentDigest::from_framed(levels.iter().map(|level| level.as_bytes()));
    BTreeMap::from([
        ("levels_count".into(), "3".into()),
        ("levels_digest".into(), digest.to_string()),
        ("mark_id".into(), "cellvit_probabilities".into()),
        ("mark_label".into(), "CellViT probabilities".into()),
        ("measurement_status".into(), "morphology_prediction".into()),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "probability_simplex".into()),
        ("value_kind".into(), "probability_simplex".into()),
    ])
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"soft-pair-mixing-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

#[test]
fn complete_probability_rows_produce_exact_multiclass_pair_mixing_and_replay() {
    let mut fixture = fixture();
    fixture.pattern.x_um = vec![0.0, 1.0, 10.0, 20.0].into_boxed_slice();
    fixture.pattern.y_um = vec![0.0; 4].into_boxed_slice();
    let binary_provenance = publish_record(
        &mut fixture,
        b"soft-pair-mixing-binary",
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
    let provenance = publish_record(
        &mut fixture,
        b"soft-pair-mixing-simplex",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        metadata(),
    );
    let mark_id = ScalarMarkId::new("cellvit_probabilities").expect("mark ID");
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
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::probability_simplex(
                ProbabilitySimplexMarkDeclaration::new(
                    mark_id.clone(),
                    "CellViT probabilities",
                    vec!["a".into(), "b".into(), "c".into()],
                    MeasurementStatus::MorphologyPrediction,
                    provenance,
                )
                .expect("declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::ProbabilitySimplex,
                MissingnessPolicy::NotPermitted,
                vec![
                    vec![1.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0],
                    vec![0.5, 0.0, 0.5],
                    vec![0.0, 0.0, 1.0],
                ],
            )
            .expect("simplex"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("table");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("input");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[21,-1],[21,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("frame");
    let config = SoftPairMixingConfig::new(
        1.5,
        SoftPairMixingLimits::new(4, 3, 12, 2, 54, 1 << 20).expect("limits"),
    )
    .expect("config");
    let direct = soft_pair_mixing(&input, &window, &mark_id, &config).expect("mixing");
    assert_eq!(direct.directed_pair_visits, 2);
    assert_eq!(direct.matrix.len(), 9);
    let cell = |source: &str, target: &str| {
        direct
            .matrix
            .iter()
            .find(|cell| cell.source_class_id == source && cell.target_class_id == target)
            .expect("matrix cell")
    };
    assert_abs_diff_eq!(cell("a", "b").pair_probability.expect("observed"), 0.5);
    assert_abs_diff_eq!(cell("b", "a").pair_probability.expect("observed"), 0.5);
    assert_abs_diff_eq!(cell("a", "b").random_label_probability, 0.125);
    assert_abs_diff_eq!(cell("a", "b").enrichment_ratio.expect("ratio"), 4.0);
    assert_abs_diff_eq!(cell("a", "a").random_label_probability, 1.0 / 12.0);
    assert_eq!(cell("b", "b").enrichment_ratio, None);

    assert!(matches!(
        soft_pair_mixing(
            &input,
            &window,
            &mark_id,
            &SoftPairMixingConfig::new(
                1.5,
                SoftPairMixingLimits::new(4, 3, 12, 1, 54, 1 << 20).expect("one-short limits"),
            )
            .expect("one-short config"),
        ),
        Err(SoftPairMixingError::PairVisitLimitExceeded {
            observed: 2,
            maximum: 1
        })
    ));

    let node = SoftPairMixingAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("soft-pair-mixing").expect("node ID"),
        &input,
        &window,
        &mark_id,
        &config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let directory = tempfile::tempdir().expect("tempdir");
    let mut durable = DurableProject::open_or_create(
        directory.path(),
        DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
            .expect("durable limits"),
    )
    .expect("project");
    let first = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.soft_pair_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("first");
    let second = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.soft_pair_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("second");
    assert_eq!(first.cache_status, CacheStatus::Miss);
    assert_eq!(second.cache_status, CacheStatus::Hit);
    assert_eq!(first.output, second.output);
    assert_eq!(durable.execution_count(), 1);
}
