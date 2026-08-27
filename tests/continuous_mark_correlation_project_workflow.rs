#![allow(dead_code)]

use marklab::{
    execute_algorithm_with_store, ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus,
    ContinuousMarkCorrelationAnalysisNode, ContinuousMarkCorrelationConfig,
    ContinuousMarkCorrelationLimits, ContinuousMarkCorrelationResult, DeclaredScalarPatternInput,
    DurableProject, DurableProjectLimits, LocalScheduler, MarkTable, MeasurementStatus,
    MissingnessPolicy, NativeRuntimeProvenance, NodeId, NucleusAreaUm2MarkDeclaration,
    ObservationWindow2D, ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, WorkflowGraph,
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
            b"continuous-mark-correlation-workflow-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    project_path: &std::path::Path,
    durable_limits: DurableProjectLimits,
    seed: u64,
) -> (CacheStatus, ContinuousMarkCorrelationResult, usize) {
    let mut fixture = fixture();
    let mut pattern = fixture.pattern.clone();
    let areas = [1.0_f32, 2.0, 3.0, 4.0];
    pattern.nucleus_area_um2 = Some(Vec::from(areas).into_boxed_slice());
    let binary_provenance = publish_record(
        &mut fixture,
        b"continuous-project-binary-provenance",
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
        b"continuous-project-area-provenance",
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
    let config = ContinuousMarkCorrelationConfig::new(
        mark_id,
        vec![0.5, 1.1],
        31,
        seed,
        0.05,
        ContinuousMarkCorrelationLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits"),
    )
    .expect("config");
    let node = ContinuousMarkCorrelationAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("continuous-mark-correlation").expect("node ID"),
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
        ArtifactSchema::new("marklab.continuous_mark_correlation", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn continuous_correlation_reopens_as_a_verified_hit_and_seed_change_misses() {
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
