use marklab::{
    execute_algorithm, ArtifactRef, ArtifactSchema, BinaryCompartmentPartition2D, CacheStatus,
    CompartmentPartitionLimits, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, DurableProject, DurableProjectLimits, LocalScheduler,
    MarklabProject, NativeRuntimeProvenance, NodeId, ObservationWindow2D, ObservationWindowLimits,
    Pattern, PatternMeta, PiecewiseCompartmentSpatialAnalysisNode,
    PiecewiseCompartmentSpatialConfig, PiecewiseCompartmentSpatialLimits,
    PiecewiseCompartmentSpatialResult, SchedulerLimits, SpatialAxis, WorkflowGraph,
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
            b"piecewise-compartment-spatial-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn window(
    text: &str,
    registry: &CoordinateRegistry,
    frame: &CoordinateFrameId,
) -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        text,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
    .with_coordinate_frame(registry, frame.clone())
    .expect("framed window")
}

fn run_once(
    path: &std::path::Path,
    seed: u64,
    swap_roles: bool,
) -> (CacheStatus, PiecewiseCompartmentSpatialResult, usize) {
    let frame = CoordinateFrameId::new("piecewise-compartment-project-xy").expect("frame ID");
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("frame")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registry");
    let observation = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
        &registry,
        &frame,
    );
    let stroma = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[5,10],[0,10],[0,0]]]]}"#,
        &registry,
        &frame,
    );
    let tumor = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#,
        &registry,
        &frame,
    );
    let (negative_id, negative, positive_id, positive) = if swap_roles {
        ("tumor", tumor, "stroma", stroma)
    } else {
        ("stroma", stroma, "tumor", tumor)
    };
    let partition = BinaryCompartmentPartition2D::new(
        observation,
        negative_id,
        negative,
        positive_id,
        positive,
        CompartmentPartitionLimits::new(64).expect("partition limits"),
    )
    .expect("partition");
    let pattern = Pattern::from_arrays(
        vec![2.0, 4.0, 6.0, 8.0, 9.0],
        vec![5.0; 5],
        vec![0; 5],
        PatternMeta {
            case_id: "durable-piecewise-compartment".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("piecewise-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    let config = PiecewiseCompartmentSpatialConfig::new(
        vec![1.1, 2.1],
        19,
        seed,
        0.05,
        PiecewiseCompartmentSpatialLimits::new(16, 8, 16, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    )
    .expect("config");
    let mut project = MarklabProject::new();
    let node = PiecewiseCompartmentSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("piecewise-compartment-spatial").expect("node ID"),
        &pattern,
        &partition,
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
        ArtifactSchema::new("marklab.piecewise_compartment_spatial", 1).expect("schema"),
        runtime(),
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn piecewise_compartment_spatial_reopens_as_a_verified_hit_and_seed_change_misses() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 20260829, false);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, 20260829, false);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, 20260830, false);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
    let swapped = run_once(&path, 20260829, true);
    assert_eq!(swapped.0, CacheStatus::Miss);
    assert_eq!(swapped.2, 3);
    assert_eq!(swapped.1.intensity.negative.compartment_id, "tumor");
}
