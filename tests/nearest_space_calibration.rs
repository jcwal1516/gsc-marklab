use marklab::{
    analyze_nearest_space_pattern, NearestSpaceConfig, NearestSpaceLimits, ObservationWindow2D,
    ObservationWindowLimits, Pattern, PatternMeta,
};

fn window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
}

fn pattern(x: Vec<f64>, y: Vec<f64>, case: &str) -> Pattern {
    Pattern::from_arrays(
        x.clone(),
        y,
        vec![0; x.len()],
        PatternMeta {
            case_id: case.into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn config(seed: u64, radii: Vec<f64>, probes: [usize; 2]) -> NearestSpaceConfig {
    NearestSpaceConfig::new(
        radii,
        probes,
        39,
        seed,
        0.05,
        1e-12,
        NearestSpaceLimits::new(64, 16, 256, 1_000_000, 1_000_000, 8 << 20).expect("limits"),
    )
    .expect("config")
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn unit(value: u64) -> f64 {
    (value >> 11) as f64 / (1_u64 << 53) as f64
}

#[test]
fn conditional_csr_component_tests_are_not_grossly_anti_conservative() {
    let mut significant = 0_usize;
    let mut tested = 0_usize;
    let mut state = 0x4353_522d_4647_4a31_u64;
    for replicate in 0..40 {
        let mut x = Vec::new();
        let mut y = Vec::new();
        for _ in 0..16 {
            state = splitmix64(state);
            x.push(10.0 * unit(state));
            state = splitmix64(state);
            y.push(10.0 * unit(state));
        }
        let result = analyze_nearest_space_pattern(
            &pattern(x, y, "csr-calibration"),
            &window(),
            &config(10_000 + replicate, vec![0.5, 1.0, 1.5], [8, 8]),
        )
        .expect("calibration result");
        for inference in [result.inference.f, result.inference.g]
            .into_iter()
            .flatten()
        {
            tested += 1;
            significant += usize::from(inference.p_global <= 0.05);
        }
    }
    assert_eq!(tested, 80);
    assert!(
        significant <= 10,
        "{significant}/80 null component tests were significant"
    );
}

#[test]
fn cluster_and_inhibition_controls_have_the_expected_j_direction() {
    let clustered = pattern(
        vec![1.0, 1.1, 1.2, 1.3, 8.0, 8.1, 8.2, 8.3],
        vec![1.0, 1.2, 1.1, 1.3, 8.0, 8.2, 8.1, 8.3],
        "clustered",
    );
    let clustered_result =
        analyze_nearest_space_pattern(&clustered, &window(), &config(51, vec![0.5], [10, 10]))
            .expect("clustered result");
    assert!(clustered_result.curve[0].j.expect("clustered J") < 1.0);

    let coordinates = [2.0, 4.0, 6.0, 8.0];
    let inhibited = pattern(
        coordinates
            .iter()
            .flat_map(|_| coordinates.iter().copied())
            .collect(),
        coordinates
            .iter()
            .flat_map(|y| std::iter::repeat_n(*y, coordinates.len()))
            .collect(),
        "inhibited",
    );
    let inhibited_result =
        analyze_nearest_space_pattern(&inhibited, &window(), &config(53, vec![1.0], [8, 8]))
            .expect("inhibited result");
    assert!(inhibited_result.curve[0].j.expect("inhibited J") > 1.0);
}
