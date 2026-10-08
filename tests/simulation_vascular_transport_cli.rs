#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn run(input: &std::path::Path, output: &std::path::Path, final_time: &str, time_step: &str) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "vascular-transport",
            "--input",
            input.to_str().unwrap(),
            "--final-time",
            final_time,
            "--time-step",
            time_step,
            "--hypoxia-threshold",
            "0.005",
            "--record-every-steps",
            "1",
            "--maximum-cell-steps",
            "10000",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

fn base_input(initial: Vec<f64>) -> serde_json::Value {
    serde_json::json!({
        "grid_x": 3,
        "grid_y": 3,
        "spacing_x_um": 1.0,
        "spacing_y_um": 1.0,
        "initial_concentration_row_major": initial,
        "diffusion_um2_per_time_row_major": vec![0.0; 9],
        "velocity_x_um_per_time_row_major": vec![0.0; 9],
        "velocity_y_um_per_time_row_major": vec![0.0; 9],
        "flow_approximation": "caller_declared_static_velocity",
        "vessel_sources": [],
        "uptake_cells": []
    })
}

#[test]
fn mapped_vessel_source_and_cell_uptake_match_the_exact_local_flow() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let mut spec = base_input(vec![0.0; 9]);
    spec["vessel_sources"] = serde_json::json!([{
        "vessel_id": "v1", "x_um": 1.0, "y_um": 1.0,
        "source_concentration_per_time": 2.0
    }]);
    spec["uptake_cells"] = serde_json::json!([{
        "cell_id": "c1", "x_um": 1.0, "y_um": 1.0,
        "linear_uptake_per_time": 1.0
    }]);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output, "0.6931471805599453", "0.6931471805599453");

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.vascular_transport");
    assert!((result["final_state"][4]["concentration"].as_f64().unwrap() - 1.0).abs() < 1e-12);
    assert_eq!(result["mapped_vessel_sources"][0]["grid_index"], 4);
    assert_eq!(result["mapped_uptake_cells"][0]["grid_index"], 4);
    assert!(
        result["solver"]["mass_balance_residual"]
            .as_f64()
            .unwrap()
            .abs()
            < 1e-12
    );
    assert_eq!(
        result["claim_status"],
        "advanced_transport_not_causal_or_hemodynamic_truth"
    );
}

#[test]
fn no_flux_diffusion_conserves_a_centered_impulse() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let mut initial = vec![0.0; 9];
    initial[4] = 1.0;
    let mut spec = base_input(initial);
    spec["diffusion_um2_per_time_row_major"] = serde_json::json!(vec![0.1; 9]);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output, "0.1", "0.1");

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    let values = result["final_state"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["concentration"].as_f64().unwrap())
        .collect::<Vec<_>>();
    assert!((values[4] - 0.96).abs() < 1e-12);
    for index in [1, 3, 5, 7] {
        assert!((values[index] - 0.01).abs() < 1e-12);
    }
    assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert!(
        result["solver"]["maximum_transport_mass_residual"]
            .as_f64()
            .unwrap()
            < 1e-12
    );
    assert_eq!(result["hypoxic_regions"].as_array().unwrap().len(), 4);
    assert!(result["hypoxic_regions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|region| region["cell_count"] == 1));
}

#[test]
fn upwind_advection_moves_mass_right_conservatively() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let initial = (0..3).flat_map(|_| [1.0, 0.0, 0.0]).collect::<Vec<_>>();
    let mut spec = base_input(initial);
    spec["velocity_x_um_per_time_row_major"] = serde_json::json!(vec![1.0; 9]);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output, "0.1", "0.1");

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    for row in 0..3 {
        let offset = row * 3;
        assert!(
            (result["final_state"][offset]["concentration"]
                .as_f64()
                .unwrap()
                - 0.9)
                .abs()
                < 1e-12
        );
        assert!(
            (result["final_state"][offset + 1]["concentration"]
                .as_f64()
                .unwrap()
                - 0.1)
                .abs()
                < 1e-12
        );
        assert!(
            result["final_state"][offset + 2]["concentration"]
                .as_f64()
                .unwrap()
                .abs()
                < 1e-12
        );
    }
    assert!((result["solver"]["advective_cfl_number"].as_f64().unwrap() - 0.1).abs() < 1e-12);
}
