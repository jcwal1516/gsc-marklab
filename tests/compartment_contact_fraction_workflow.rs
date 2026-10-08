use approx::assert_abs_diff_eq;
use marklab::{
    compartment_contact_fractions, execute_algorithm, ArtifactRef, ArtifactSchema,
    BinaryCompartmentPartition2D, CacheStatus, CompartmentContactAnalysisNode,
    CompartmentContactResult, CompartmentPartitionLimits, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, DurableProject, DurableProjectLimits,
    LocalScheduler, MarklabProject, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, SchedulerLimits, SpatialAxis, WorkflowGraph,
};

const GEOS_ORACLE: &str = include_str!("fixtures/compartment_partition/geos_rectangle_oracle.json");

fn partition(swapped: bool) -> BinaryCompartmentPartition2D {
    let frame_id = CoordinateFrameId::new("contact-frame").expect("frame ID");
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
    let domain = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
    );
    let left =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[5,10],[0,10],[0,0]]]]}"#);
    let right =
        window(r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#);
    let (negative_id, negative, positive_id, positive) = if swapped {
        ("tumor", right, "stroma", left)
    } else {
        ("stroma", left, "tumor", right)
    };
    BinaryCompartmentPartition2D::new(
        domain,
        negative_id,
        negative,
        positive_id,
        positive,
        CompartmentPartitionLimits::new(64).expect("partition limits"),
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
            b"compartment-contact-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    swapped: bool,
) -> (CacheStatus, CompartmentContactResult, usize) {
    let partition = partition(swapped);
    let mut project = MarklabProject::new();
    let node = CompartmentContactAnalysisNode::new(
        &mut project,
        NodeId::new("compartment-contact").expect("node ID"),
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
        ArtifactSchema::new("marklab.compartment_contact_fraction", 1).expect("schema"),
        runtime(),
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn shared_interface_contact_uses_complete_compartment_boundary_denominator() {
    let result = compartment_contact_fractions(&partition(false));
    let oracle: serde_json::Value = serde_json::from_str(GEOS_ORACLE).expect("GEOS oracle");
    assert_eq!(
        result.denominator,
        "complete_compartment_boundary_including_tissue_edge_and_shared_interface"
    );
    assert_eq!(result.negative.compartment_id, "stroma");
    assert_eq!(result.positive.compartment_id, "tumor");
    for contact in [&result.negative, &result.positive] {
        assert_abs_diff_eq!(
            contact.shared_interface_length_um,
            oracle["interface_length_um"].as_f64().expect("interface"),
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            contact.outer_tissue_boundary_length_um,
            oracle["compartment_outer_boundary_length_um"]
                .as_f64()
                .expect("outer boundary"),
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            contact.denominator_boundary_length_um,
            oracle["compartment_boundary_length_um"]
                .as_f64()
                .expect("denominator"),
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(contact.contact_fraction, 1.0 / 3.0, epsilon = 1e-12);
    }
}

#[test]
fn contact_fraction_reopens_as_a_hit_and_orientation_changes_identity() {
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
    assert_eq!(changed.1.negative.compartment_id, "tumor");
}
