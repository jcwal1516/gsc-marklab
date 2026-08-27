use approx::assert_abs_diff_eq;
use marklab::{
    analyze_nearest_space_pattern, DistributionPointStatus, JPointStatus, NearestSpaceConfig,
    NearestSpaceLimits, NearestSpaceStatus, ObservationWindow2D, ObservationWindowLimits, Pattern,
    PatternMeta,
};

fn window() -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("window")
}

fn pattern() -> Pattern {
    Pattern::from_arrays(
        vec![2.0, 8.0, 2.0, 8.0],
        vec![2.0, 2.0, 8.0, 8.0],
        vec![0; 4],
        PatternMeta {
            case_id: "fgj-oracle".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: Some("fgj-slide".into()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

fn limits() -> NearestSpaceLimits {
    NearestSpaceLimits::new(32, 16, 64, 100_000, 100_000, 1 << 20).expect("limits")
}

#[test]
fn regular_grid_f_g_j_matches_a_hand_oracle_and_never_persists_infinity() {
    let config =
        NearestSpaceConfig::new(vec![0.5, 1.0], [2, 2], 19, 20260827, 0.05, 1e-12, limits())
            .expect("configuration");

    let first = analyze_nearest_space_pattern(&pattern(), &window(), &config).expect("analysis");
    let second = analyze_nearest_space_pattern(&pattern(), &window(), &config).expect("replay");

    assert_eq!(first, second);
    assert_eq!(first.status, NearestSpaceStatus::Available);
    assert_eq!(first.probes.requested_grid, [2, 2]);
    assert_eq!(first.probes.retained_probe_count, 4);
    assert_abs_diff_eq!(first.probes.spacing_um[0], 5.0, epsilon = 1e-12);
    assert_abs_diff_eq!(first.probes.spacing_um[1], 5.0, epsilon = 1e-12);
    assert_abs_diff_eq!(
        first.probes.maximum_location_error_um,
        5.0_f64.hypot(5.0) / 2.0
    );

    let near = &first.curve[0];
    assert_eq!(near.f_status, DistributionPointStatus::Available);
    assert_eq!(near.g_status, DistributionPointStatus::Available);
    assert_eq!(near.j_status, JPointStatus::Available);
    assert_eq!(near.eligible_probes, 4);
    assert_eq!(near.probes_with_event_within_radius, 0);
    assert_eq!(near.eligible_event_centers, 4);
    assert_eq!(near.events_with_neighbor_within_radius, 0);
    assert_eq!(near.f, Some(0.0));
    assert_eq!(near.g, Some(0.0));
    assert_eq!(near.j, Some(1.0));

    let far = &first.curve[1];
    assert_eq!(far.f, Some(1.0));
    assert_eq!(far.g, Some(0.0));
    assert_eq!(far.j_status, JPointStatus::DenominatorTooSmall);
    assert_eq!(far.j, None);
    assert!(first.curve.iter().all(|point| {
        point
            .f
            .into_iter()
            .chain(point.g)
            .chain(point.j)
            .all(f64::is_finite)
    }));
    assert_eq!(first.null_design.conditioned_point_count, 4);
    assert_eq!(first.null_design.simulations, 19);
    assert!(first.inference.f.is_some());
    assert!(first.inference.g.is_some());
}

#[test]
fn probes_respect_holes_and_query_budget_fails_before_a_result() {
    let donut = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[
            [[0,0],[10,0],[10,10],[0,10],[0,0]],
            [[4,4],[6,4],[6,6],[4,6],[4,4]]
        ]]}"#,
        ObservationWindowLimits::new(4_096, 4, 8, 64, 256).expect("window limits"),
    )
    .expect("donut");
    let config = NearestSpaceConfig::new(vec![0.5], [5, 5], 19, 7, 0.05, 1e-12, limits())
        .expect("configuration");
    let result = analyze_nearest_space_pattern(&pattern(), &donut, &config).expect("analysis");
    assert_eq!(result.probes.retained_probe_count, 24);

    let one_short = NearestSpaceConfig::new(
        vec![0.5],
        [2, 2],
        19,
        7,
        0.05,
        1e-12,
        NearestSpaceLimits::new(32, 16, 64, 1, 100_000, 1 << 20).expect("limits"),
    )
    .expect("configuration");
    assert!(analyze_nearest_space_pattern(&pattern(), &window(), &one_short).is_err());
}

#[test]
fn observed_f_and_g_match_an_independent_brute_force_oracle() {
    let input = Pattern::from_arrays(
        vec![1.0, 2.0, 5.0, 7.0, 9.0],
        vec![1.0, 6.0, 5.0, 8.0, 2.0],
        vec![0; 5],
        PatternMeta {
            case_id: "differential".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    let radii = vec![0.5, 1.0, 2.5];
    let config = NearestSpaceConfig::new(radii.clone(), [4, 4], 19, 31, 0.05, 1e-12, limits())
        .expect("config");
    let result = analyze_nearest_space_pattern(&input, &window(), &config).expect("analysis");

    let probes = (0..4)
        .flat_map(|y| (0..4).map(move |x| (1.25 + x as f64 * 2.5, 1.25 + y as f64 * 2.5)))
        .collect::<Vec<_>>();
    for (point, radius) in result.curve.iter().zip(radii) {
        let event_distances = (0..input.len())
            .map(|row| {
                (0..input.len())
                    .filter(|other| *other != row)
                    .map(|other| {
                        (input.x_um[row] - input.x_um[other])
                            .hypot(input.y_um[row] - input.y_um[other])
                    })
                    .fold(f64::INFINITY, f64::min)
            })
            .collect::<Vec<_>>();
        let eligible_events = (0..input.len())
            .filter(|row| {
                input.x_um[*row]
                    .min(10.0 - input.x_um[*row])
                    .min(input.y_um[*row])
                    .min(10.0 - input.y_um[*row])
                    >= radius
            })
            .collect::<Vec<_>>();
        let expected_g = eligible_events
            .iter()
            .filter(|row| event_distances[**row] <= radius)
            .count() as f64
            / eligible_events.len() as f64;
        let eligible_probes = probes
            .iter()
            .filter(|(x, y)| x.min(10.0 - x).min(*y).min(10.0 - y) >= radius)
            .collect::<Vec<_>>();
        let expected_f = eligible_probes
            .iter()
            .filter(|(x, y)| {
                (0..input.len())
                    .map(|row| (x - input.x_um[row]).hypot(y - input.y_um[row]))
                    .fold(f64::INFINITY, f64::min)
                    <= radius
            })
            .count() as f64
            / eligible_probes.len() as f64;
        assert_abs_diff_eq!(point.f.expect("F"), expected_f, epsilon = 1e-12);
        assert_abs_diff_eq!(point.g.expect("G"), expected_g, epsilon = 1e-12);
    }
}

#[test]
fn empty_and_singleton_patterns_round_trip_as_typed_insufficient_event_results() {
    let config =
        NearestSpaceConfig::new(vec![0.5], [2, 2], 19, 41, 0.05, 1e-12, limits()).expect("config");
    for (x, y, marks) in [(vec![], vec![], vec![]), (vec![5.0], vec![5.0], vec![0])] {
        let input = Pattern::from_arrays(
            x,
            y,
            marks,
            PatternMeta {
                case_id: "sparse".into(),
                timepoint: "baseline".into(),
                protein: "unmarked".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern");
        let result = analyze_nearest_space_pattern(&input, &window(), &config).expect("analysis");
        assert_eq!(result.status, NearestSpaceStatus::InsufficientEvents);
        assert!(result
            .curve
            .iter()
            .all(|point| point.g.is_none() && point.j.is_none()));
        let encoded = marklab::NearestSpaceResultDocument::new(result)
            .expect("document")
            .to_json_pretty()
            .expect("encode");
        marklab::NearestSpaceResultDocument::from_json(&encoded).expect("decode");
    }
}

#[test]
fn pinned_scipy_ckdtree_oracle_agrees_for_every_f_g_j_field() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/nearest_space/scipy_rectangle_oracle.json"
    ))
    .expect("oracle JSON");
    assert_eq!(oracle["oracle"], "scipy.spatial.cKDTree");
    assert_eq!(oracle["scipy_version"], "1.18.1");
    let coordinates = oracle["points"].as_array().expect("points");
    let input = Pattern::from_arrays(
        coordinates
            .iter()
            .map(|row| row[0].as_f64().expect("x"))
            .collect(),
        coordinates
            .iter()
            .map(|row| row[1].as_f64().expect("y"))
            .collect(),
        vec![0; coordinates.len()],
        PatternMeta {
            case_id: "scipy-oracle".into(),
            timepoint: "baseline".into(),
            protein: "unmarked".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    let rows = oracle["radii"].as_array().expect("radii");
    let config = NearestSpaceConfig::new(
        rows.iter()
            .map(|row| row["radius_um"].as_f64().expect("radius"))
            .collect(),
        [4, 4],
        19,
        43,
        0.05,
        1e-12,
        limits(),
    )
    .expect("config");
    let result = analyze_nearest_space_pattern(&input, &window(), &config).expect("result");
    for (actual, expected) in result.curve.iter().zip(rows) {
        assert_eq!(
            actual.eligible_event_centers,
            expected["eligible_events"].as_u64().expect("events") as usize
        );
        assert_eq!(
            actual.events_with_neighbor_within_radius,
            expected["events_with_neighbor"]
                .as_u64()
                .expect("neighbor events") as usize
        );
        assert_eq!(
            actual.eligible_probes,
            expected["eligible_probes"].as_u64().expect("probes") as usize
        );
        assert_eq!(
            actual.probes_with_event_within_radius,
            expected["probes_with_event"]
                .as_u64()
                .expect("probe events") as usize
        );
        assert_abs_diff_eq!(actual.f.expect("F"), expected["f"].as_f64().expect("F"));
        assert_abs_diff_eq!(actual.g.expect("G"), expected["g"].as_f64().expect("G"));
        match expected["j"].as_f64() {
            Some(expected_j) => assert_abs_diff_eq!(actual.j.expect("J"), expected_j),
            None => assert_eq!(actual.j, None),
        }
    }
}
