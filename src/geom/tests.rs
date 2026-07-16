use crate::geom::{
    components::ComponentSummary, mask::TumorMask, spatial_index::mean_nearest_neighbor_distance,
};
use approx::assert_abs_diff_eq;

#[test]
fn geojson_multipolygon_mask_respects_holes_and_area() {
    let geojson = r#"
{
  "type": "MultiPolygon",
  "coordinates": [
    [
      [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]],
      [[3.0, 3.0], [7.0, 3.0], [7.0, 7.0], [3.0, 7.0], [3.0, 3.0]]
    ],
    [
      [[20.0, 0.0], [24.0, 0.0], [24.0, 4.0], [20.0, 4.0], [20.0, 0.0]]
    ]
  ]
}
"#;

    let mask = TumorMask::from_geojson_str(geojson).expect("mask");

    assert!(mask.contains(1.0, 1.0));
    assert!(!mask.contains(5.0, 5.0));
    assert!(mask.contains(21.0, 1.0));
    assert!(!mask.contains(30.0, 30.0));
    assert_abs_diff_eq!(mask.area_um2(), 100.0 - 16.0 + 16.0, epsilon = 1e-9);
    assert_abs_diff_eq!(
        mask.effective_diameter_um(),
        (4.0 * 100.0 / std::f64::consts::PI).sqrt(),
        epsilon = 1e-9
    );
}

#[test]
fn geojson_mask_rejects_non_multipolygon_input() {
    let point = r#"{"type": "Point", "coordinates": [0.0, 0.0]}"#;

    let err = TumorMask::from_geojson_str(point).expect_err("point should be rejected");

    assert!(err.to_string().contains("MultiPolygon"));
}

#[test]
fn component_summary_reports_largest_fraction() {
    let summary = ComponentSummary::from_component_ids(&[7, 7, 7, 2, 2, 9]);

    assert_eq!(summary.component_count, 3);
    assert_abs_diff_eq!(summary.largest_fraction, 0.5, epsilon = 1e-12);
}

#[test]
fn mean_nearest_neighbor_distance_uses_each_points_closest_neighbor() {
    let distance =
        mean_nearest_neighbor_distance(&[0.0, 3.0, 10.0], &[0.0, 4.0, 0.0]).expect("distance");

    assert_abs_diff_eq!(
        distance,
        (5.0 + 5.0 + (65.0_f64).sqrt()) / 3.0,
        epsilon = 1e-12
    );
}
