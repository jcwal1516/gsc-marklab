use marklab::{
    analyze_inhomogeneous_spatial_pattern, InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    ObservationWindow2D, ObservationWindowLimits, Pattern, PatternMeta,
};

#[test]
fn stable_cell_id_folds_match_an_independent_gaussian_intensity_oracle() {
    let mut pattern = Pattern::from_arrays(
        vec![2.0, 3.0, 7.0, 9.0],
        vec![2.0, 2.0, 7.0, 7.0],
        vec![0; 4],
        PatternMeta {
            case_id: "cross-fit".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    pattern.cell_ids = Some(
        ["slide:a", "slide:b", "slide:c", "slide:d"]
            .map(str::to_owned)
            .into(),
    );
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window");
    let config = InhomogeneousSpatialConfig::new_cross_fitted(
        vec![1.1],
        2.0,
        [10, 10],
        2,
        19,
        20260831,
        0.05,
        1e-12,
        InhomogeneousSpatialLimits::new(16, 16, 1_000, 2_000_000, 1_000_000, 1_000_000, 1 << 20)
            .expect("limits"),
    )
    .expect("cross-fit config");
    let result = analyze_inhomogeneous_spatial_pattern(&pattern, &window, &config)
        .expect("cross-fitted analysis");
    assert_eq!(result.intensity.cross_fit, "balanced_cell_id_rank_2_fold");
    assert!(result
        .intensity
        .point_values
        .iter()
        .all(|point| point.training_point_count == 2));

    let bandwidth = 2.0;
    let kernel = |left: (f64, f64), right: (f64, f64)| {
        let squared = (left.0 - right.0).powi(2) + (left.1 - right.1).powi(2);
        (-squared / (2.0 * bandwidth * bandwidth)).exp()
            / (2.0 * std::f64::consts::PI * bandwidth * bandwidth)
    };
    let location = (2.0, 2.0);
    let mut boundary_mass = 0.0;
    for y in 0..10 {
        for x in 0..10 {
            boundary_mass += kernel(location, (x as f64 + 0.5, y as f64 + 0.5));
        }
    }
    let expected =
        2.0 * (kernel(location, (3.0, 2.0)) + kernel(location, (9.0, 7.0))) / boundary_mass;
    assert!(
        (result.intensity.point_values[0].intensity_per_um2 - expected).abs() < 1e-15,
        "{} versus {expected}",
        result.intensity.point_values[0].intensity_per_um2
    );
}
