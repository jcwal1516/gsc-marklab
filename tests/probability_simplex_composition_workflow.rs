#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use std::collections::BTreeMap;

use marklab::{
    execute_algorithm_with_store, soft_class_composition, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, LocalScheduler, MarkTable, MeasurementStatus, MissingnessPolicy,
    NativeRuntimeProvenance, NodeId, ProbabilitySimplexMarkDeclaration, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SchedulerLimits,
    SoftClassCompositionAnalysisNode, SoftClassCompositionError, SoftClassCompositionLimits,
    SoftClassCompositionResult, WorkflowGraph,
};
use support::{binary_metadata, fixture, publish_record, MARK_SCHEMA};

const PYTHON_ORACLE: &str =
    include_str!("fixtures/probability_simplex/python_composition_oracle.json");

fn simplex_metadata(levels: &[&str]) -> BTreeMap<String, String> {
    let digest = marklab::ContentDigest::from_framed(levels.iter().map(|level| level.as_bytes()));
    BTreeMap::from([
        ("levels_count".into(), levels.len().to_string()),
        ("levels_digest".into(), digest.to_string()),
        ("mark_id".into(), "cellvit_annotation_probabilities".into()),
        (
            "mark_label".into(),
            "CellViT annotation probabilities".into(),
        ),
        ("measurement_status".into(), "morphology_prediction".into()),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "probability_simplex".into()),
        ("value_kind".into(), "probability_simplex".into()),
    ])
}

fn limits(bytes: usize) -> SoftClassCompositionLimits {
    SoftClassCompositionLimits::new(4, 3, 12, bytes).expect("composition limits")
}

fn table(fixture: &mut support::Fixture) -> (MarkTable, ScalarMarkId) {
    let binary_provenance = publish_record(
        fixture,
        b"simplex-binary-provenance",
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
    let simplex_provenance = publish_record(
        fixture,
        b"simplex-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        simplex_metadata(&["neoplastic", "inflammatory", "connective"]),
    );
    let simplex_id =
        ScalarMarkId::new("cellvit_annotation_probabilities").expect("simplex mark ID");
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
                    simplex_id.clone(),
                    "CellViT annotation probabilities",
                    vec![
                        "neoplastic".into(),
                        "inflammatory".into(),
                        "connective".into(),
                    ],
                    MeasurementStatus::MorphologyPrediction,
                    simplex_provenance,
                )
                .expect("simplex declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::ProbabilitySimplex,
                MissingnessPolicy::NotPermitted,
                vec![
                    vec![1.0, 0.0, 0.0],
                    vec![0.5, 0.5, 0.0],
                    vec![0.0, 0.25, 0.75],
                    vec![0.0, 0.0, 1.0],
                ],
            )
            .expect("simplex column"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("mark table");
    (table, simplex_id)
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
            b"soft-class-composition-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    maximum_bytes: usize,
) -> (CacheStatus, SoftClassCompositionResult, usize) {
    let mut fixture = fixture();
    let (table, mark_id) = table(&mut fixture);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let limits = SoftClassCompositionLimits::new(4, 3, 12, maximum_bytes).expect("limits");
    let node = SoftClassCompositionAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("soft-class-composition").expect("node ID"),
        &input,
        &mark_id,
        &limits,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let durable_limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");
    let mut durable = DurableProject::open_or_create(path, durable_limits).expect("project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.soft_class_composition", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn complete_probability_rows_flow_to_soft_composition_and_entropy_without_thresholding() {
    let mut fixture = fixture();
    let (table, simplex_id) = table(&mut fixture);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let result =
        soft_class_composition(&input, &simplex_id, &limits(1 << 20)).expect("soft composition");
    let replay = soft_class_composition(&input, &simplex_id, &limits(1 << 20))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("Python oracle");
    assert_eq!(result.mark_id, simplex_id.as_str());
    assert_eq!(result.measurement_status, "morphology_prediction");
    assert_eq!(result.row_count, 4);
    assert_eq!(result.class_count, 3);
    assert_eq!(result.classes.len(), 3);
    for (class, expected) in result.classes.iter().zip(
        oracle["mean_probabilities"]
            .as_array()
            .expect("mean probabilities"),
    ) {
        assert_eq!(
            class.mean_probability,
            expected.as_f64().expect("probability")
        );
    }
    assert_eq!(
        result.mean_row_entropy_nats,
        oracle["mean_row_entropy_nats"]
            .as_f64()
            .expect("row entropy")
    );
    assert_eq!(
        result.aggregate_composition_entropy_nats,
        oracle["aggregate_composition_entropy_nats"]
            .as_f64()
            .expect("aggregate entropy")
    );
    assert_eq!(result.maximum_row_sum_absolute_error, 0.0);
    assert!(matches!(
        soft_class_composition(
            &input,
            &simplex_id,
            &limits(result.estimated_storage_bytes - 1),
        ),
        Err(SoftClassCompositionError::RetainedByteLimitExceeded {
            required,
            maximum,
        }) if required == maximum + 1
    ));
    assert!(matches!(
        soft_class_composition(
            &input,
            &simplex_id,
            &SoftClassCompositionLimits::new(3, 3, 9, 1 << 20).expect("point limit"),
        ),
        Err(SoftClassCompositionError::PointLimitExceeded {
            observed: 4,
            maximum: 3,
        })
    ));
    assert!(matches!(
        soft_class_composition(
            &input,
            &simplex_id,
            &SoftClassCompositionLimits::new(4, 2, 8, 1 << 20).expect("class limit"),
        ),
        Err(SoftClassCompositionError::ClassLimitExceeded {
            observed: 3,
            maximum: 2,
        })
    ));
}

#[test]
fn malformed_simplex_rows_and_actual_resource_ceilings_fail() {
    let declaration = |artifact| {
        ProbabilitySimplexMarkDeclaration::new(
            ScalarMarkId::new("cellvit_annotation_probabilities").expect("mark ID"),
            "CellViT annotation probabilities",
            vec!["a".into(), "b".into()],
            MeasurementStatus::MorphologyPrediction,
            artifact,
        )
        .expect("declaration")
    };
    let mut fixture = fixture();
    let provenance = publish_record(
        &mut fixture,
        b"invalid-simplex-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        simplex_metadata(&["a", "b"]),
    );
    assert!(matches!(
        ScalarMarkColumn::probability_simplex(
            declaration(provenance),
            ScalarMarkModality::Morphology,
            ScalarMarkUnit::ProbabilitySimplex,
            MissingnessPolicy::NotPermitted,
            vec![vec![0.8, 0.3]],
        ),
        Err(marklab::DeclaredScalarInputError::InvalidProbabilitySimplexRow { row: 0, .. })
    ));
    assert!(matches!(
        ScalarMarkColumn::probability_simplex(
            declaration(provenance),
            ScalarMarkModality::Morphology,
            ScalarMarkUnit::ProbabilitySimplex,
            MissingnessPolicy::NotPermitted,
            vec![vec![1.0]],
        ),
        Err(
            marklab::DeclaredScalarInputError::InvalidProbabilitySimplexShape {
                row: 0,
                expected: 2,
                observed: 1,
            }
        )
    ));
    assert_eq!(
        SoftClassCompositionLimits::new(4, 3, 11, 1 << 20),
        Err(SoftClassCompositionError::InvalidResourceLimit)
    );
}

#[test]
fn soft_composition_reopens_as_a_hit_and_limits_change_identity() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 1 << 20);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, 1 << 20);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, (1 << 20) + 1);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
