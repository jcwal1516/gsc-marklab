use crate::geom::{
    components::ComponentSummary,
    mask::TumorMask,
    spatial_index::{mean_nearest_neighbor_distance, Neighbor, SpatialIndex2D},
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

#[test]
fn nearest_neighbor_matches_bruteforce() {
    for points in spatial_index_cases() {
        let (x, y) = split_points(&points);
        let index = SpatialIndex2D::new(&x, &y).expect("index");

        for source in 0..points.len() {
            let expected = brute_force_neighbors(&points, points[source], Some(source), None);
            assert_eq!(
                index.nearest_neighbor(source).expect("nearest"),
                expected.first().copied().map(neighbor_from_pair),
                "nearest mismatch for source {source} in {points:?}"
            );
        }
    }
}

#[test]
fn knn_matches_bruteforce() {
    for points in spatial_index_cases() {
        let (x, y) = split_points(&points);
        let index = SpatialIndex2D::new(&x, &y).expect("index");

        for source in 0..points.len() {
            let expected = brute_force_neighbors(&points, points[source], Some(source), None);
            for k in [0, 1, 2, points.len(), points.len() + 3] {
                let expected = expected.iter().copied().take(k).collect::<Vec<_>>();
                assert_neighbors_equal(&index.k_nearest(source, k).expect("k nearest"), &expected);
            }
        }
    }
}

#[test]
fn radius_query_matches_bruteforce() {
    let points = randomish_points(97);
    let (x, y) = split_points(&points);
    let index = SpatialIndex2D::new(&x, &y).expect("index");

    for source in [0, 7, 48, 96] {
        for radius in [0.0, 1.0, 7.5, 50.0, f64::MAX] {
            let expected =
                brute_force_neighbors(&points, points[source], Some(source), Some(radius));
            assert_neighbors_equal(
                &index.within_radius(source, radius).expect("radius query"),
                &expected,
            );
        }
    }

    let query = [4.25, -3.75];
    let expected = brute_force_neighbors(&points, query, None, Some(12.0));
    assert_neighbors_equal(
        &index
            .points_within_radius(query[0], query[1], 12.0)
            .expect("point radius query"),
        &expected,
    );
}

#[test]
fn duplicate_coordinate_ties() {
    let points = vec![[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [1.0, 0.0]];
    let (x, y) = split_points(&points);
    let index = SpatialIndex2D::new(&x, &y).expect("index");

    assert_eq!(
        index.k_nearest(1, 3).expect("neighbors"),
        vec![
            Neighbor {
                index: 0,
                distance_um: 0.0,
            },
            Neighbor {
                index: 2,
                distance_um: 0.0,
            },
            Neighbor {
                index: 3,
                distance_um: 1.0,
            },
        ]
    );
    assert_eq!(
        index.within_radius(1, 0.0).expect("zero radius"),
        vec![
            Neighbor {
                index: 0,
                distance_um: 0.0,
            },
            Neighbor {
                index: 2,
                distance_um: 0.0,
            },
        ]
    );
    assert_eq!(
        index
            .points_within_radius(0.0, 0.0, 0.0)
            .expect("point zero radius")
            .iter()
            .map(|neighbor| neighbor.index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn deterministic_query_order() {
    let points = randomish_points(128);
    let (x, y) = split_points(&points);
    let index = SpatialIndex2D::new(&x, &y).expect("index");
    let expected_knn = index.k_nearest(17, 12).expect("kNN");
    let expected_radius = index.within_radius(17, 20.0).expect("radius");

    for _ in 0..10 {
        assert_eq!(index.k_nearest(17, 12).expect("kNN"), expected_knn);
        assert_eq!(
            index.within_radius(17, 20.0).expect("radius"),
            expected_radius
        );
    }
    assert!(expected_radius.windows(2).all(|pair| {
        pair[0].distance_um < pair[1].distance_um
            || pair[0].distance_um == pair[1].distance_um && pair[0].index < pair[1].index
    }));
}

#[test]
fn spatial_index_rejects_invalid_coordinates_and_queries() {
    assert!(SpatialIndex2D::new(&[0.0], &[]).is_err());
    assert!(SpatialIndex2D::new(&[f64::NAN], &[0.0]).is_err());
    assert!(SpatialIndex2D::new(&[0.0], &[f64::INFINITY]).is_err());

    let index = SpatialIndex2D::new(&[0.0, 1.0], &[0.0, 0.0]).expect("index");
    assert!(index.nearest_neighbor(2).is_err());
    assert!(index.k_nearest(2, 1).is_err());
    assert!(index.within_radius(2, 1.0).is_err());
    assert!(index.within_radius(0, -1.0).is_err());
    assert!(index.within_radius(0, f64::NAN).is_err());
    assert!(index.points_within_radius(f64::NAN, 0.0, 1.0).is_err());
    assert!(index.points_within_radius(0.0, 0.0, -1.0).is_err());
}

fn spatial_index_cases() -> Vec<Vec<[f64; 2]>> {
    let grid = (0..5)
        .flat_map(|row| (0..7).map(move |column| [column as f64, row as f64]))
        .collect();
    vec![
        vec![[0.0, 0.0], [3.0, 4.0]],
        grid,
        vec![[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        vec![[-5.0, 2.0], [-1.0, 2.0], [2.0, 2.0], [9.0, 2.0]],
        vec![
            [1.0e12, -1.0e12],
            [1.0e12 + 3.0, -1.0e12 + 4.0],
            [1.0e12 + 20.0, -1.0e12 - 5.0],
        ],
        vec![[-1.0e200, 0.0], [0.0, 0.0], [1.0e200, 0.0]],
        randomish_points(64),
    ]
}

fn randomish_points(count: usize) -> Vec<[f64; 2]> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let x = ((state >> 32) as i32 as f64) / 50_000_000.0;
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let y = ((state >> 32) as i32 as f64) / 50_000_000.0;
            [x, y]
        })
        .collect()
}

fn split_points(points: &[[f64; 2]]) -> (Vec<f64>, Vec<f64>) {
    (
        points.iter().map(|point| point[0]).collect(),
        points.iter().map(|point| point[1]).collect(),
    )
}

fn brute_force_neighbors(
    points: &[[f64; 2]],
    query: [f64; 2],
    excluded_index: Option<usize>,
    radius: Option<f64>,
) -> Vec<(usize, f64)> {
    let mut neighbors = points
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != excluded_index)
        .filter_map(|(index, point)| {
            let distance = (point[0] - query[0]).hypot(point[1] - query[1]);
            radius
                .is_none_or(|radius| distance <= radius)
                .then_some((index, distance))
        })
        .collect::<Vec<_>>();
    neighbors.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    neighbors
}

fn neighbor_from_pair((index, distance_um): (usize, f64)) -> Neighbor {
    Neighbor { index, distance_um }
}

fn assert_neighbors_equal(actual: &[Neighbor], expected: &[(usize, f64)]) {
    assert_eq!(actual.len(), expected.len(), "actual={actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.index, expected.0);
        assert_eq!(actual.distance_um, expected.1);
    }
}
