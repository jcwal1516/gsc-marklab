use approx::assert_abs_diff_eq;
use marklab::{
    analyze_inhomogeneous_spatial_pattern, InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    InhomogeneousSpatialPointStatus, ObservationWindow2D, ObservationWindowLimits, Pattern,
    PatternMeta,
};

const PYTHON_ORACLE: &str =
    include_str!("fixtures/inhomogeneous_spatial/python_rectangle_oracle.json");

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![2.0, 3.0, 7.0, 9.0],
        vec![2.0, 2.0, 7.0, 7.0],
        vec![0; 4],
        PatternMeta {
            case_id: "inhomogeneous-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("inhomogeneous-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn config(seed: u64) -> InhomogeneousSpatialConfig {
    config_with(
        seed,
        1e-12,
        InhomogeneousSpatialLimits::new(16, 16, 1_000, 1_000_000, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    )
}

fn config_with(
    seed: u64,
    minimum_intensity_per_um2: f64,
    limits: InhomogeneousSpatialLimits,
) -> InhomogeneousSpatialConfig {
    InhomogeneousSpatialConfig::new(
        vec![1.1],
        2.0,
        [10, 10],
        19,
        seed,
        0.05,
        minimum_intensity_per_um2,
        limits,
    )
    .expect("configuration")
}

#[test]
fn gaussian_leave_one_out_intensity_flows_into_border_inhomogeneous_kl() {
    let result = analyze_inhomogeneous_spatial_pattern(&pattern(), &window(), &config(20260827))
        .expect("inhomogeneous result");
    let replay = analyze_inhomogeneous_spatial_pattern(&pattern(), &window(), &config(20260827))
        .expect("deterministic replay");
    assert_eq!(result, replay);

    assert_eq!(result.intensity.estimator, "gaussian_kernel");
    assert_eq!(
        result.intensity.cross_fit,
        "leave_one_out_n_over_n_minus_one"
    );
    assert_eq!(
        result.intensity.boundary_correction,
        "deterministic_cell_center_quadrature"
    );
    assert_eq!(result.intensity.bandwidth_um, 2.0);
    assert_eq!(result.intensity.integration_grid, [10, 10]);
    assert_eq!(result.intensity.retained_probe_count, 100);
    assert_eq!(result.intensity.fixed_grid.len(), 100);
    assert_eq!(result.intensity.fixed_grid_digest.len(), 64);
    assert_abs_diff_eq!(
        result.intensity.fixed_grid_total_mass,
        result
            .intensity
            .fixed_grid
            .iter()
            .map(|point| point.cell_mass)
            .sum::<f64>(),
        epsilon = 1e-12
    );
    assert_eq!(result.intensity.probe_spacing_um, [1.0, 1.0]);
    assert_eq!(result.intensity.point_values.len(), 4);

    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("oracle JSON");
    for (index, point) in result.intensity.point_values.iter().enumerate() {
        assert_eq!(point.row, index);
        assert_abs_diff_eq!(
            point.boundary_mass,
            oracle["boundary_masses"][index]
                .as_f64()
                .expect("boundary mass"),
            epsilon = 1e-14
        );
        assert_abs_diff_eq!(
            point.intensity_per_um2,
            oracle["intensities"][index].as_f64().expect("intensity"),
            epsilon = 1e-14
        );
        assert_eq!(point.training_point_count, 3);
    }

    let point = &result.curve[0];
    assert_eq!(point.status, InhomogeneousSpatialPointStatus::Available);
    assert_eq!(point.eligible_centers, 3);
    assert_eq!(point.directed_pairs, 2);
    assert_abs_diff_eq!(
        point.inverse_intensity_pair_sum,
        oracle["inverse_intensity_pair_sum"]
            .as_f64()
            .expect("pair sum"),
        epsilon = 1e-10
    );
    assert_abs_diff_eq!(
        point.eligible_center_inverse_intensity_sum,
        oracle["eligible_center_inverse_intensity_sum"]
            .as_f64()
            .expect("center sum"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.k.expect("K"),
        oracle["k"].as_f64().expect("oracle K"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.l.expect("L"),
        oracle["l"].as_f64().expect("oracle L"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.theoretical_k,
        std::f64::consts::PI * 1.1_f64.powi(2),
        epsilon = 1e-12
    );
    assert_eq!(
        result.inference.null_model,
        "fixed_gridded_inhomogeneous_binomial"
    );
    assert_eq!(result.inference.simulations_completed, 19);
}

#[test]
fn near_zero_and_each_actual_work_ceiling_fail_explicitly() {
    let baseline = analyze_inhomogeneous_spatial_pattern(&pattern(), &window(), &config(17))
        .expect("baseline");
    let make_limits = |intensity, pairs, draws, bytes| {
        InhomogeneousSpatialLimits::new(16, 16, 1_000, intensity, pairs, draws, bytes)
            .expect("limits")
    };
    assert!(matches!(
        analyze_inhomogeneous_spatial_pattern(
            &pattern(),
            &window(),
            &config_with(
                17,
                1e-12,
                make_limits(
                    1_000_000,
                    1_000_000,
                    1_000_000,
                    baseline.estimated_storage_bytes - 1
                )
            )
        ),
        Err(marklab::InhomogeneousSpatialError::RetainedByteLimitExceeded {
            required,
            maximum
        }) if required == maximum + 1
    ));
    assert!(matches!(
        analyze_inhomogeneous_spatial_pattern(
            &pattern(),
            &window(),
            &config_with(
                17,
                1e-12,
                make_limits(
                    baseline.intensity_evaluations - 1,
                    1_000_000,
                    1_000_000,
                    1 << 20
                )
            )
        ),
        Err(marklab::InhomogeneousSpatialError::IntensityEvaluationLimitExceeded { .. })
    ));
    assert!(matches!(
        analyze_inhomogeneous_spatial_pattern(
            &pattern(),
            &window(),
            &config_with(
                17,
                1e-12,
                make_limits(
                    1_000_000,
                    baseline.total_pair_visits - 1,
                    1_000_000,
                    1 << 20
                )
            )
        ),
        Err(marklab::InhomogeneousSpatialError::PairVisitLimitExceeded { .. })
    ));
    assert!(matches!(
        analyze_inhomogeneous_spatial_pattern(
            &pattern(),
            &window(),
            &config_with(
                17,
                1e-12,
                make_limits(
                    1_000_000,
                    1_000_000,
                    baseline.inference.null_draws - 1,
                    1 << 20
                )
            )
        ),
        Err(marklab::InhomogeneousSpatialError::NullDrawLimitExceeded { .. })
    ));
    assert!(matches!(
        analyze_inhomogeneous_spatial_pattern(
            &pattern(),
            &window(),
            &config_with(
                17,
                1.0,
                make_limits(1_000_000, 1_000_000, 1_000_000, 1 << 20)
            )
        ),
        Err(marklab::InhomogeneousSpatialError::NearZeroIntensity { row: 0, .. })
    ));
}

#[test]
fn polygon_hole_excludes_quadrature_probes_and_invalid_inputs_fail() {
    let donut = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[3,3],[7,3],[7,7],[3,7],[3,3]]
        ]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("donut");
    let safe = Pattern::from_arrays(
        vec![1.0, 2.0, 8.0, 9.0],
        vec![1.0, 1.0, 8.0, 8.0],
        vec![0; 4],
        pattern().meta,
    )
    .expect("safe pattern");
    let result =
        analyze_inhomogeneous_spatial_pattern(&safe, &donut, &config(23)).expect("donut result");
    assert_eq!(result.window.hole_count, 1);
    assert_eq!(result.intensity.retained_probe_count, 84);

    let singleton =
        Pattern::from_arrays(vec![1.0], vec![1.0], vec![0], pattern().meta).expect("singleton");
    assert!(matches!(
        analyze_inhomogeneous_spatial_pattern(&singleton, &window(), &config(23)),
        Err(marklab::InhomogeneousSpatialError::InsufficientPoints)
    ));
    let limits =
        InhomogeneousSpatialLimits::new(16, 16, 1_000, 1_000_000, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits");
    assert!(InhomogeneousSpatialConfig::new(
        vec![f64::MAX],
        2.0,
        [10, 10],
        19,
        23,
        0.05,
        1e-12,
        limits
    )
    .is_err());
}

#[test]
fn kernel_cutoff_and_separable_edge_mass_match_brute_force_intensities() {
    // Seven three-event clusters lie at least 100 um (20 bandwidths) apart,
    // beyond the kernel cutoff, around a hole that removes lattice probes.
    // The isolated event at (250, 150) has no other event within the cutoff,
    // so its leave-one-out sum must still include every distant event.
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[300,0],[300,300],[0,300],[0,0]],
            [[120,120],[180,120],[180,180],[120,180],[120,120]]
        ]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("holed window");
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for (center_x, center_y) in [
        (50.0, 50.0),
        (150.0, 50.0),
        (250.0, 50.0),
        (50.0, 150.0),
        (50.0, 250.0),
        (150.0, 250.0),
        (250.0, 250.0),
    ] {
        for (dx, dy) in [(0.0, 0.0), (3.0, 0.0), (0.0, 4.0)] {
            x.push(center_x + dx);
            y.push(center_y + dy);
        }
    }
    x.push(250.0);
    y.push(150.0);
    let count = x.len();
    let mut pattern =
        Pattern::from_arrays(x.clone(), y.clone(), vec![0; count], pattern().meta).expect("events");
    pattern.cell_ids = Some(
        (0..count)
            .map(|row| format!("cell-{row:02}"))
            .collect::<Vec<_>>()
            .into(),
    );

    let bandwidth = 5.0;
    let simulations = 19;
    let kernel = |left: (f64, f64), right: (f64, f64)| {
        let squared = (left.0 - right.0).powi(2) + (left.1 - right.1).powi(2);
        (-squared / (2.0 * bandwidth * bandwidth)).exp()
            / (2.0 * std::f64::consts::PI * bandwidth * bandwidth)
    };
    let probes = (0..30)
        .flat_map(|row| {
            (0..30).map(move |column| ((column as f64 + 0.5) * 10.0, (row as f64 + 0.5) * 10.0))
        })
        .filter(|probe| window.contains(probe.0, probe.1))
        .collect::<Vec<_>>();
    assert_eq!(probes.len(), 900 - 36);
    let edge_mass = |at: (f64, f64)| {
        probes
            .iter()
            .map(|probe| kernel(at, *probe) * 100.0)
            .sum::<f64>()
    };
    let event = |row: usize| (x[row], y[row]);
    let sum_over = |at: (f64, f64), keep: &dyn Fn(usize) -> bool| {
        (0..count)
            .filter(|row| keep(*row))
            .map(|row| kernel(at, event(row)))
            .sum::<f64>()
    };
    let close = |actual: f64, expected: f64, what: &str| {
        assert!(
            expected > 0.0 && ((actual - expected) / expected).abs() < 1e-12,
            "{what}: {actual:e} != {expected:e}"
        );
    };
    let limits =
        InhomogeneousSpatialLimits::new(64, 4, 1_000, 10_000_000, 1_000_000, 1_000_000, 1 << 24)
            .expect("limits");

    let plain = InhomogeneousSpatialConfig::new(
        vec![1.0],
        bandwidth,
        [30, 30],
        simulations,
        20261004,
        0.05,
        1e-200,
        limits,
    )
    .expect("plain config");
    let result = analyze_inhomogeneous_spatial_pattern(&pattern, &window, &plain).expect("plain");
    for (row, point) in result.intensity.point_values.iter().enumerate() {
        let mass = edge_mass(event(row));
        close(point.boundary_mass, mass, "edge mass");
        let expected =
            count as f64 / (count - 1) as f64 * sum_over(event(row), &|other| other != row) / mass;
        close(point.intensity_per_um2, expected, "leave-one-out intensity");
    }
    for (grid_point, probe) in result.intensity.fixed_grid.iter().zip(&probes) {
        assert_eq!((grid_point.x_um, grid_point.y_um), *probe);
        let expected = sum_over(*probe, &|_| true) / edge_mass(*probe);
        close(
            grid_point.intensity_per_um2,
            expected,
            "fixed-grid intensity",
        );
    }
    // Brute force evaluated every event and probe for every event, probe, and
    // null location; the cutoff and separable edge mass must avoid most of it.
    let brute_force = count * (count - 1)
        + count * probes.len()
        + probes.len() * (count + probes.len())
        + simulations * count * (count + probes.len());
    assert!(
        result.intensity_evaluations * 5 < brute_force,
        "{} of {brute_force} brute-force evaluations",
        result.intensity_evaluations
    );

    // Three cross-fit folds hold rows 0, 3, ... (8 events), 1, 4, ... and
    // 2, 5, ... (7 each), because Cell IDs sort in row order.
    let folds = 3;
    let fold_size = |fold: usize| (0..count).filter(|row| row % folds == fold).count();
    let cross = InhomogeneousSpatialConfig::new_cross_fitted(
        vec![1.0],
        bandwidth,
        [30, 30],
        folds,
        simulations,
        20261004,
        0.05,
        1e-200,
        limits,
    )
    .expect("cross-fit config");
    let result =
        analyze_inhomogeneous_spatial_pattern(&pattern, &window, &cross).expect("cross-fit");
    for (row, point) in result.intensity.point_values.iter().enumerate() {
        let fold = row % folds;
        let training = count - fold_size(fold);
        let expected = count as f64 / training as f64
            * sum_over(event(row), &|other| other % folds != fold)
            / edge_mass(event(row));
        close(
            point.intensity_per_um2,
            expected,
            "cross-fitted event intensity",
        );
    }
    for (grid_point, probe) in result.intensity.fixed_grid.iter().zip(&probes) {
        let numerator = (0..folds)
            .map(|fold| {
                fold_size(fold) as f64 / (count - fold_size(fold)) as f64
                    * sum_over(*probe, &|other| other % folds != fold)
            })
            .sum::<f64>();
        close(
            grid_point.intensity_per_um2,
            numerator / edge_mass(*probe),
            "cross-fitted fixed-grid intensity",
        );
    }
}
