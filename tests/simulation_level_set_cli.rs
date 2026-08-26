#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn run(
    input: &std::path::Path,
    output: &std::path::Path,
    final_time: &str,
    time_step: &str,
    reinitialize_every_steps: &str,
) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "evolve-interface",
            "--input",
            input.to_str().unwrap(),
            "--final-time",
            final_time,
            "--time-step",
            time_step,
            "--reinitialize-every-steps",
            reinitialize_every_steps,
            "--record-every-steps",
            "1",
            "--maximum-cell-steps",
            "10000",
            "--maximum-reinitialization-distance-visits",
            "100000",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn constant_speed_translates_a_planar_interface_exactly() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let initial = (0..4)
        .flat_map(|_| (0..6).map(|x| f64::from(x) - 2.0))
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "grid_x": 6,
            "grid_y": 4,
            "spacing_x_um": 1.0,
            "spacing_y_um": 1.0,
            "initial_phi_row_major": initial,
            "normal_speed_um_per_time_row_major": vec![1.0; 24],
            "curvature_weight_um2_per_time": 0.1
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output, "1", "0.1", "0");

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.level_set_interface");
    for row in result["final_state"].as_array().unwrap() {
        let x = row["x_um"].as_f64().unwrap();
        if x > 0.0 {
            assert!((row["phi"].as_f64().unwrap() - (x - 3.0)).abs() < 1e-12);
        }
    }
    assert_eq!(result["solver"]["completed_steps"], 10);
    assert_eq!(result["solver"]["reinitializations"], 0);
    assert!((result["solver"]["curvature_cfl_number"].as_f64().unwrap() - 0.04).abs() < 1e-12);
    assert_eq!(result["final_interface"]["horizontal_crossings"], 4);
    assert_eq!(
        result["claim_status"],
        "experimental_interface_not_tumour_forecast"
    );
}

#[test]
fn reinitialization_recovers_planar_signed_distance_without_moving_the_zero_contour() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let initial = (0..3)
        .flat_map(|_| (0..5).map(|x| 2.0 * (f64::from(x) - 2.0)))
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "grid_x": 5,
            "grid_y": 3,
            "spacing_x_um": 1.0,
            "spacing_y_um": 1.0,
            "initial_phi_row_major": initial,
            "normal_speed_um_per_time_row_major": vec![0.0; 15],
            "curvature_weight_um2_per_time": 0.0
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("result.json");
    run(&input, &output, "0.1", "0.1", "1");

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    for row in result["final_state"].as_array().unwrap() {
        let expected = row["x_um"].as_f64().unwrap() - 2.0;
        assert!((row["phi"].as_f64().unwrap() - expected).abs() < 1e-12);
    }
    assert_eq!(result["solver"]["reinitializations"], 1);
    assert_eq!(result["initial_interface"], result["final_interface"]);
    assert!(
        result["solver"]["reinitialization_distance_visits"]
            .as_u64()
            .unwrap()
            > 0
    );
}
