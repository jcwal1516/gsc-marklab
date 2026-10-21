use serde_json::{json, Value};

fn request(window: Value, cells: &[(f64, f64)], origin: [f64; 2], width: f64) -> Vec<u8> {
    let cells = cells
        .iter()
        .enumerate()
        .map(|(index, (x, y))| json!({"id": format!("c{index}"), "x_um": x, "y_um": y}))
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "format": "marklab.morisita_dispersion_request", "version": 1,
        "coordinate_frame_id": "slide-um", "window": window, "cells": cells,
        "quadrat": {"origin_um": origin, "width_um": width},
        "limits": {"maximum_cells": 1000, "maximum_candidate_quadrats": 1000,
            "maximum_clip_vertices": 1000}
    }))
    .unwrap()
}

fn rectangle(x1: f64, y1: f64) -> Value {
    json!({"type": "MultiPolygon", "coordinates": [[[[0, 0], [x1, 0], [x1, y1], [0, y1], [0, 0]]]]})
}

#[test]
fn quadrats_tile_the_window_exactly_and_cells_use_half_open_cells() {
    // A 300 x 200 rectangle tiled by 100 um quadrats from its corner: all six quadrats touch
    // the boundary and pass the exact clip. The cell at x = 100 belongs to column 1.
    let cells = [
        (10.0, 10.0),
        (20.0, 30.0),
        (100.0, 50.0),
        (250.0, 150.0),
        (299.0, 199.0),
    ];
    let result = marklab::structure::morisita_dispersion_from_json(&request(
        rectangle(300.0, 200.0),
        &cells,
        [0.0, 0.0],
        100.0,
    ))
    .unwrap();
    assert_eq!(result.included_quadrats, 6);
    assert_eq!(result.counted_cells, 5);
    let count = |column, row| {
        result
            .quadrat_counts
            .iter()
            .find(|q| q.column == column && q.row == row)
            .unwrap()
            .count
    };
    assert_eq!((count(0, 0), count(1, 0), count(2, 1)), (2, 1, 2));
    // q = 6, N = 5, sum n(n-1) = 2 + 2: I = 6 * 4 / 20.
    assert_eq!(result.morisita_index, Some(6.0 * 4.0 / 20.0));

    // An L-shaped window: quadrats crossing the notch are excluded with their cells.
    let l_shape = json!({"type": "MultiPolygon", "coordinates": [[[[0, 0], [300, 0], [300, 100],
        [150, 100], [150, 200], [0, 200], [0, 0]]]]});
    let result = marklab::structure::morisita_dispersion_from_json(&request(
        l_shape,
        &[(10.0, 10.0), (20.0, 30.0), (100.0, 50.0), (120.0, 150.0)],
        [0.0, 0.0],
        100.0,
    ))
    .unwrap();
    // Inside: (0,0), (1,0), (2,0), (0,1); (1,1) straddles x = 150 and its cell is excluded.
    assert_eq!(result.included_quadrats, 4);
    assert_eq!(result.excluded_cells, 1);
    let shifted = marklab::structure::morisita_dispersion_from_json(&request(
        rectangle(300.0, 200.0),
        &cells,
        [50.0, 0.0],
        100.0,
    ))
    .unwrap();
    // Origin 50: columns [50,150) and [150,250) fit; the cells at x = 10, 20, 250, 299 fall in
    // partial quadrats and are excluded.
    assert_eq!(shifted.included_quadrats, 4);
    assert_eq!(shifted.excluded_cells, 4);
}

#[test]
fn malformed_morisita_requests_are_rejected() {
    let outside = request(rectangle(100.0, 100.0), &[(150.0, 50.0)], [0.0, 0.0], 50.0);
    assert!(marklab::structure::morisita_dispersion_from_json(&outside).is_err());
    let zero_width = request(rectangle(100.0, 100.0), &[(50.0, 50.0)], [0.0, 0.0], 0.0);
    assert!(marklab::structure::morisita_dispersion_from_json(&zero_width).is_err());
}

fn kriging_request(search_radius: f64) -> Vec<u8> {
    // Block (0, 0) holds the value 1 and block (1, 0) the value 3, 100 um blocks from 0.
    let observations = (0..8)
        .map(|index| {
            let (x, value) = if index < 4 { (20.0, 1.0) } else { (120.0, 3.0) };
            json!({"id": format!("o{index}"), "x_um": x + 10.0 * (index % 2) as f64,
                "y_um": 20.0 + 15.0 * (index % 4) as f64, "value": value})
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "format": "marklab.ordinary_kriging_request", "version": 1,
        "coordinate_frame_id": "slide-um", "value_name": "marker", "value_unit": "a.u.",
        "observations": observations,
        "variogram": {"model": "spherical", "range_um": 80.0, "partial_sill": 1.0, "nugget": 0.1},
        "neighborhood": {"search_radius_um": search_radius, "minimum_neighbors": 2,
            "maximum_neighbors": 6},
        "cross_validation": {"block_width_um": 100.0, "origin_um": [0.0, 0.0]},
        "targets": [{"id": "t", "x_um": 25.0, "y_um": 40.0}],
        "limits": {"maximum_observations": 100, "maximum_targets": 10,
            "maximum_distance_evaluations": 1000}
    }))
    .unwrap()
}

#[test]
fn kriging_hold_out_uses_only_other_blocks() {
    let result = marklab::structure::ordinary_kriging_from_json(&kriging_request(300.0)).unwrap();
    assert_eq!(result.cross_validation.blocks, 2);
    // Weights sum to one, so a held-out block is predicted exactly by the other block's
    // constant value: block (0, 0) as 3 and block (1, 0) as 1.
    for row in &result.cross_validation.held_out {
        let expected = if row.block == [0, 0] { 3.0 } else { 1.0 };
        assert!((row.prediction.prediction.unwrap() - expected).abs() < 1e-12);
    }
    assert_eq!(result.cross_validation.summary.available, 8);
    assert!((result.cross_validation.summary.rmse.unwrap() - 2.0).abs() < 1e-12);
    // A radius that excludes the other block leaves every held-out prediction unavailable.
    let isolated = marklab::structure::ordinary_kriging_from_json(&kriging_request(60.0)).unwrap();
    assert_eq!(isolated.cross_validation.summary.available, 0);
    assert!(isolated.cross_validation.summary.rmse.is_none());
    assert!(isolated.targets[0].prediction.is_some());
}

fn bond_request(points: &[(f64, f64)], symmetry: u32, rule: Value, extent: f64) -> Vec<u8> {
    let cells = points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| json!({"id": format!("c{index}"), "x_um": x, "y_um": y}))
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "format": "marklab.bond_orientational_order_request", "version": 1,
        "coordinate_frame_id": "slide-um",
        "window": rectangle(extent, extent), "cells": cells, "symmetry": symmetry,
        "neighbors": rule, "limits": {"maximum_cells": 10000}
    }))
    .unwrap()
}

#[test]
fn lattices_have_their_known_k_atic_order_and_border_cells_are_excluded() {
    // Triangular lattice, spacing 10 um, rotated by 0.1 rad: interior cells have psi_6 = e^{0.6 i}.
    let (spacing, angle) = (10.0_f64, 0.1_f64);
    let mut hexagonal = Vec::new();
    // The lattice fills the whole window, as the border rule assumes of observed cells.
    for row in -20..=20 {
        for column in -20..=20 {
            let x = spacing * (column as f64 + 0.5 * row as f64);
            let y = spacing * row as f64 * 3f64.sqrt() / 2.0;
            let (xr, yr) = (
                x * angle.cos() - y * angle.sin(),
                x * angle.sin() + y * angle.cos(),
            );
            if xr.abs() < 100.0 && yr.abs() < 100.0 {
                hexagonal.push((xr + 100.0, yr + 100.0));
            }
        }
    }
    let result = marklab::structure::bond_orientational_order_from_json(&bond_request(
        &hexagonal,
        6,
        json!({"kind": "nearest_count", "count": 6}),
        200.0,
    ))
    .unwrap();
    assert!(result.included_cells > 100 && result.excluded_cells > 0);
    for cell in result.cells.iter().filter(|cell| cell.included) {
        assert!((cell.psi_real.unwrap() - (6.0 * angle).cos()).abs() < 1e-12);
        assert!((cell.psi_imaginary.unwrap() - (6.0 * angle).sin()).abs() < 1e-12);
    }
    assert!((result.magnitude_of_mean.unwrap() - 1.0).abs() < 1e-12);

    // Square lattice: four nearest neighbors give psi_4 = 1 and psi_6 = 0.
    let square = (0..20)
        .flat_map(|i| (0..20).map(move |j| (5.0 + 10.0 * i as f64, 5.0 + 10.0 * j as f64)))
        .collect::<Vec<_>>();
    for (symmetry, expected) in [(4, 1.0), (6, 0.0)] {
        let result = marklab::structure::bond_orientational_order_from_json(&bond_request(
            &square,
            symmetry,
            json!({"kind": "radius", "radius_um": 10.5}),
            200.0,
        ))
        .unwrap();
        assert!((result.mean_magnitude.unwrap() - expected).abs() < 1e-12);
        // Cells within 10.5 um of the boundary (the outer ring) are excluded.
        assert_eq!(result.excluded_cells, 4 * 19);
    }
}

fn mask_request(rows: &[String], sizes: &[usize]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "format": "marklab.lacunarity_fractal_dimension_request", "version": 1,
        "mask_id": "m", "pixel_size_um": 0.5, "foreground_definition": "tumor",
        "rows": rows, "box_sizes_px": sizes,
        "limits": {"maximum_pixels": 10000, "maximum_box_sizes": 10}
    }))
    .unwrap()
}

#[test]
fn masks_have_their_known_dimension_and_lacunarity() {
    // Sierpinski carpet of level 4: boxes of side 3^j tile it with 8^(4-j) occupied boxes.
    let carpet = (0..81)
        .map(|row: usize| {
            (0..81)
                .map(|column: usize| {
                    let hole = (0..4).any(|level| {
                        let scale = 3_usize.pow(level);
                        (row / scale) % 3 == 1 && (column / scale) % 3 == 1
                    });
                    if hole {
                        '0'
                    } else {
                        '1'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let result = marklab::structure::lacunarity_fractal_dimension_from_json(&mask_request(
        &carpet,
        &[1, 3, 9, 27, 81],
    ))
    .unwrap();
    let occupied = result
        .scales
        .iter()
        .map(|s| s.occupied_boxes)
        .collect::<Vec<_>>();
    assert_eq!(occupied, [4096, 512, 64, 8, 1]);
    assert!((result.box_counting_dimension.unwrap() - 8f64.ln() / 3f64.ln()).abs() < 1e-12);
    assert!((result.regression_r_squared.unwrap() - 1.0).abs() < 1e-12);
    assert_eq!(result.scales[1].box_size_um, 1.5);

    // Checkerboard: single pixels have mass 0 or 1 (lacunarity 2); every 2 x 2 box has mass 2.
    let checker = (0..6)
        .map(|row| {
            (0..6)
                .map(|c| if (row + c) % 2 == 0 { '1' } else { '0' })
                .collect()
        })
        .collect::<Vec<String>>();
    let result = marklab::structure::lacunarity_fractal_dimension_from_json(&mask_request(
        &checker,
        &[1, 2],
    ))
    .unwrap();
    assert_eq!(result.scales[0].lacunarity, Some(2.0));
    assert_eq!(result.scales[1].lacunarity, Some(1.0));
    assert_eq!(result.scales[1].gliding_positions, 25);

    // Boxes past the right and bottom edges are partial: one corner pixel of a 5 x 5 mask
    // occupies one of the nine size-2 boxes. An empty mask has no lacunarity or dimension.
    let mut corner = vec!["00000".to_string(); 5];
    corner[4] = "00001".into();
    let result = marklab::structure::lacunarity_fractal_dimension_from_json(&mask_request(
        &corner,
        &[1, 2, 5],
    ))
    .unwrap();
    let occupied = result
        .scales
        .iter()
        .map(|s| s.occupied_boxes)
        .collect::<Vec<_>>();
    assert_eq!(occupied, [1, 1, 1]);
    assert_eq!(result.box_counting_dimension, Some(0.0));
    // Constant counts have no variance to explain, so R^2 = 1 - RSS / TSS is undefined.
    assert!(result.regression_r_squared.is_none());
    let empty = vec!["000".to_string(); 3];
    let result =
        marklab::structure::lacunarity_fractal_dimension_from_json(&mask_request(&empty, &[1, 3]))
            .unwrap();
    assert!(result.box_counting_dimension.is_none() && result.scales[0].lacunarity.is_none());

    // Ragged rows, foreign characters and box sizes that are not increasing or exceed a side.
    for (rows, sizes) in [
        (vec!["11".to_string(), "1".to_string()], vec![1]),
        (vec!["12".to_string(), "11".to_string()], vec![1]),
        (vec!["11".to_string(), "11".to_string()], vec![2, 1]),
        (vec!["11".to_string(), "11".to_string()], vec![3]),
    ] {
        assert!(
            marklab::structure::lacunarity_fractal_dimension_from_json(&mask_request(
                &rows, &sizes
            ))
            .is_err()
        );
    }
}

fn information_request(marks: &[(f64, f64)], null_model: Value, permutations: usize) -> Vec<u8> {
    let cells = marks
        .iter()
        .enumerate()
        .map(|(index, (first, second))| {
            json!({"id": format!("c{index}"), "x_um": 10.0 * index as f64, "y_um": 5.0,
                "first": first, "second": second})
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "format": "marklab.mutual_information_request", "version": 1,
        "coordinate_frame_id": "slide-um", "first_name": "Ki67", "second_name": "PD-L1",
        "cells": cells, "neighbors_k": 1, "null": null_model,
        "permutations": permutations, "seed": 7,
        "limits": {"maximum_cells": 100, "maximum_distance_evaluations": 1000000,
            "maximum_variogram_pairs": 1000}
    }))
    .unwrap()
}

#[test]
fn mutual_information_has_its_hand_computed_value_and_blocks_restrict_the_null() {
    let blocks = |width: f64| {
        json!({"kind": "block_permutation", "origin_um": [0.0, 0.0],
        "width_um": width})
    };
    // Marks on the Golomb ruler {0, 1, 3, 7} have distinct pairwise differences, so equal
    // distances come from the same values and floating-point rounding cannot split a tie.
    // Identical marks with k = 1: no cell has another strictly closer than its nearest joint
    // neighbor, so I = psi(4) - psi(1) = 1 + 1/2 + 1/3.
    let same = [(0.0, 0.0), (1.0, 1.0), (3.0, 3.0), (7.0, 7.0)];
    let result =
        marklab::marks::mutual_information_from_json(&information_request(&same, blocks(1.0), 0))
            .unwrap();
    assert!((result.estimate_nats - 11.0 / 6.0).abs() < 1e-12);
    assert!(result.upper_p_value.is_none());

    // Hand count for (0,0), (1,7), (3,1), (7,3): epsilon = 3, 6, 3, 4, n_x = 1, 2, 1, 0 and
    // n_y = 1, 1, 2, 2, so I = H(3) - (1 + 1 + 3/2 + 1 + 1 + 3/2 + 0 + 3/2) / 4 = -7/24. The
    // estimate is left negative: bias near independence, not a negative information.
    let mixed = [(0.0, 0.0), (1.0, 7.0), (3.0, 1.0), (7.0, 3.0)];
    // Cells are 10 um apart, so 5 um blocks hold one cell each and the null cannot move a mark.
    let singletons =
        marklab::marks::mutual_information_from_json(&information_request(&mixed, blocks(5.0), 19))
            .unwrap();
    assert!((singletons.estimate_nats + 7.0 / 24.0).abs() < 1e-12);
    assert_eq!(singletons.singleton_blocks, Some(4));
    assert!((singletons.null_mean_nats.unwrap() - singletons.estimate_nats).abs() < 1e-15);
    assert!(singletons.null_standard_deviation_nats.unwrap() < 1e-15);
    assert_eq!(singletons.upper_p_value, Some(1.0));

    // One block is the ordinary permutation test; the seed fixes the draws.
    let pooled = information_request(&mixed, blocks(1000.0), 19);
    let first = marklab::marks::mutual_information_from_json(&pooled).unwrap();
    assert_eq!(first.occupied_blocks, Some(1));
    assert!(first.null_standard_deviation_nats.unwrap() > 0.0);
    assert_eq!(
        first,
        marklab::marks::mutual_information_from_json(&pooled).unwrap()
    );

    // A cell with k identical joint neighbors has epsilon = 0, outside the continuous estimator.
    let tied = [(0.0, 0.0), (0.0, 0.0), (1.0, 3.0), (3.0, 1.0)];
    assert!(
        marklab::marks::mutual_information_from_json(&information_request(&tied, blocks(1.0), 0))
            .is_err()
    );
}

#[test]
fn variogram_matching_surrogates_are_reproducible_and_bounded() {
    // Twenty cells on a line, 10 um apart, with a smooth trend in the second mark.
    let marks = (0..20)
        .map(|i| ((i * 7 % 20) as f64 + 0.1 * i as f64, (i as f64 / 3.0).sin()))
        .collect::<Vec<_>>();
    let null = |edges: Value| {
        json!({"kind": "variogram_matching",
        "smoothing_neighbors": [1, 3, 6], "lag_edges_um": edges})
    };
    let request = information_request(&marks, null(json!([0.0, 15.0, 35.0, 60.0])), 9);
    let first = marklab::marks::mutual_information_from_json(&request).unwrap();
    assert_eq!(
        first,
        marklab::marks::mutual_information_from_json(&request).unwrap()
    );
    let selected = first.selected_smoothing_neighbors.as_ref().unwrap();
    assert_eq!(selected.iter().sum::<usize>(), 9);
    // Pairs at 10, 20, ..., 60 um: 19 + 18 + 17 + 16 + 15 + 14 of them.
    assert_eq!(first.variogram_pairs, Some(99));
    assert!(first.occupied_blocks.is_none() && first.upper_p_value.is_some());
    // One lag bin cannot be fitted; a declared pair budget below the pair count is refused.
    let single_bin = information_request(&marks, null(json!([0.0, 15.0])), 9);
    assert!(marklab::marks::mutual_information_from_json(&single_bin).is_err());
    let mut tight: Value = serde_json::from_slice(&request).unwrap();
    tight["limits"]["maximum_variogram_pairs"] = json!(50);
    assert!(
        marklab::marks::mutual_information_from_json(&serde_json::to_vec(&tight).unwrap()).is_err()
    );
}

fn mapper_request(threshold: f64, minimum_shared: usize, lens_constant: bool) -> Vec<u8> {
    // Ten cells whose lens and single feature are 0, 1, ..., 9.
    let cells = (0..10)
        .map(|i| {
            let lens = if lens_constant { 1.0 } else { i as f64 };
            json!({"id": format!("c{i}"), "lens": lens, "features": [i as f64]})
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "format": "marklab.mapper_graph_request", "version": 1,
        "lens_name": "depth", "feature_names": ["marker"], "cells": cells,
        "cover": {"intervals": 2, "overlap_fraction": 0.5},
        "distance_threshold": threshold, "minimum_shared_cells": minimum_shared,
        "limits": {"maximum_cells": 100, "maximum_distance_evaluations": 1000}
    }))
    .unwrap()
}

#[test]
fn mapper_cover_clusters_and_nerve_have_their_known_graph() {
    // Range 9, two bins of 4.5 and overlap 0.5: intervals [-2.25, 6.75] and [2.25, 11.25]
    // hold cells 0-6 and 3-9. With threshold 1 each interval is one chain-linked cluster,
    // and the two clusters share cells 3-6.
    let result =
        marklab::structure::mapper_graph_from_json(&mapper_request(1.0, 1, false)).unwrap();
    assert_eq!(
        (result.intervals[0].lower, result.intervals[1].upper),
        (-2.25, 11.25)
    );
    let members = result
        .nodes
        .iter()
        .map(|node| node.members.join(","))
        .collect::<Vec<_>>();
    assert_eq!(members, ["c0,c1,c2,c3,c4,c5,c6", "c3,c4,c5,c6,c7,c8,c9"]);
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].shared_cells, 4);
    assert_eq!(result.components, 1);
    // A threshold below the spacing leaves single cells: 14 nodes, the 4 shared cells link
    // their two copies, so 10 components remain.
    let result =
        marklab::structure::mapper_graph_from_json(&mapper_request(0.5, 1, false)).unwrap();
    assert_eq!(
        (result.nodes.len(), result.edges.len(), result.components),
        (14, 4, 10)
    );
    // Requiring 5 shared cells removes the only edge of the threshold-1 graph.
    let result =
        marklab::structure::mapper_graph_from_json(&mapper_request(1.0, 5, false)).unwrap();
    assert_eq!((result.edges.len(), result.components), (0, 2));
    // A constant lens has no cover.
    assert!(marklab::structure::mapper_graph_from_json(&mapper_request(1.0, 1, true)).is_err());
}

fn landy_szalay_request(randoms: &[[f64; 2]]) -> Vec<u8> {
    let cells = [(0.0, 0.0), (1.0, 0.0), (3.0, 0.0)]
        .iter()
        .enumerate()
        .map(|(index, (x, y))| json!({"id": format!("c{index}"), "x_um": x, "y_um": y}))
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "format": "marklab.landy_szalay_request", "version": 1,
        "coordinate_frame_id": "slide-um",
        "window": {"type": "MultiPolygon", "coordinates": [[[[-1, -1], [10, -1], [10, 10],
            [-1, 10], [-1, -1]]]]},
        "cells": cells, "random_catalog_id": "hand", "random_points": randoms,
        "bin_edges_um": [0.0, 2.0, 4.0],
        "limits": {"maximum_cells": 10, "maximum_random_points": 10,
            "maximum_pair_evaluations": 1000}
    }))
    .unwrap()
}

#[test]
fn landy_szalay_counts_pairs_once_in_half_open_bins() {
    // Cells at x = 0, 1, 3 and randoms (0, 1), (2, 0), (9, 9). Distances in [0, 2) and [2, 4):
    // DD 1 and 2 (1; 2, 3), DR 4 and 2 (1, 1.41, 1, 1; 2, 3.16), RR 0 and 1 (2.24); the pair at
    // exactly 2 um falls in the second bin. With n = n_R = 3, the second bin has
    // xi = (2/3 - 2 * 2/9 + 1/3) / (1/3) = 5/3; the first has no random pairs.
    let result = marklab::structure::landy_szalay_from_json(&landy_szalay_request(&[
        [0.0, 1.0],
        [2.0, 0.0],
        [9.0, 9.0],
    ]))
    .unwrap();
    let counts = |field: fn(&marklab::structure::CorrelationBin) -> u64| {
        result.bins.iter().map(field).collect::<Vec<_>>()
    };
    assert_eq!(counts(|bin| bin.data_data_pairs), [1, 2]);
    assert_eq!(counts(|bin| bin.data_random_pairs), [4, 2]);
    assert_eq!(counts(|bin| bin.random_random_pairs), [0, 1]);
    assert!(result.bins[0].xi.is_none());
    assert!((result.bins[1].xi.unwrap() - 5.0 / 3.0).abs() < 1e-12);
    // Random points must sample the window.
    let outside = landy_szalay_request(&[[0.0, 1.0], [20.0, 0.0]]);
    assert!(marklab::structure::landy_szalay_from_json(&outside).is_err());
}

fn structure_factor_request(points: &[(f64, f64)]) -> Vec<u8> {
    let cells = points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| json!({"id": format!("c{index}"), "x_um": x, "y_um": y}))
        .collect::<Vec<_>>();
    // A 10 um square; k_max = pi gives |m| <= 5 on each axis.
    serde_json::to_vec(&json!({
        "format": "marklab.hyperuniformity_request", "version": 1,
        "coordinate_frame_id": "slide-um",
        "window": {"origin_um": [100.0, 200.0], "side_um": [10.0, 10.0]},
        "cells": cells, "maximum_wavenumber_per_axis": std::f64::consts::PI,
        "wavenumber_bin_edges": [0.0, 1.0, 2.0, 3.0, 4.0, 5.0], "fit_wavenumber_stop": 3.0,
        "limits": {"maximum_cells": 1000, "maximum_wavevector_evaluations": 1000000}
    }))
    .unwrap()
}

#[test]
fn structure_factor_vanishes_for_a_filling_lattice_and_matches_two_points() {
    // A 10 x 10 lattice of spacing 1 filling the square: every allowed wavevector with
    // |m| <= 5 misses the reciprocal lattice (multiples of 10), so S = 0 and H = 0.
    let lattice = (0..100)
        .map(|i| (100.5 + (i % 10) as f64, 200.5 + (i / 10) as f64))
        .collect::<Vec<_>>();
    let result =
        marklab::structure::hyperuniformity_from_json(&structure_factor_request(&lattice)).unwrap();
    assert_eq!(result.wavevectors.len(), 11 * 11 - 1);
    assert!(result
        .wavevectors
        .iter()
        .all(|value| value.structure_factor < 1e-20));
    assert!(result.h_index.unwrap().abs() < 1e-12 && result.first_peak_bin.is_none());

    // Two cells 2.5 um apart along x: S = 1 + cos(2 pi m_x 2.5 / 10), whatever m_y is.
    let result = marklab::structure::hyperuniformity_from_json(&structure_factor_request(&[
        (101.0, 205.0),
        (103.5, 205.0),
    ]))
    .unwrap();
    for value in &result.wavevectors {
        let expected = 1.0 + (std::f64::consts::TAU * value.m[0] as f64 * 0.25).cos();
        assert!((value.structure_factor - expected).abs() < 1e-12);
    }
    // A cell on the far edge is outside the half-open rectangle.
    assert!(
        marklab::structure::hyperuniformity_from_json(&structure_factor_request(&[
            (101.0, 205.0),
            (110.0, 205.0),
        ]))
        .is_err()
    );
}
