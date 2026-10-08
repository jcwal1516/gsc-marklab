#![allow(dead_code)]

use marklab::{
    execute_algorithm_with_store, ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits, LocalScheduler, MarkTable,
    MeasurementStatus, MissingnessPolicy, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, ProbabilityMarkDeclaration, ProbabilityPairAnalysisNode,
    ProbabilityPairConfig, ProbabilityPairLimits, ProbabilityPairResult, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, WorkflowGraph,
};
use tempfile::TempDir;

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"probability-pair-workflow-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    project_path: &std::path::Path,
    durable_limits: DurableProjectLimits,
    seed: u64,
) -> (CacheStatus, ProbabilityPairResult, usize) {
    let mut fixture = fixture();
    let mut pattern = fixture.pattern.clone();
    let probabilities = [1.0_f32, 0.5, 0.0, 1.0];
    pattern.mark_prob = Some(Vec::from(probabilities).into_boxed_slice());
    let binary_provenance = publish_record(
        &mut fixture,
        b"probability-project-binary-provenance",
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
        b"probability-project-provenance",
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
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
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
    let config = ProbabilityPairConfig::new(
        probability_mark_id,
        vec![0.5, 1.1],
        31,
        seed,
        0.05,
        ProbabilityPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("config");
    let node = ProbabilityPairAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("probability-pair").expect("node ID"),
        &input,
        &window,
        &config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let mut durable =
        DurableProject::open_or_create(project_path, durable_limits).expect("project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.probability_pair", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn probability_pair_reopens_as_a_verified_hit_and_seed_change_misses() {
    let root = TempDir::new().expect("root");
    let project = root.path().join("project");
    let limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");

    let first = run_once(&project, limits, 20260827);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&project, limits, 20260827);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1, "a hit must not append an execution");
    let changed = run_once(&project, limits, 20260828);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
