use serde_json::{json, Value};

#[cfg(feature = "cli")]
use std::{fs, process::Command};
#[cfg(feature = "cli")]
use tempfile::tempdir;

fn rectangle_with_optional_hole(hole: Option<[[f64; 2]; 5]>) -> Value {
    let mut rings = vec![json!([[0, 0], [100, 0], [100, 100], [0, 100], [0, 0]])];
    if let Some(hole) = hole {
        rings.push(json!(hole));
    }
    json!({"type": "MultiPolygon", "coordinates": [rings]})
}

fn request(cells: Value, field: Value, window: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "format": "marklab.nematic_orientation_request",
        "version": 1,
        "coordinate_frame_id": "slide-um",
        "orientation_source": "nuclear_major_axis",
        "weight_name": "uniform",
        "window": window,
        "cells": cells,
        "minimum_resolved_order": 0.1,
        "local": {"radius_um": 0.25, "minimum_cells": 1},
        "field": field,
        "defects": {
            "minimum_corner_order": 0.5,
            "maximum_edge_phase_step_rad": 2.8,
            "winding_tolerance": 0.1
        },
        "limits": {
            "maximum_cells": 1000,
            "maximum_grid_points": 1000,
            "maximum_neighbor_visits": 100000,
            "maximum_plaquettes": 1000
        }
    }))
    .unwrap()
}

fn cell(id: &str, x: f64, y: f64, orientation_rad: f64, weight: f64) -> Value {
    json!({
        "id": id,
        "x_um": x,
        "y_um": y,
        "orientation_rad": orientation_rad,
        "weight": weight
    })
}

fn field(origin: [f64; 2], spacing_um: f64, dimensions: [usize; 2], radius: f64) -> Value {
    json!({
        "origin_um": origin,
        "spacing_um": spacing_um,
        "dimensions": dimensions,
        "support_radius_um": radius,
        "minimum_cells": 1
    })
}

#[test]
fn aligned_and_balanced_axial_orientations_have_known_q_and_s() {
    let aligned = vec![
        cell("a", 40.0, 40.0, 0.3, 1.0),
        cell("b", 60.0, 40.0, 0.3 + std::f64::consts::PI, 3.0),
        cell("c", 40.0, 60.0, 0.3, 2.0),
        cell("d", 60.0, 60.0, 0.3, 4.0),
    ];
    let result = marklab::structure::nematic_orientation_from_json(&request(
        json!(aligned),
        field([40.0, 40.0], 20.0, [2, 2], 0.25),
        rectangle_with_optional_hole(None),
    ))
    .unwrap();
    assert_eq!(result.orientation_source, "nuclear_major_axis");
    assert_eq!(result.weight_name, "uniform");
    assert!((result.global.tensor.scalar_order.unwrap() - 1.0).abs() < 1e-12);
    assert!((result.global.tensor.q_xx.unwrap() - 0.6_f64.cos()).abs() < 1e-12);
    assert!((result.global.tensor.q_xy.unwrap() - 0.6_f64.sin()).abs() < 1e-12);
    assert!((result.global.tensor.q_yy.unwrap() + 0.6_f64.cos()).abs() < 1e-12);
    assert!((result.global.tensor.director_rad.unwrap() - 0.3).abs() < 1e-12);
    assert_eq!(result.positive_half_defects, 0);
    assert_eq!(result.negative_half_defects, 0);

    // Explicit weights apply to Q: three horizontal votes and one vertical vote give Q_xx = 1/2.
    let weighted = vec![
        cell("horizontal", 40.0, 40.0, 0.0, 3.0),
        cell("vertical", 60.0, 60.0, std::f64::consts::FRAC_PI_2, 1.0),
    ];
    let result = marklab::structure::nematic_orientation_from_json(&request(
        json!(weighted),
        field([40.0, 40.0], 20.0, [2, 2], 0.25),
        rectangle_with_optional_hole(None),
    ))
    .unwrap();
    assert!((result.global.tensor.q_xx.unwrap() - 0.5).abs() < 1e-12);
    assert!(result.global.tensor.q_xy.unwrap().abs() < 1e-12);
    assert!((result.global.tensor.scalar_order.unwrap() - 0.5).abs() < 1e-12);

    let balanced = vec![
        cell("a", 40.0, 40.0, 0.0, 1.0),
        cell("b", 60.0, 40.0, std::f64::consts::FRAC_PI_4, 1.0),
        cell("c", 40.0, 60.0, std::f64::consts::FRAC_PI_2, 1.0),
        cell("d", 60.0, 60.0, 3.0 * std::f64::consts::FRAC_PI_4, 1.0),
    ];
    let result = marklab::structure::nematic_orientation_from_json(&request(
        json!(balanced),
        field([50.0, 50.0], 1.0, [2, 2], 30.0),
        rectangle_with_optional_hole(None),
    ))
    .unwrap();
    assert!(result.global.tensor.scalar_order.unwrap().abs() < 1e-12);
    assert!(result.global.tensor.director_rad.is_none());
    assert!(result.plaquettes.iter().all(|row| row.status
        != marklab::structure::DefectStatus::PositiveHalf
        && row.status != marklab::structure::DefectStatus::NegativeHalf));
}

fn half_charge_request(sign: f64, rotation: f64) -> Vec<u8> {
    let center: [f64; 2] = [50.0, 50.0];
    let points: [(f64, f64); 4] = [(40.0, 40.0), (60.0, 40.0), (40.0, 60.0), (60.0, 60.0)];
    let cells = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| {
            let polar = (y - center[1]).atan2(x - center[0]);
            cell(
                &format!("c{index}"),
                x,
                y,
                rotation + 0.5 * sign * polar,
                1.0,
            )
        })
        .collect::<Vec<_>>();
    request(
        json!(cells),
        field([40.0, 40.0], 20.0, [2, 2], 0.25),
        rectangle_with_optional_hole(None),
    )
}

#[test]
fn plaquette_winding_resolves_positive_and_negative_half_defects_and_is_rotation_invariant() {
    for (sign, expected) in [
        (1.0, marklab::structure::DefectStatus::PositiveHalf),
        (-1.0, marklab::structure::DefectStatus::NegativeHalf),
    ] {
        for rotation in [0.0, 0.37] {
            let result = marklab::structure::nematic_orientation_from_json(&half_charge_request(
                sign, rotation,
            ))
            .unwrap();
            assert_eq!(result.plaquettes.len(), 1);
            assert_eq!(result.plaquettes[0].status, expected);
            assert!((result.plaquettes[0].topological_charge.unwrap() - sign * 0.5).abs() < 1e-12);
        }
    }
}

#[test]
fn holes_and_resource_limits_leave_defects_unresolved_or_reject_work() {
    let hole = [
        [45.0, 45.0],
        [55.0, 45.0],
        [55.0, 55.0],
        [45.0, 55.0],
        [45.0, 45.0],
    ];
    let points: [(f64, f64); 4] = [(40.0, 40.0), (60.0, 40.0), (40.0, 60.0), (60.0, 60.0)];
    let cells = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| {
            let polar = (y - 50.0).atan2(x - 50.0);
            cell(&format!("c{index}"), x, y, 0.5 * polar, 1.0)
        })
        .collect::<Vec<_>>();
    let result = marklab::structure::nematic_orientation_from_json(&request(
        json!(cells),
        field([40.0, 40.0], 20.0, [2, 2], 0.25),
        rectangle_with_optional_hole(Some(hole)),
    ))
    .unwrap();
    assert_eq!(
        result.plaquettes[0].status,
        marklab::structure::DefectStatus::WindowTruncated
    );
    assert!(result.plaquettes[0].topological_charge.is_none());

    let mut limited: Value = serde_json::from_slice(&half_charge_request(1.0, 0.0)).unwrap();
    limited["limits"]["maximum_neighbor_visits"] = json!(3);
    let bytes = serde_json::to_vec(&limited).unwrap();
    assert!(marklab::structure::nematic_orientation_from_json(&bytes).is_err());
}

#[cfg(feature = "cli")]
#[test]
fn numerics_cli_publishes_the_nematic_result() {
    let directory = tempdir().expect("temporary directory");
    let input = directory.path().join("request.json");
    let output = directory.path().join("result.json");
    fs::write(&input, half_charge_request(1.0, 0.0)).expect("request");

    let status = Command::new(env!("CARGO_BIN_EXE_marklab"))
        .args([
            "numerics",
            "nematic-orientation",
            "--input",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .status()
        .expect("run nematic orientation");
    assert!(status.success());

    let result: Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.nematic_orientation");
    assert_eq!(result["version"], 1);
    assert_eq!(result["positive_half_defects"], 1);
    assert_eq!(result["negative_half_defects"], 0);
    assert_eq!(result["plaquettes"][0]["status"], "positive_half");
    assert_eq!(result["plaquettes"][0]["topological_charge"], 0.5);
}
