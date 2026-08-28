use approx::assert_abs_diff_eq;
use marklab::{
    compartment_fragmentation, execute_algorithm, ArtifactRef, ArtifactSchema,
    BinaryCompartmentPartition2D, CacheStatus, CompartmentFragmentationAnalysisNode,
    CompartmentFragmentationResult, CompartmentPartitionLimits, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, DurableProject, DurableProjectLimits,
    LocalScheduler, MarklabProject, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, SchedulerLimits, SpatialAxis, WorkflowGraph,
};

const GEOS_ORACLE: &str =
    include_str!("fixtures/compartment_fragmentation/geos_fragmentation_oracle.json");

fn partition(swapped: bool) -> BinaryCompartmentPartition2D {
    let frame_id = CoordinateFrameId::new("fragmentation-frame").expect("frame ID");
    let frame = CoordinateFrame::new(
        frame_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("frame");
    let registry =
        CoordinateRegistry::new(vec![frame], Vec::new(), Vec::new(), Vec::new()).expect("registry");
    let window = |text| {
        ObservationWindow2D::from_geojson_str(text, ObservationWindowLimits::default())
            .expect("window")
            .with_coordinate_frame(&registry, frame_id.clone())
            .expect("bound window")
    };
    let domain =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#);
    let islands = window(
        r#"{"type":"MultiPolygon","coordinates":[
            [[[1,1],[2,1],[2,2],[1,2],[1,1]]],
            [[[4,4],[6,4],[6,6],[4,6],[4,4]]]
        ]}"#,
    );
    let background = window(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[1,1],[2,1],[2,2],[1,2],[1,1]],
            [[4,4],[6,4],[6,6],[4,6],[4,4]]
        ]]}"#,
    );
    let (negative_id, negative, positive_id, positive) = if swapped {
        ("background", background, "islands", islands)
    } else {
        ("islands", islands, "background", background)
    };
    BinaryCompartmentPartition2D::new(
        domain,
        negative_id,
        negative,
        positive_id,
        positive,
        CompartmentPartitionLimits::new(128).expect("partition limits"),
    )
    .expect("partition")
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
            b"compartment-fragmentation-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    swapped: bool,
) -> (CacheStatus, CompartmentFragmentationResult, usize) {
    let partition = partition(swapped);
    let mut project = MarklabProject::new();
    let node = CompartmentFragmentationAnalysisNode::new(
        &mut project,
        NodeId::new("compartment-fragmentation").expect("node ID"),
        &partition,
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
        ArtifactSchema::new("marklab.compartment_fragmentation", 1).expect("schema"),
        runtime(),
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn exact_components_drive_area_concentration_entropy_and_shape_burden() {
    let result = compartment_fragmentation(&partition(false));
    let oracle: serde_json::Value = serde_json::from_str(GEOS_ORACLE).expect("GEOS oracle");
    let islands = &result.negative;
    assert_eq!(islands.compartment_id, "islands");
    assert_eq!(islands.component_count, 2);
    assert_eq!(islands.hole_count, 0);
    assert_eq!(islands.component_areas_um2, vec![1.0, 4.0]);
    assert_abs_diff_eq!(
        islands.largest_component_area_fraction,
        0.8,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        islands.component_area_entropy_nats,
        oracle["islands_component_area_entropy_nats"]
            .as_f64()
            .expect("component entropy"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        islands.normalized_component_area_entropy,
        oracle["islands_normalized_component_area_entropy"]
            .as_f64()
            .expect("normalized component entropy"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(islands.perimeter_area_ratio_per_um, 2.4, epsilon = 1e-12);

    let background = &result.positive;
    assert_eq!(background.compartment_id, "background");
    assert_eq!(background.component_count, 1);
    assert_eq!(background.hole_count, 2);
    assert_eq!(background.component_areas_um2, vec![95.0]);
    assert_eq!(background.largest_component_area_fraction, 1.0);
    assert_eq!(
        background.component_area_entropy_nats.to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        background.normalized_component_area_entropy.to_bits(),
        0.0_f64.to_bits()
    );
    assert_abs_diff_eq!(
        background.perimeter_area_ratio_per_um,
        oracle["background_perimeter_area_ratio_per_um"]
            .as_f64()
            .expect("background ratio"),
        epsilon = 1e-12
    );
    assert_eq!(
        result.cell_mixing_status,
        "unavailable_requires_declared_physical_adjacency_scale_and_typed_cell_rows"
    );
}

#[test]
fn fragmentation_reopens_as_a_hit_and_orientation_changes_identity() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, false);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, false);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, true);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
    assert_eq!(changed.1.negative.compartment_id, "background");
}
