use approx::assert_abs_diff_eq;
use marklab::{
    analyze_piecewise_compartment_spatial_pattern, BinaryCompartmentPartition2D,
    CompartmentPartitionLimits, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, InhomogeneousSpatialError, ObservationWindow2D,
    ObservationWindowLimits, Pattern, PatternMeta, PiecewiseCompartmentRole,
    PiecewiseCompartmentSpatialConfig, PiecewiseCompartmentSpatialLimits, SpatialAxis,
};

fn framed_window(
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

fn fixture() -> (Pattern, ObservationWindow2D, BinaryCompartmentPartition2D) {
    let frame = CoordinateFrameId::new("piecewise-compartment-physical-xy").expect("frame ID");
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("physical frame")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registry");
    let observation = framed_window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
        &registry,
        &frame,
    );
    let negative = framed_window(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[5,10],[0,10],[0,0]]]]}"#,
        &registry,
        &frame,
    );
    let positive = framed_window(
        r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#,
        &registry,
        &frame,
    );
    let partition = BinaryCompartmentPartition2D::new(
        observation.clone(),
        "stroma",
        negative,
        "tumor",
        positive,
        CompartmentPartitionLimits::new(64).expect("partition limits"),
    )
    .expect("partition");
    let pattern = Pattern::from_arrays(
        vec![2.0, 4.0, 6.0, 8.0, 9.0],
        vec![5.0; 5],
        vec![0; 5],
        PatternMeta {
            case_id: "piecewise-case".into(),
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
    (pattern, observation, partition)
}

fn config() -> PiecewiseCompartmentSpatialConfig {
    config_with(
        PiecewiseCompartmentSpatialLimits::new(16, 8, 16, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    )
}

fn config_with(limits: PiecewiseCompartmentSpatialLimits) -> PiecewiseCompartmentSpatialConfig {
    PiecewiseCompartmentSpatialConfig::new(vec![1.1, 2.1], 19, 20260829, 0.05, limits)
        .expect("config")
}

#[test]
fn interface_events_and_sparse_compartments_are_rejected_without_reassignment() {
    let (mut interface_pattern, _, partition) = fixture();
    interface_pattern.x_um[1] = 5.0;
    assert!(matches!(
        analyze_piecewise_compartment_spatial_pattern(&interface_pattern, &partition, &config()),
        Err(InhomogeneousSpatialError::PointOnCompartmentInterface { row: 1 })
    ));

    let (mut sparse_pattern, _, partition) = fixture();
    sparse_pattern.x_um[1] = 5.5;
    assert!(matches!(
        analyze_piecewise_compartment_spatial_pattern(&sparse_pattern, &partition, &config()),
        Err(InhomogeneousSpatialError::SparseCompartment {
            ref compartment_id,
            observed: 1,
        }) if compartment_id == "stroma"
    ));
}

#[test]
fn piecewise_compartment_query_and_null_draw_ceilings_are_hard() {
    let (pattern, _, partition) = fixture();
    let query_short = config_with(
        PiecewiseCompartmentSpatialLimits::new(16, 8, 4, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    );
    assert!(matches!(
        analyze_piecewise_compartment_spatial_pattern(&pattern, &partition, &query_short),
        Err(InhomogeneousSpatialError::CompartmentQueryLimitExceeded { maximum: 4 })
    ));

    let draw_short = config_with(
        PiecewiseCompartmentSpatialLimits::new(16, 8, 16, 1_000_000, 94, 1 << 20).expect("limits"),
    );
    assert!(matches!(
        analyze_piecewise_compartment_spatial_pattern(&pattern, &partition, &draw_short),
        Err(InhomogeneousSpatialError::NullDrawLimitExceeded { maximum: 94 })
    ));
}

fn direct_border_k(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    intensities: &[f64],
    radius: f64,
) -> (usize, usize, f64, f64, f64) {
    let mut eligible_centers = 0;
    let mut directed_pairs = 0;
    let mut center_sum = 0.0;
    let mut pair_sum = 0.0;
    for source in 0..pattern.len() {
        if window
            .boundary_distance_um(pattern.x_um[source], pattern.y_um[source])
            .expect("boundary distance")
            < radius
        {
            continue;
        }
        eligible_centers += 1;
        center_sum += 1.0 / intensities[source];
        for target in 0..pattern.len() {
            if source == target {
                continue;
            }
            let distance = (pattern.x_um[source] - pattern.x_um[target])
                .hypot(pattern.y_um[source] - pattern.y_um[target]);
            if distance <= radius {
                directed_pairs += 1;
                pair_sum += 1.0 / (intensities[source] * intensities[target]);
            }
        }
    }
    (
        eligible_centers,
        directed_pairs,
        center_sum,
        pair_sum,
        pair_sum / center_sum,
    )
}

#[test]
fn leave_one_out_piecewise_compartment_intensity_flows_into_border_kl() {
    let (pattern, observation, partition) = fixture();
    let result = analyze_piecewise_compartment_spatial_pattern(&pattern, &partition, &config())
        .expect("piecewise-compartment result");
    let replay = analyze_piecewise_compartment_spatial_pattern(&pattern, &partition, &config())
        .expect("deterministic replay");
    assert_eq!(result, replay);

    assert_eq!(
        result.intensity.estimator,
        "piecewise_constant_binary_compartment"
    );
    assert_eq!(
        result.intensity.cross_fit,
        "leave_one_out_within_compartment"
    );
    assert_eq!(
        result.intensity.boundary_correction,
        "exact_compartment_area"
    );
    assert_eq!(result.intensity.interface_event_policy, "reject");
    assert_eq!(result.intensity.negative.compartment_id, "stroma");
    assert_eq!(result.intensity.negative.event_count, 2);
    assert_eq!(result.intensity.negative.area_um2, 50.0);
    assert_eq!(result.intensity.negative.intensity_per_um2, 0.02);
    assert_eq!(result.intensity.positive.compartment_id, "tumor");
    assert_eq!(result.intensity.positive.event_count, 3);
    assert_eq!(result.intensity.positive.area_um2, 50.0);
    assert_eq!(result.intensity.positive.intensity_per_um2, 0.04);

    let expected_roles = [
        PiecewiseCompartmentRole::Negative,
        PiecewiseCompartmentRole::Negative,
        PiecewiseCompartmentRole::Positive,
        PiecewiseCompartmentRole::Positive,
        PiecewiseCompartmentRole::Positive,
    ];
    let expected_intensities = [0.02, 0.02, 0.04, 0.04, 0.04];
    for (row, point) in result.intensity.point_values.iter().enumerate() {
        assert_eq!(point.row, row);
        assert_eq!(point.role, expected_roles[row]);
        assert_eq!(point.intensity_per_um2, expected_intensities[row]);
        assert_eq!(point.training_point_count, if row < 2 { 1 } else { 2 });
    }

    for (point, radius) in result.curve.iter().zip([1.1, 2.1]) {
        let oracle = direct_border_k(&pattern, &observation, &expected_intensities, radius);
        assert_eq!(point.eligible_centers, oracle.0);
        assert_eq!(point.directed_pairs, oracle.1);
        assert_abs_diff_eq!(
            point.eligible_center_inverse_intensity_sum,
            oracle.2,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(point.inverse_intensity_pair_sum, oracle.3, epsilon = 1e-12);
        assert_abs_diff_eq!(point.k.expect("K"), oracle.4, epsilon = 1e-12);
        assert_abs_diff_eq!(
            point.l.expect("L"),
            (oracle.4 / std::f64::consts::PI).sqrt(),
            epsilon = 1e-12
        );
    }
    assert_eq!(
        result.inference.null_model,
        "fixed_binary_compartment_counts_uniform_within_exact_partition"
    );
    assert_eq!(
        result.inference.randomization_unit,
        "whole_location_pattern_conditioned_on_binary_compartment_counts"
    );
    assert_eq!(result.inference.simulations_completed, 19);
    assert!(result.inference.null_draws >= 5 * 19);
}
