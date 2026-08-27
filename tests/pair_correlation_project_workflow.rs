use marklab::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheStatus, ClassicalSpatialLimits,
    DurableProject, DurableProjectLimits, HomogeneousPairCorrelationAnalysisNode,
    HomogeneousPairCorrelationConfig, HomogeneousPairCorrelationResult, LocalScheduler,
    MarklabProject, NativeRuntimeProvenance, NodeId, ObservationWindow2D, ObservationWindowLimits,
    Pattern, PatternMeta, SchedulerLimits, WorkflowGraph,
};
use tempfile::TempDir;

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"pair-correlation-workflow-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    project_path: &std::path::Path,
    durable_limits: DurableProjectLimits,
    seed: u64,
) -> (CacheStatus, HomogeneousPairCorrelationResult, usize) {
    let pattern = Pattern::from_arrays(
        vec![3.0, 4.0, 7.0],
        vec![5.0, 5.0, 5.0],
        vec![0; 3],
        PatternMeta {
            case_id: "durable-pair-correlation".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("pair-correlation-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window");
    let config = HomogeneousPairCorrelationConfig::new(
        vec![1.0, 2.0],
        0.5,
        19,
        seed,
        0.05,
        ClassicalSpatialLimits::new(16, 16, 100_000, 100_000, 1 << 20).expect("limits"),
    )
    .expect("config");
    let mut project = MarklabProject::new();
    let node = HomogeneousPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("pair-correlation").expect("node ID"),
        &pattern,
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
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.homogeneous_pair_correlation", 1).expect("schema"),
        runtime(),
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn pair_correlation_reopens_as_a_verified_hit_and_seed_change_misses() {
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
