use approx::assert_abs_diff_eq;
use marklab::{
    BinaryCompartmentPartition2D, CompartmentPartitionError, CompartmentPartitionLimits,
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    ObservationWindow2D, ObservationWindowLimits, SpatialAxis,
};

const GEOS_ORACLE: &str = include_str!("fixtures/compartment_partition/geos_rectangle_oracle.json");

fn limits() -> ObservationWindowLimits {
    ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits")
}

fn frame(value: &str) -> (CoordinateRegistry, CoordinateFrameId) {
    let id = CoordinateFrameId::new(value).expect("frame ID");
    let physical = CoordinateFrame::new(
        id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("physical frame");
    (
        CoordinateRegistry::new(vec![physical], Vec::new(), Vec::new(), Vec::new())
            .expect("registry"),
        id,
    )
}

fn window(
    text: &str,
    registry: &CoordinateRegistry,
    frame: &CoordinateFrameId,
) -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(text, limits())
        .expect("window")
        .with_coordinate_frame(registry, frame.clone())
        .expect("bound window")
}

fn fixture(
    boundary_limit: usize,
) -> Result<BinaryCompartmentPartition2D, CompartmentPartitionError> {
    let (registry, frame) = frame("slide-physical-xy");
    let domain = window(
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
    BinaryCompartmentPartition2D::new(
        domain,
        "stroma",
        stroma,
        "tumor",
        tumor,
        CompartmentPartitionLimits::new(boundary_limit).expect("partition limits"),
    )
}

#[test]
fn aligned_binary_partition_has_oriented_interface_distance_not_tissue_edge_distance() {
    let partition = fixture(64).expect("partition");
    let descriptor = partition.descriptor();
    let oracle: serde_json::Value = serde_json::from_str(GEOS_ORACLE).expect("GEOS oracle");
    assert_eq!(descriptor.negative_compartment_id, "stroma");
    assert_eq!(descriptor.positive_compartment_id, "tumor");
    assert_eq!(descriptor.interface_segment_count, 1);
    assert_eq!(descriptor.validated_boundary_segment_count, 14);
    assert_abs_diff_eq!(
        descriptor.interface_length_um,
        oracle["interface_length_um"]
            .as_f64()
            .expect("interface length"),
        epsilon = 1e-12
    );
    assert_eq!(
        descriptor.negative_area_um2,
        oracle["negative_area_um2"].as_f64().expect("negative area")
    );
    assert_eq!(
        descriptor.positive_area_um2,
        oracle["positive_area_um2"].as_f64().expect("positive area")
    );
    assert_eq!(
        descriptor.observation_area_um2,
        oracle["observation_area_um2"]
            .as_f64()
            .expect("domain area")
    );
    assert_eq!(descriptor.coordinate_frame_id.as_str(), "slide-physical-xy");
    assert_eq!(descriptor.logical_digest.to_string().len(), 64);

    assert_abs_diff_eq!(
        partition
            .signed_interface_distance_um(2.0, 5.0)
            .expect("stroma distance"),
        -oracle["query_distances_um"]["stroma"]
            .as_f64()
            .expect("stroma distance"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        partition
            .signed_interface_distance_um(7.0, 5.0)
            .expect("tumor distance"),
        oracle["query_distances_um"]["tumor"]
            .as_f64()
            .expect("tumor distance"),
        epsilon = 1e-12
    );
    assert_eq!(
        partition
            .signed_interface_distance_um(5.0, 4.0)
            .expect("interface")
            .to_bits(),
        0.0_f64.to_bits()
    );
    assert_abs_diff_eq!(
        partition
            .signed_interface_distance_um(0.0, 5.0)
            .expect("tissue edge remains stroma"),
        -oracle["query_distances_um"]["tissue_edge"]
            .as_f64()
            .expect("tissue-edge query distance"),
        epsilon = 1e-12
    );
    assert_eq!(
        partition.signed_interface_distance_um(-0.1, 5.0),
        Err(CompartmentPartitionError::OutsideObservationWindow)
    );
    assert_eq!(
        partition.signed_interface_distance_um(f64::NAN, 5.0),
        Err(CompartmentPartitionError::NonFiniteQueryPoint)
    );
}

#[test]
fn gaps_overlaps_frame_drift_and_unaligned_boundary_segments_fail() {
    let (registry, frame_id) = frame("partition-frame");
    let domain = || {
        window(
            r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
            &registry,
            &frame_id,
        )
    };
    let left = |right: f64| {
        window(
            &format!(
                r#"{{"type":"MultiPolygon","coordinates":[[[[0,0],[{right},0],[{right},10],[0,10],[0,0]]]]}}"#
            ),
            &registry,
            &frame_id,
        )
    };
    let right = |left: f64| {
        window(
            &format!(
                r#"{{"type":"MultiPolygon","coordinates":[[[[{left},0],[10,0],[10,10],[{left},10],[{left},0]]]]}}"#
            ),
            &registry,
            &frame_id,
        )
    };
    let partition_limits = CompartmentPartitionLimits::new(64).expect("partition limits");

    for (negative, positive) in [(left(4.0), right(5.0)), (left(6.0), right(5.0))] {
        assert!(matches!(
            BinaryCompartmentPartition2D::new(
                domain(),
                "negative",
                negative,
                "positive",
                positive,
                partition_limits,
            ),
            Err(CompartmentPartitionError::NotExactPartition { .. })
        ));
    }

    let unsegmented_domain = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        &registry,
        &frame_id,
    );
    assert!(matches!(
        BinaryCompartmentPartition2D::new(
            unsegmented_domain,
            "stroma",
            left(5.0),
            "tumor",
            right(5.0),
            partition_limits,
        ),
        Err(CompartmentPartitionError::NotExactPartition { .. })
    ));

    let (other_registry, other_frame) = frame("other-frame");
    let other = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#,
        &other_registry,
        &other_frame,
    );
    assert!(matches!(
        BinaryCompartmentPartition2D::new(
            domain(),
            "stroma",
            left(5.0),
            "tumor",
            other,
            partition_limits,
        ),
        Err(CompartmentPartitionError::CoordinateFrameMismatch)
    ));

    assert!(matches!(
        fixture(13),
        Err(CompartmentPartitionError::BoundarySegmentLimitExceeded {
            observed: 14,
            maximum: 13,
        })
    ));

    let unbound = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
        limits(),
    )
    .expect("unbound domain");
    assert!(matches!(
        BinaryCompartmentPartition2D::new(
            unbound,
            "stroma",
            left(5.0),
            "tumor",
            right(5.0),
            partition_limits,
        ),
        Err(CompartmentPartitionError::MissingCoordinateFrame)
    ));

    assert!(matches!(
        BinaryCompartmentPartition2D::new(
            domain(),
            "tumor",
            left(5.0),
            "tumor",
            right(5.0),
            partition_limits,
        ),
        Err(CompartmentPartitionError::DuplicateCompartmentId)
    ));
}

#[test]
fn disconnected_compartments_without_a_biological_interface_are_unavailable() {
    let (registry, frame) = frame("disconnected-frame");
    let domain = window(
        r#"{"type":"MultiPolygon","coordinates":[
            [[[0,0],[2,0],[2,2],[0,2],[0,0]]],
            [[[4,0],[6,0],[6,2],[4,2],[4,0]]]
        ]}"#,
        &registry,
        &frame,
    );
    let negative = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]]]]}"#,
        &registry,
        &frame,
    );
    let positive = window(
        r#"{"type":"MultiPolygon","coordinates":[[[[4,0],[6,0],[6,2],[4,2],[4,0]]]]}"#,
        &registry,
        &frame,
    );
    assert!(matches!(
        BinaryCompartmentPartition2D::new(
            domain,
            "negative",
            negative,
            "positive",
            positive,
            CompartmentPartitionLimits::new(64).expect("partition limits"),
        ),
        Err(CompartmentPartitionError::NoSharedInterface)
    ));
}
