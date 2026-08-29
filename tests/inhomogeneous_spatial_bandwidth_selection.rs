use approx::assert_abs_diff_eq;
use marklab::{
    analyze_selected_inhomogeneous_spatial_pattern, GaussianBandwidthSelectionConfig,
    GaussianBandwidthSelectionLimits, InhomogeneousSpatialLimits, ObservationWindow2D,
    ObservationWindowLimits, Pattern, PatternMeta,
};

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![2.0, 3.0, 7.0, 9.0],
        vec![2.0, 2.0, 7.0, 7.0],
        vec![0; 4],
        PatternMeta {
            case_id: "bandwidth-selection-case".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("bandwidth-selection-slide".into()),
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

fn config(selection_evaluations: usize) -> GaussianBandwidthSelectionConfig {
    GaussianBandwidthSelectionConfig::new(
        vec![1.1],
        vec![1.0, 2.0, 3.0],
        [10, 10],
        19,
        20260829,
        0.05,
        1e-12,
        InhomogeneousSpatialLimits::new(16, 8, 1_000, 1_000_000, 1_000_000, 1_000_000, 1 << 20)
            .expect("analysis limits"),
        GaussianBandwidthSelectionLimits::new(8, selection_evaluations).expect("selection limits"),
    )
    .expect("config")
}

fn gaussian(left: (f64, f64), right: (f64, f64), bandwidth: f64) -> f64 {
    let squared = (left.0 - right.0).powi(2) + (left.1 - right.1).powi(2);
    (-squared / (2.0 * bandwidth * bandwidth)).exp()
        / (2.0 * std::f64::consts::PI * bandwidth * bandwidth)
}

fn direct_mean_log_leave_one_out_intensity(pattern: &Pattern, bandwidth: f64) -> f64 {
    let mut logs = 0.0;
    for row in 0..pattern.len() {
        let location = (pattern.x_um[row], pattern.y_um[row]);
        let mut boundary_mass = 0.0;
        for y in 0..10 {
            for x in 0..10 {
                boundary_mass += gaussian(location, (x as f64 + 0.5, y as f64 + 0.5), bandwidth);
            }
        }
        let raw = (0..pattern.len())
            .filter(|other| *other != row)
            .map(|other| {
                gaussian(
                    location,
                    (pattern.x_um[other], pattern.y_um[other]),
                    bandwidth,
                )
            })
            .sum::<f64>();
        let intensity = pattern.len() as f64 / (pattern.len() - 1) as f64 * raw / boundary_mass;
        logs += intensity.ln();
    }
    logs / pattern.len() as f64
}

#[test]
fn prespecified_leave_one_out_likelihood_selects_bandwidth_before_kl() {
    let pattern = pattern();
    let result =
        analyze_selected_inhomogeneous_spatial_pattern(&pattern, &window(), &config(1_000_000))
            .expect("selected result");
    let replay =
        analyze_selected_inhomogeneous_spatial_pattern(&pattern, &window(), &config(1_000_000))
            .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.selection.method, "leave_one_out_log_likelihood");
    assert_eq!(result.selection.tie_break, "smallest_bandwidth_um");
    assert_eq!(result.selection.selected_bandwidth_um, 1.0);
    assert_eq!(result.analysis.intensity.bandwidth_um, 1.0);
    assert_eq!(result.selection.candidates.len(), 3);
    for candidate in &result.selection.candidates {
        assert_abs_diff_eq!(
            candidate.mean_log_leave_one_out_intensity,
            direct_mean_log_leave_one_out_intensity(&pattern, candidate.bandwidth_um),
            epsilon = 1e-12
        );
    }
    assert!(
        !result.selection.selection_uses_spatial_curve,
        "selection must not inspect K/L/g"
    );
}

#[test]
fn selection_intensity_work_ceiling_is_hard() {
    let baseline =
        analyze_selected_inhomogeneous_spatial_pattern(&pattern(), &window(), &config(1_000_000))
            .expect("baseline");
    assert!(matches!(
        analyze_selected_inhomogeneous_spatial_pattern(
            &pattern(),
            &window(),
            &config(baseline.selection.intensity_evaluations - 1),
        ),
        Err(marklab::InhomogeneousSpatialError::IntensityEvaluationLimitExceeded { .. })
    ));
}
