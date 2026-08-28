use marklab::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheStatus, DurableProject,
    DurableProjectLimits, InhomogeneousPairCorrelationAnalysisNode,
    InhomogeneousPairCorrelationConfig, InhomogeneousPairCorrelationResult,
    InhomogeneousSpatialConfig, InhomogeneousSpatialLimits, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeId, ObservationWindow2D, ObservationWindowLimits, Pattern,
    PatternMeta, SchedulerLimits, WorkflowGraph,
};

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"inhomogeneous-pair-correlation-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    seed: u64,
) -> (CacheStatus, InhomogeneousPairCorrelationResult, usize) {
    let pattern = Pattern::from_arrays(
        vec![2.0, 3.0, 7.0, 9.0],
        vec![2.0, 2.0, 7.0, 7.0],
        vec![0; 4],
        PatternMeta {
            case_id: "durable-inhomogeneous-g".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("inhomogeneous-g-slide".into()),
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
    let intensity = InhomogeneousSpatialConfig::new(
        vec![1.0],
        2.0,
        [10, 10],
        19,
        seed,
        0.05,
        1e-12,
        InhomogeneousSpatialLimits::new(16, 16, 1_000, 1_000_000, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    )
    .expect("intensity config");
    let config =
        InhomogeneousPairCorrelationConfig::new(intensity, 0.5).expect("pair configuration");
    let mut project = MarklabProject::new();
    let node = InhomogeneousPairCorrelationAnalysisNode::new(
        &mut project,
        NodeId::new("inhomogeneous-pair-correlation").expect("node ID"),
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
    let limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");
    let mut durable = DurableProject::open_or_create(path, limits).expect("durable project");
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.inhomogeneous_pair_correlation", 1).expect("schema"),
        runtime(),
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn inhomogeneous_g_reopens_as_a_verified_hit_and_seed_change_misses() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 20260827);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, 20260827);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, 20260828);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
