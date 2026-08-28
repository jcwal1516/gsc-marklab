use marklab::{
    analyze_inhomogeneous_pair_correlation, analyze_inhomogeneous_spatial_pattern,
    InhomogeneousPairCorrelationConfig, InhomogeneousSpatialConfig, InhomogeneousSpatialLimits,
    ObservationWindow2D, ObservationWindowLimits, Pattern, PatternMeta,
};

fn window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn gradient_pattern(replicate: usize) -> Pattern {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ replicate as u64;
    let mut x = Vec::new();
    let mut y = Vec::new();
    for _ in 0..40 {
        state = splitmix64(state);
        x.push(10.0 * unit_interval(state).sqrt());
        state = splitmix64(state);
        y.push(10.0 * unit_interval(state));
    }
    Pattern::from_arrays(
        x,
        y,
        vec![0; 40],
        PatternMeta {
            case_id: format!("inhomogeneous-null-{replicate}"),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some(format!("inhomogeneous-null-slide-{replicate}")),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn config(seed: u64) -> InhomogeneousSpatialConfig {
    config_at(seed, vec![1.0, 2.0])
}

fn config_at(seed: u64, radii_um: Vec<f64>) -> InhomogeneousSpatialConfig {
    InhomogeneousSpatialConfig::new(
        radii_um,
        2.0,
        [12, 12],
        19,
        seed,
        0.05,
        1e-10,
        InhomogeneousSpatialLimits::new(64, 8, 256, 10_000_000, 10_000_000, 1_000_000, 4 << 20)
            .expect("limits"),
    )
    .expect("config")
}

#[test]
fn fixed_gradient_inhomogeneous_poisson_control_is_not_grossly_anti_conservative() {
    let window = window();
    let mut rejections = 0;
    for replicate in 0..20 {
        let result = analyze_inhomogeneous_spatial_pattern(
            &gradient_pattern(replicate),
            &window,
            &config(20260827 + replicate as u64),
        )
        .expect("calibration result");
        let p = result.inference.p_global.expect("global p-value");
        assert!(p.is_finite() && (0.0..=1.0).contains(&p));
        if p <= 0.05 {
            rejections += 1;
        }
    }
    assert!(
        rejections <= 6,
        "inhomogeneous null rejected {rejections}/20 deterministic controls"
    );
}

#[test]
fn prespecified_tight_cluster_control_has_excess_short_range_k_after_reweighting() {
    let pattern = tight_cluster_pattern();
    let result = analyze_inhomogeneous_spatial_pattern(&pattern, &window(), &config(77))
        .expect("cluster result");
    assert!(
        result.curve[0].k.expect("short-range K") > result.curve[0].theoretical_k,
        "the prespecified tight-cluster control should retain short-range excess"
    );
}

#[test]
fn inhomogeneous_g_gradient_null_and_cluster_direction_are_prespecified() {
    let window = window();
    let mut rejections = 0;
    for replicate in 0..20 {
        let result = analyze_inhomogeneous_pair_correlation(
            &gradient_pattern(replicate),
            &window,
            &InhomogeneousPairCorrelationConfig::new(config(20260927 + replicate as u64), 0.5)
                .expect("g config"),
        )
        .expect("g calibration result");
        let p = result.inference.p_global.expect("global p-value");
        assert!(p.is_finite() && (0.0..=1.0).contains(&p));
        if p <= 0.05 {
            rejections += 1;
        }
    }
    assert!(
        rejections <= 6,
        "inhomogeneous g null rejected {rejections}/20 deterministic controls"
    );
    let cluster = analyze_inhomogeneous_pair_correlation(
        &tight_cluster_pattern(),
        &window,
        &InhomogeneousPairCorrelationConfig::new(config_at(79, vec![0.3]), 0.15)
            .expect("cluster g config"),
    )
    .expect("cluster g result");
    assert!(cluster.curve[0].g.expect("short-range g") > 1.0);
}

fn tight_cluster_pattern() -> Pattern {
    let mut x = Vec::new();
    let mut y = Vec::new();
    for (center_x, center_y) in [(3.0, 3.0), (7.0, 7.0)] {
        for row in 0..4 {
            for column in 0..5 {
                x.push(center_x + (column as f64 - 2.0) * 0.15);
                y.push(center_y + (row as f64 - 1.5) * 0.15);
            }
        }
    }
    Pattern::from_arrays(
        x,
        y,
        vec![0; 40],
        PatternMeta {
            case_id: "inhomogeneous-cluster-control".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("inhomogeneous-cluster-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("cluster pattern")
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn unit_interval(value: u64) -> f64 {
    (value >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}
