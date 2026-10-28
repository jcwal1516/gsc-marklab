use std::path::Path;
use std::process::Command;

use marklab::space::{
    load_space_input, space_census, space_cis, space_dist, space_input_from_json,
    space_mi_result_from_json, space_rank, space_trans, SpaceCisParameters, SpaceDistCensus,
    SpaceDistParameters, SpaceDistValues, SpaceError, SpaceMiResult, SpaceNullStatus,
    SpaceRankParameters, SpaceRankView, SpaceTransParameters,
};
use serde_json::{json, Value};

fn square(side: f64) -> Value {
    json!({"type": "MultiPolygon", "coordinates":
        [[[[-1.0, -1.0], [side, -1.0], [side, side], [-1.0, side], [-1.0, -1.0]]]]})
}

fn cell_type(classes: &[&str]) -> Value {
    json!({"id": "cell_type", "label": "cell type", "kind": "categorical", "unit": "categorical",
        "measurement_status": "imported_prediction", "provenance": "synthetic", "levels": classes})
}

fn ki67() -> Value {
    json!({"id": "ki67", "label": "Ki67", "kind": "continuous", "unit": "au",
        "measurement_status": "measured", "provenance": "synthetic"})
}

fn inline_request(specimens: Value, extra: Value) -> Vec<u8> {
    let mut request = json!({
        "format": "marklab.space_request", "version": 1,
        "source": {"kind": "inline", "channels": [cell_type(&["A", "B", "C"]), ki67()],
            "specimens": specimens},
        "objects": ["cell_type"],
    });
    for (key, value) in extra.as_object().unwrap() {
        request[key] = value.clone();
    }
    serde_json::to_vec(&request).unwrap()
}

/// A `side` x `side` unit grid. Class A fills vertical stripes `stripe` cells wide (or a
/// pseudo-random half when `stripe` is zero); Ki67 is 10 on A cells and 1 on B cells.
fn grid_specimen(id: &str, side: usize, stripe: usize) -> Value {
    let mut ids = Vec::new();
    let mut coordinates = Vec::new();
    let mut classes = Vec::new();
    let mut values = Vec::new();
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    for x in 0..side {
        for y in 0..side {
            let class_a = match x.checked_div(stripe) {
                Some(band) => band.is_multiple_of(2),
                None => {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state.is_multiple_of(2)
                }
            };
            ids.push(format!("{id}:{x}:{y}"));
            coordinates.push([x as f64, y as f64]);
            classes.push(if class_a { 0.0 } else { 1.0 });
            values.push(if class_a { 10.0 } else { 1.0 });
        }
    }
    json!({"specimen_id": id, "coordinate_frame_id": format!("{id}-um"),
        "window": square(side as f64), "cell_ids": ids, "coordinates_um": coordinates,
        "observations": {"cell_type": classes, "ki67": values}})
}

#[test]
fn census_reports_class_percentages_and_scalar_means_of_inclusive_discs() {
    // c3 has no class (background); class C has no cells and is therefore not a variable.
    let specimen = json!({"specimen_id": "s1", "coordinate_frame_id": "s1-um",
        "window": square(6.0), "cell_ids": ["c0", "c1", "c2", "c3", "c4"],
        "coordinates_um": [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [0.0, 1.5], [5.0, 5.0]],
        "observations": {"cell_type": [0.0, 1.0, 0.0, null, 1.0],
            "ki67": [1.0, 3.0, 5.0, 7.0, 9.0]}});
    let request = inline_request(
        json!([specimen]),
        json!({
            "linked_scalars": [{"channel": "cell_type", "class": "A", "scalar": "ki67"},
                {"channel": "cell_type", "class": "B", "scalar": "ki67"}],
            "independent_scalars": ["ki67"],
            "census": {"radii_um": [1.0, 2.0], "seed": 7,
                "seed_cell_ids": {"s1": [["c0"], ["c1"]]}}
        }),
    );
    let input = space_input_from_json(&request).unwrap();
    let result = space_census(&input).unwrap();
    let specimen = &result.specimens[0];
    assert_eq!(
        specimen.variables,
        [
            "cell_type.A",
            "cell_type.B",
            "cell_type.A_ki67",
            "cell_type.B_ki67",
            "ki67"
        ]
    );
    // Radius 1 around c0 reaches c1 at exactly 1 um (inclusive) but not c3 at 1.5 um.
    let disc = &specimen.radii[0].discs[0];
    assert_eq!((disc.seed_cell_id.as_str(), disc.member_count), ("c0", 2));
    assert_eq!(disc.values, [50.0, 50.0, 1.0, 3.0, 2.0]);
    // Radius 2 around c1 holds c0, c1, c2 and the unclassified c3, which counts only toward
    // the independent scalar.
    let disc = &specimen.radii[1].discs[0];
    assert_eq!(disc.member_count, 4);
    assert_eq!(
        disc.values,
        [100.0 * (2.0 / 3.0), 100.0 * (1.0 / 3.0), 3.0, 3.0, 4.0]
    );
}

#[test]
fn scalar_magnitude_and_retained_memory_are_bounded() {
    let request = |ki67: f64, limits: Value| {
        let specimen = json!({"specimen_id": "s1", "coordinate_frame_id": "s1-um",
            "window": square(2.0), "cell_ids": ["c0", "c1"],
            "coordinates_um": [[0.0, 0.0], [1.0, 0.0]],
            "observations": {"cell_type": [0.0, 1.0], "ki67": [ki67, ki67]}});
        inline_request(
            json!([specimen]),
            json!({
                "independent_scalars": ["ki67"],
                "census": {"radii_um": [1.0], "seed": 1, "seed_cell_ids": {"s1": [["c0"]]}},
                "limits": limits
            }),
        )
    };
    // The bound keeps disc sums, grid steps and squared distances finite.
    let largest =
        space_census(&space_input_from_json(&request(1e150, json!({}))).unwrap()).unwrap();
    assert_eq!(
        largest.specimens[0].radii[0].discs[0].values,
        [50.0, 50.0, 1e150]
    );
    assert!(matches!(
        space_input_from_json(&request(1e200, json!({}))),
        Err(SpaceError::Invalid(message)) if message.contains("magnitude at most 1e150")
    ));
    let small =
        space_input_from_json(&request(1.0, json!({"maximum_retained_bytes": 64}))).unwrap();
    assert!(matches!(
        space_census(&small),
        Err(SpaceError::Resource(message)) if message.contains("retained-memory")
    ));
    // One disc of 1,024 cells: its output row fits, but drawing its members stops at the
    // budget instead of allocating them and the census plan first.
    let crowded = inline_request(
        json!([grid_specimen("g", 32, 4)]),
        json!({"census": {"radii_um": [100.0], "seed": 1, "seed_cell_ids": {"g": [["g:0:0"]]}},
            "limits": {"maximum_retained_bytes": 4096}}),
    );
    assert!(matches!(
        space_census(&space_input_from_json(&crowded).unwrap()),
        Err(SpaceError::Resource(message)) if message.contains("retained-memory limits allow")
    ));
}

fn cis_input(specimens: Value, sample: usize) -> marklab::space::SpaceInput {
    let request = inline_request(
        specimens,
        json!({
            "independent_scalars": ["ki67"],
            "census": {"radii_um": [3.0], "sample_size": [sample], "seed": 11}
        }),
    );
    space_input_from_json(&request).unwrap()
}

#[test]
fn cis_detects_striped_co_patterning_and_not_random_labels() {
    let input = cis_input(
        json!([
            grid_specimen("stripes", 40, 8),
            grid_specimen("random", 40, 0)
        ]),
        200,
    );
    let parameters = SpaceCisParameters {
        depth: 2,
        bootstraps: 60,
        ..SpaceCisParameters::default()
    };
    let result = space_cis(&input, &parameters).unwrap();
    let random = &result.specimens[0];
    let stripes = &result.specimens[1];
    assert_eq!(
        (random.specimen_id.as_str(), stripes.specimen_id.as_str()),
        ("random", "stripes")
    );
    // floor(((200 + 1) / 10)^(1/2)) = 4 bins.
    assert_eq!((stripes.bins, stripes.minimum_disc_count), (4, 200));
    let find = |specimen: &marklab::space::SpaceCisSpecimen, names: &[&str]| {
        specimen.radii[0]
            .ensembles
            .iter()
            .find(|ensemble| ensemble.variables == names)
            .unwrap()
            .clone()
    };
    // Discs inside stripes are pure; shuffled discs approach the 50:50 average, so composition
    // and its covariation with Ki67 carry far more information than the null.
    for names in [&["cell_type.A"][..], &["cell_type.A", "ki67"]] {
        let ensemble = find(stripes, names);
        assert!(ensemble.cis_mi > 0.5, "{names:?}: {ensemble:?}");
        assert!(ensemble.z_score.unwrap() > 10.0, "{names:?}: {ensemble:?}");
        assert_eq!(ensemble.p_value_monte_carlo, 1.0 / 61.0);
        assert!(ensemble.p_adjust_bh < 1e-10);
    }
    // Random positions: class composition alone shows no co-patterning.
    for names in [
        &["cell_type.A"][..],
        &["cell_type.B"],
        &["cell_type.A", "cell_type.B"],
    ] {
        let ensemble = find(random, names);
        assert_eq!(ensemble.status, SpaceNullStatus::Available);
        assert!(ensemble.z_score.unwrap().abs() < 5.0, "{ensemble:?}");
    }
    // An independent scalar is shuffled separately from classes, as SPACE shuffles separate
    // image layers, so per-cell coupling of Ki67 to class A is detected even at random
    // positions. A linked scalar would move with its cell instead.
    assert!(find(random, &["cell_type.A", "ki67"]).z_score.unwrap() > 10.0);
    // Depth 1 and depth 2 ensembles of the three variables: 3 + 3.
    assert_eq!(stripes.radii[0].ensembles.len(), 6);
}

#[test]
fn ensemble_constraints_follow_r_measure_cis_mi() {
    let input = cis_input(json!([grid_specimen("stripes", 40, 8)]), 200);
    let names = |parameters: SpaceCisParameters| {
        space_cis(&input, &parameters).unwrap().specimens[0].radii[0]
            .ensembles
            .iter()
            .map(|ensemble| ensemble.variables.join("+"))
            .collect::<Vec<_>>()
    };
    let base = SpaceCisParameters {
        depth: 2,
        bootstraps: 4,
        ..SpaceCisParameters::default()
    };
    // `all` filters only the largest size; lower sizes stay for the subtraction.
    let all = names(SpaceCisParameters {
        all: vec!["ki67".into()],
        ..base.clone()
    });
    assert_eq!(
        all,
        [
            "cell_type.A",
            "cell_type.B",
            "ki67",
            "cell_type.A+ki67",
            "cell_type.B+ki67"
        ]
    );
    // `not` removes a variable everywhere.
    let not = names(SpaceCisParameters {
        not: vec!["cell_type.B".into()],
        ..base.clone()
    });
    assert_eq!(not, ["cell_type.A", "ki67", "cell_type.A+ki67"]);
    // When `all` and `alo` fill the depth, every other variable is dropped.
    let filled = names(SpaceCisParameters {
        all: vec!["ki67".into()],
        alo: vec!["cell_type.B".into()],
        ..base.clone()
    });
    assert_eq!(filled, ["cell_type.B", "ki67", "cell_type.B+ki67"]);

    let error = |parameters: SpaceCisParameters| space_cis(&input, &parameters).unwrap_err();
    assert!(matches!(
        error(SpaceCisParameters {
            all: vec!["ki67".into(), "cell_type.A".into()],
            alo: vec!["cell_type.B".into()],
            ..base.clone()
        }),
        SpaceError::Invalid(message) if message.contains("exceeds ensemble depth")
    ));
    assert!(matches!(
        error(SpaceCisParameters { not: vec!["cell_type.Z".into()], ..base.clone() }),
        SpaceError::Invalid(message) if message.contains("not a census variable")
    ));
    // Depth 3 needs 79 discs for two bins per variable; R would silently use one bin.
    let small = cis_input(json!([grid_specimen("stripes", 40, 8)]), 60);
    assert!(matches!(
        space_cis(&small, &SpaceCisParameters { depth: 3, bootstraps: 4, ..base }).unwrap_err(),
        SpaceError::Invalid(message) if message.contains("at least 79")
    ));
}

fn grouped(mut specimen: Value, groups: Value) -> Value {
    specimen["groups"] = groups;
    specimen
}

/// Three striped and three well-mixed specimens; `batch` cuts across the two patterns.
fn trans_specimens() -> Value {
    json!([
        grouped(
            grid_specimen("striped-1", 30, 4),
            json!({"pattern": "striped", "batch": "a"})
        ),
        grouped(
            grid_specimen("striped-2", 30, 5),
            json!({"pattern": "striped", "batch": "b"})
        ),
        grouped(
            grid_specimen("striped-3", 30, 6),
            json!({"pattern": "striped", "batch": "a"})
        ),
        grouped(
            grid_specimen("mixed-1", 30, 0),
            json!({"pattern": "mixed", "batch": "b"})
        ),
        grouped(
            grid_specimen("mixed-2", 30, 0),
            json!({"pattern": "mixed", "batch": "a"})
        ),
        grouped(
            grid_specimen("mixed-3", 30, 0),
            json!({"pattern": "mixed", "batch": "b"})
        ),
    ])
}

#[test]
fn trans_ranks_ensembles_that_separate_groups() {
    let input = cis_input(trans_specimens(), 200);
    let parameters = SpaceTransParameters {
        depth: 2,
        bootstraps: 200,
        ..SpaceTransParameters::default()
    };
    let result = space_trans(&input, &parameters).unwrap();
    let factors = result
        .factors
        .iter()
        .map(|factor| (factor.factor.as_str(), factor.levels.len()))
        .collect::<Vec<_>>();
    assert_eq!(factors, [("batch", 2), ("pattern", 2)]);
    // floor(((200 + 1) / 10)^(1/2)) = 4 bins; three variables give 3 + 3 ensembles.
    assert_eq!((result.bins, result.minimum_disc_count), (4, 200));
    let radius = &result.radii[0];
    assert_eq!(radius.disc_counts, [200; 6]);
    assert_eq!(radius.ensembles.len(), 6);
    // Each specimen is rounded on its own range, so striped discs (0 to 100% A) spread over
    // the extreme bins while mixed discs crowd the middle: composition separates the
    // patterns but not the batches. Six specimens give 15 divergences to shuffle, so the
    // null is wide and Z near 3 is close to the most a perfect split can reach.
    let class_a = &radius.ensembles[0];
    assert_eq!(class_a.variables, ["cell_type.A"]);
    let [batch, pattern] = [&class_a.factors[0], &class_a.factors[1]];
    assert_eq!(
        (batch.factor.as_str(), pattern.factor.as_str()),
        ("batch", "pattern")
    );
    assert!(pattern.trans_mi > batch.trans_mi, "{class_a:?}");
    assert!(pattern.z_score > 2.5, "{pattern:?}");
    assert!(pattern.p_value_monte_carlo <= 0.01, "{pattern:?}");
    assert!(batch.z_score.abs() < 1.0, "{batch:?}");
    assert_eq!(pattern.status, SpaceNullStatus::Available);

    let error = |parameters: SpaceTransParameters| space_trans(&input, &parameters).unwrap_err();
    assert!(matches!(
        error(SpaceTransParameters { factors: vec!["site".into()], ..parameters.clone() }),
        SpaceError::Invalid(message) if message.contains("no level for grouping factor site")
    ));
    assert!(matches!(
        error(SpaceTransParameters { specimens: vec!["mixed-1".into()], ..parameters }),
        SpaceError::Invalid(message) if message.contains("at least two specimens")
    ));
}

#[test]
fn dist_reports_census_values_and_their_smoothed_joint_distribution() {
    let input = cis_input(json!([grid_specimen("stripes", 40, 8)]), 200);
    let parameters = |variables: &[&str], bins, randomized| SpaceDistParameters {
        variables: variables.iter().map(|name| (*name).to_owned()).collect(),
        radius_um: 3.0,
        specimen: None,
        bins,
        randomized,
    };
    // Without bins, each disc's values are the census's.
    let census = space_census(&input).unwrap();
    let discs = &census.specimens[0].radii[0].discs;
    let observed = space_dist(&input, &parameters(&["cell_type.A", "ki67"], None, false)).unwrap();
    assert_eq!(observed.census, SpaceDistCensus::Observed);
    let SpaceDistValues::Discs { discs: values } = &observed.values else {
        panic!("expected disc values");
    };
    let names = &census.specimens[0].variables;
    let column = |name: &str| names.iter().position(|known| known == name).unwrap();
    assert_eq!(values.len(), 200);
    for (disc, row) in discs.iter().zip(values) {
        assert_eq!(
            row,
            &[
                disc.values[column("cell_type.A")],
                disc.values[column("ki67")]
            ]
        );
    }

    // Two classes of one channel on four bins: 16 cells, those summing above 100% removed,
    // the rest a probability distribution that keeps unobserved cells positive.
    let joint = space_dist(
        &input,
        &parameters(&["cell_type.A", "cell_type.B"], Some(4), false),
    )
    .unwrap();
    let SpaceDistValues::Distribution { axes, cells } = &joint.values else {
        panic!("expected a distribution");
    };
    assert!(axes
        .iter()
        .all(|axis| axis.rounded && axis.values.len() == 4));
    assert_eq!(cells.len(), 16);
    for cell in cells {
        let impossible = cell.values.iter().sum::<f64>() > 100.0;
        assert_eq!(cell.frequency.is_none(), impossible, "{cell:?}");
        assert!(cell.frequency.is_none_or(|frequency| frequency > 0.0));
    }
    let total = cells.iter().filter_map(|cell| cell.frequency).sum::<f64>();
    assert!((total - 1.0).abs() < 1e-12);
    // Pure stripe interiors dominate: 0% A with 100% B, and 100% A with 0% B.
    let frequency = |a: f64, b: f64| {
        cells
            .iter()
            .find(|cell| cell.values == [a, b])
            .and_then(|cell| cell.frequency)
            .unwrap()
    };
    assert!(frequency(0.0, 100.0) + frequency(100.0, 0.0) > 0.5);

    // With more bins than distinct values, those values are used unrounded.
    let distinct = space_dist(&input, &parameters(&["cell_type.A"], Some(255), false)).unwrap();
    let SpaceDistValues::Distribution { axes, cells } = &distinct.values else {
        panic!("expected a distribution");
    };
    assert!(!axes[0].rounded);
    let mut expected = values.iter().map(|row| row[0]).collect::<Vec<_>>();
    expected.sort_by(f64::total_cmp);
    expected.dedup();
    assert_eq!(axes[0].values, expected);
    assert_eq!(cells.len(), expected.len());

    // One randomized census: the same discs, members shuffled, so stripes no longer give
    // pure discs; the same seed gives the same census.
    let randomized = space_dist(&input, &parameters(&["cell_type.A"], None, true)).unwrap();
    assert_eq!(randomized.census, SpaceDistCensus::Randomized);
    assert_eq!(randomized.disc_count, 200);
    assert!(randomized.variables[0].maximum < 100.0);
    assert_eq!(
        randomized,
        space_dist(&input, &parameters(&["cell_type.A"], None, true)).unwrap()
    );

    let error = |parameters: SpaceDistParameters| space_dist(&input, &parameters).unwrap_err();
    assert!(matches!(
        error(parameters(&["cell_type.A", "cell_type.B", "ki67"], None, false)),
        SpaceError::Invalid(message) if message.contains("one or two variables")
    ));
    assert!(matches!(
        error(parameters(&["cell_type.A"], Some(1), false)),
        SpaceError::Invalid(message) if message.contains("between 2 and 255")
    ));
    assert!(matches!(
        error(parameters(&["cell_type.C"], None, false)),
        SpaceError::Invalid(message) if message.contains("no cells of variable cell_type.C")
    ));
}

#[test]
fn rank_filters_and_orders_as_r_plot_mi_rank() {
    let input = cis_input(
        json!([
            grid_specimen("stripes", 40, 8),
            grid_specimen("random", 40, 0)
        ]),
        200,
    );
    let cis = space_cis(
        &input,
        &SpaceCisParameters {
            depth: 2,
            bootstraps: 60,
            ..SpaceCisParameters::default()
        },
    )
    .unwrap();
    // The result is read back from its JSON, as the CLI does.
    let source = space_mi_result_from_json(&serde_json::to_vec(&cis).unwrap()).unwrap();
    assert_eq!(source, SpaceMiResult::Cis(cis.clone()));
    let rank = |parameters: SpaceRankParameters| space_rank(&source, &parameters);
    let base = SpaceRankParameters {
        radius_um: Some(3.0),
        specimen: Some("stripes".into()),
        ..SpaceRankParameters::default()
    };
    let result = rank(base.clone()).unwrap();
    let SpaceRankView::Ensembles {
        ensembles,
        variables,
        mi_threshold,
        ..
    } = &result.view
    else {
        panic!("expected a ranking");
    };
    // Kept: BH P <= 0.05 and |cisMI| >= 0.1 (R's cisMI default), by decreasing |Z|.
    let all = &cis.specimens[1].radii[0].ensembles;
    let expected = all
        .iter()
        .filter(|ensemble| ensemble.p_adjust_bh <= 0.05 && ensemble.cis_mi.abs() >= 0.1)
        .count();
    assert_eq!((*mi_threshold, ensembles.len()), (0.1, expected));
    assert!(expected > 0);
    let magnitudes = ensembles
        .iter()
        .map(|ensemble| ensemble.z_score.unwrap().abs())
        .collect::<Vec<_>>();
    assert!(magnitudes.windows(2).all(|pair| pair[0] >= pair[1]));
    // Each variable's score: its ensembles' summed |Z| over the number kept.
    for variable in variables {
        let sum = ensembles
            .iter()
            .filter(|ensemble| ensemble.variables.contains(&variable.name))
            .map(|ensemble| ensemble.z_score.unwrap().abs())
            .sum::<f64>();
        assert!((variable.score - sum / ensembles.len() as f64).abs() < 1e-12);
        assert_eq!(variable.unbounded_ensembles, 0);
    }
    assert!(variables
        .windows(2)
        .all(|pair| pair[0].score >= pair[1].score));

    let kept = |parameters: SpaceRankParameters| match rank(parameters).unwrap().view {
        SpaceRankView::Ensembles { ensembles, .. } => ensembles,
        SpaceRankView::Radii { .. } => panic!("expected a ranking"),
    };
    assert!(kept(SpaceRankParameters {
        not: vec!["ki67".into()],
        ..base.clone()
    })
    .iter()
    .all(|ensemble| !ensemble.variables.contains(&"ki67".to_owned())));
    assert!(kept(SpaceRankParameters {
        all: vec!["cell_type.A".into()],
        depths: vec![2],
        p_threshold: 1.0,
        mi_threshold: Some(0.0),
        ..base.clone()
    })
    .iter()
    .all(|ensemble| ensemble.variables.len() == 2
        && ensemble.variables.contains(&"cell_type.A".to_owned())));
    // R refuses more required variables than the smallest ranked depth.
    assert!(matches!(
        rank(SpaceRankParameters {
            all: vec!["cell_type.A".into()],
            alo: vec!["ki67".into()],
            ..base.clone()
        }),
        Err(SpaceError::Invalid(message)) if message.contains("smallest ensemble depth")
    ));

    // One ensemble across radii (R plot_MI_radius).
    let followed = rank(SpaceRankParameters {
        radius_um: None,
        ensemble: vec!["ki67".into(), "cell_type.A".into()],
        ..base.clone()
    })
    .unwrap();
    let SpaceRankView::Radii { radii, .. } = &followed.view else {
        panic!("expected radii");
    };
    let stored = all
        .iter()
        .find(|ensemble| ensemble.variables == ["cell_type.A", "ki67"])
        .unwrap();
    assert_eq!(radii.len(), 1);
    assert_eq!(
        (radii[0].mi, radii[0].z_score),
        (stored.cis_mi, stored.z_score)
    );

    // A transMI result ranks one factor; transMI keeps values at or above 0 by default.
    let trans = space_trans(
        &cis_input(trans_specimens(), 120),
        &SpaceTransParameters {
            depth: 2,
            bootstraps: 50,
            ..SpaceTransParameters::default()
        },
    )
    .unwrap();
    let trans = SpaceMiResult::Trans(trans);
    let parameters = SpaceRankParameters {
        radius_um: Some(3.0),
        factor: Some("pattern".into()),
        p_threshold: 1.0,
        ..SpaceRankParameters::default()
    };
    let SpaceRankView::Ensembles { ensembles, .. } = space_rank(&trans, &parameters).unwrap().view
    else {
        panic!("expected a ranking");
    };
    assert!(ensembles.iter().all(|ensemble| ensemble.mi >= 0.0));
    assert!(matches!(
        space_rank(&trans, &SpaceRankParameters { factor: None, ..parameters }),
        Err(SpaceError::Invalid(message)) if message.contains("name the grouping factor")
    ));
    assert!(matches!(
        space_mi_result_from_json(br#"{"format": "marklab.space_census", "version": 1}"#),
        Err(SpaceError::Invalid(message)) if message.contains("not a cisMI or transMI result")
    ));
}

#[cfg(feature = "parallel")]
#[test]
fn results_do_not_depend_on_thread_count() {
    let input = cis_input(trans_specimens(), 120);
    let cis = SpaceCisParameters {
        depth: 3,
        bootstraps: 20,
        specimens: vec!["mixed-1".into()],
        ..SpaceCisParameters::default()
    };
    let trans = SpaceTransParameters {
        depth: 2,
        bootstraps: 20,
        ..SpaceTransParameters::default()
    };
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                (
                    space_cis(&input, &cis).unwrap(),
                    space_trans(&input, &trans).unwrap(),
                )
            })
    };
    assert_eq!(run(1), run(4));
}

#[test]
fn cellvit_tables_and_multiplex_recipes_are_direct_sources() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    std::fs::write(
        path.join("cells.csv"),
        "cell_id,x_centroid_um,y_centroid_um,predicted_class,class_probability\n\
         a,0,0,Tumor,0.9\nb,1,0,lymphocyte,0.8\nc,2,0,tumour,0.7\nd,0,1.5,Lymphocyte,0.95\n",
    )
    .unwrap();
    std::fs::write(path.join("window.geojson"), square(6.0).to_string()).unwrap();
    let census = json!({"radii_um": [2.0], "seed": 3, "seed_cell_ids": {"slide": [["b"]]}});
    let cellvit = json!({
        "format": "marklab.space_request", "version": 1,
        "source": {"kind": "cellvit_he_csv", "specimens": [{"path": "cells.csv",
            "window_path": "window.geojson", "specimen_id": "slide",
            "coordinate_frame_id": "slide-um", "groups": {"status": "biopsy"}}]},
        "objects": ["cellvit_class"],
        "linked_scalars": [{"channel": "cellvit_class", "class": "tumor",
            "scalar": "class_probability"}],
        "census": census
    });
    std::fs::write(path.join("cellvit.json"), cellvit.to_string()).unwrap();
    let from_csv = space_census(&load_space_input(&path.join("cellvit.json")).unwrap()).unwrap();
    // The same cells declared inline, with CellViT's normalized class names in sorted order.
    let inline = json!({
        "format": "marklab.space_request", "version": 1,
        "source": {"kind": "inline", "channels": [
            {"id": "cellvit_class", "label": "class", "kind": "categorical",
             "unit": "categorical", "measurement_status": "imported_prediction",
             "provenance": "inline", "levels": ["lymphocyte", "tumor"]},
            {"id": "class_probability", "label": "probability", "kind": "continuous",
             "unit": "probability", "measurement_status": "imported_prediction",
             "provenance": "inline"}],
            "specimens": [{"specimen_id": "slide", "coordinate_frame_id": "slide-um",
                "groups": {"status": "biopsy"}, "window": square(6.0),
                "cell_ids": ["a", "b", "c", "d"],
                "coordinates_um": [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [0.0, 1.5]],
                "observations": {"cellvit_class": [1.0, 0.0, 1.0, 0.0],
                    "class_probability": [0.9, 0.8, 0.7, 0.95]}}]},
        "objects": ["cellvit_class"],
        "linked_scalars": [{"channel": "cellvit_class", "class": "tumor",
            "scalar": "class_probability"}],
        "census": census
    });
    let from_inline =
        space_census(&space_input_from_json(&serde_json::to_vec(&inline).unwrap()).unwrap())
            .unwrap();
    assert_eq!(from_csv.specimens, from_inline.specimens);
    assert_eq!(
        from_csv.specimens[0].radii[0].discs[0].values,
        [50.0, 50.0, (0.9 + 0.7) / 2.0]
    );
    // Several tables share one class list, so a class means the same in every specimen.
    std::fs::write(
        path.join("second.csv"),
        "cell_id,x_centroid_um,y_centroid_um,predicted_class,class_probability\n\
         e,1,1,Stroma,0.6\nf,2,1,Tumor,0.9\n",
    )
    .unwrap();
    let mut two = cellvit.clone();
    two["source"]["specimens"].as_array_mut().unwrap().push(
        json!({"path": "second.csv", "window_path": "window.geojson",
            "specimen_id": "slide-2", "coordinate_frame_id": "slide-2-um"}),
    );
    two["census"] = json!({"radii_um": [2.0], "seed": 3,
        "seed_cell_ids": {"slide": [["b"]], "slide-2": [["e"]]}});
    std::fs::write(path.join("two.json"), two.to_string()).unwrap();
    let from_two = space_census(&load_space_input(&path.join("two.json")).unwrap()).unwrap();
    assert_eq!(from_two.specimens[0], from_csv.specimens[0]);
    assert_eq!(
        from_two.specimens[1].variables,
        [
            "cellvit_class.stroma",
            "cellvit_class.tumor",
            "cellvit_class.tumor_class_probability"
        ]
    );

    // The repository's synthetic multiplex recipe: slides become specimens with their group.
    let recipe = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/multiplex-study/recipe.json");
    let request = |scalar: &str| {
        json!({
            "format": "marklab.space_request", "version": 1,
            "source": {"kind": "multiplex_study_recipe", "path": recipe},
            "objects": ["cell_class"], "independent_scalars": [scalar],
            "census": {"radii_um": [1.5], "sample_size": [2], "seed": 5}
        })
    };
    std::fs::write(path.join("recipe-request.json"), request("CD3").to_string()).unwrap();
    let result =
        space_census(&load_space_input(&path.join("recipe-request.json")).unwrap()).unwrap();
    assert!(result.specimens.len() > 1);
    assert!(result
        .specimens
        .iter()
        .all(|specimen| specimen.groups.contains_key("group")));
    // A scalar with missing values is refused rather than read as zero.
    std::fs::write(path.join("missing.json"), request("phenotype").to_string()).unwrap();
    assert!(matches!(
        load_space_input(&path.join("missing.json")),
        Err(SpaceError::Invalid(message)) if message.contains("missing phenotype")
    ));
}

#[test]
fn cli_publishes_json_and_tables_once() {
    let directory = tempfile::tempdir().unwrap();
    let request = directory.path().join("request.json");
    std::fs::write(
        &request,
        inline_request(
            json!([grid_specimen("stripes", 30, 6)]),
            json!({"census": {"radii_um": [2.0, 3.0], "sample_size": [100, 100], "seed": 1}}),
        ),
    )
    .unwrap();
    let out = directory.path().join("cis.json");
    let table = directory.path().join("cis.csv");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_marklab"))
            .args(["space", "cis", "--request"])
            .arg(&request)
            .arg("--out")
            .arg(&out)
            .arg("--table")
            .arg(&table)
            .args(["--depth", "2", "--bootstraps", "10", "--radii", "3"])
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let result: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.space_cis_result");
    assert_eq!(result["specimens"][0]["radii"].as_array().unwrap().len(), 1);
    let csv = std::fs::read_to_string(&table).unwrap();
    assert!(csv.starts_with("specimen_id,radius_um,depth,v1,v2,observed_mi,"));
    // Depth 1 and 2 ensembles of two classes: 2 + 1 rows.
    assert_eq!(csv.lines().count(), 1 + 3);
    let second = run();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already exists"));

    // The cisMI result ranked, and one distribution, each with its tables.
    let path = |name: &str| directory.path().join(name);
    let marklab = |arguments: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_marklab"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let text = |name: &str| path(name).to_str().unwrap().to_owned();
    marklab(&[
        "space",
        "rank",
        "--result",
        &text("cis.json"),
        "--radius",
        "3",
        "--p-threshold",
        "1",
        "--out",
        &text("rank.json"),
        "--table",
        &text("rank.csv"),
        "--variable-table",
        &text("rank-variables.csv"),
    ]);
    let ranked: Value = serde_json::from_slice(&std::fs::read(path("rank.json")).unwrap()).unwrap();
    assert_eq!(ranked["view"]["kind"], "ensembles");
    let csv = std::fs::read_to_string(path("rank.csv")).unwrap();
    assert!(csv.starts_with("rank,depth,v1,v2,mi,z_score,"));
    let csv = std::fs::read_to_string(path("rank-variables.csv")).unwrap();
    assert!(csv.starts_with("rank,variable,score,unbounded_ensembles,ensembles\n"));
    marklab(&[
        "space",
        "dist",
        "--request",
        &text("request.json"),
        "--variables",
        "cell_type.A,cell_type.B",
        "--radius",
        "3",
        "--bins",
        "4",
        "--out",
        &text("dist.json"),
        "--table",
        &text("dist.csv"),
    ]);
    let csv = std::fs::read_to_string(path("dist.csv")).unwrap();
    assert!(csv.starts_with("cell_type.A,cell_type.B,frequency\n"));
    assert_eq!(csv.lines().count(), 1 + 16);

    let request = directory.path().join("trans-request.json");
    std::fs::write(
        &request,
        inline_request(
            trans_specimens(),
            json!({"census": {"radii_um": [3.0], "sample_size": [100], "seed": 1}}),
        ),
    )
    .unwrap();
    let (out, table) = (
        directory.path().join("trans.json"),
        directory.path().join("trans.csv"),
    );
    let trans = Command::new(env!("CARGO_BIN_EXE_marklab"))
        .args(["space", "trans", "--request"])
        .arg(&request)
        .arg("--out")
        .arg(&out)
        .arg("--table")
        .arg(&table)
        .args(["--depth", "1", "--bootstraps", "10", "--factor", "pattern"])
        .output()
        .unwrap();
    assert!(
        trans.status.success(),
        "{}",
        String::from_utf8_lossy(&trans.stderr)
    );
    let result: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.space_trans_result");
    let csv = std::fs::read_to_string(&table).unwrap();
    assert!(csv.starts_with("radius_um,depth,v1,factor,trans_mi,"));
    // Two classes at depth 1, one factor.
    assert_eq!(csv.lines().count(), 1 + 2);
}
