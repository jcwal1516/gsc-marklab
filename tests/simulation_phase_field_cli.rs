#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

fn run(
    input: &std::path::Path,
    output: &std::path::Path,
    final_time: &str,
    time_step: &str,
    maximum_cell_steps: &str,
) -> assert_cmd::assert::Assert {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "phase-field",
            "--input",
            input.to_str().unwrap(),
            "--final-time",
            final_time,
            "--time-step",
            time_step,
            "--record-every-steps",
            "1",
            "--maximum-cell-steps",
            maximum_cell_steps,
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
}

fn write_input(path: &std::path::Path, initial: Vec<f64>) {
    write_input_with_geometry(path, initial, 1.0, 1.0, 1.0);
}

fn write_input_with_geometry(
    path: &std::path::Path,
    initial: Vec<f64>,
    spacing_x_um: f64,
    spacing_y_um: f64,
    bulk_energy_scale: f64,
) {
    fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "grid_x": 4,
            "grid_y": 4,
            "spacing_x_um": spacing_x_um,
            "spacing_y_um": spacing_y_um,
            "initial_phase_row_major": initial,
            "mobility_um2_per_time": 1.0,
            "bulk_energy_scale": bulk_energy_scale,
            "gradient_energy_coefficient_um2": 0.5
        }))
        .unwrap(),
    )
    .expect("input");
}

#[test]
fn constant_phase_is_stationary_and_conserved() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    write_input(&input, vec![0.25; 16]);
    let output = directory.path().join("result.json");
    run(&input, &output, "0.2", "0.1", "100").success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    assert_eq!(result["format"], "marklab.cahn_hilliard_phase_field");
    assert_eq!(result["solver"]["boundary_condition"], "periodic");
    assert_eq!(result["solver"]["completed_steps"], 2);
    assert_eq!(result["solver"]["cell_steps"], 32);
    assert_eq!(result["conservation"]["mean_conserved"], true);
    assert_eq!(result["energy"]["nonincreasing"], true);
    for row in result["final_state"].as_array().expect("state") {
        assert!((row["phase"].as_f64().unwrap() - 0.25).abs() < 1e-12);
    }
}

#[test]
fn nontrivial_field_conserves_mass_and_dissipates_free_energy() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    write_input(
        &input,
        (0..16)
            .map(|index| if index % 2 == 0 { 0.2 } else { -0.2 })
            .collect(),
    );
    let output = directory.path().join("result.json");
    run(&input, &output, "0.1", "0.01", "1000").success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    let initial_mass = result["conservation"]["initial_mass"].as_f64().unwrap();
    let final_mass = result["conservation"]["final_mass"].as_f64().unwrap();
    assert!((final_mass - initial_mass).abs() < 1e-12);
    assert!(
        result["energy"]["final_free_energy"].as_f64().unwrap()
            < result["energy"]["initial_free_energy"].as_f64().unwrap()
    );
    assert_eq!(result["energy"]["nonincreasing"], true);
    assert_eq!(result["solver"]["energy_rejections"], 0);
}

#[test]
fn small_checkerboard_mode_matches_the_stabilized_spectral_step() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let amplitude = 1e-6;
    write_input(
        &input,
        (0..16)
            .map(|index| {
                if (index % 4) % 2 == 0 {
                    amplitude
                } else {
                    -amplitude
                }
            })
            .collect(),
    );
    let output = directory.path().join("result.json");
    run(&input, &output, "0.01", "0.01", "100").success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    let stabilization = 1.0 + 3.0 * amplitude * amplitude;
    let q = 4.0;
    let expected_factor =
        (1.0 + 0.01 * q * stabilization + 0.01 * q) / (1.0 + 0.01 * q * (stabilization + 0.5 * q));
    for (index, row) in result["final_state"].as_array().unwrap().iter().enumerate() {
        let expected = if (index % 4) % 2 == 0 {
            amplitude * expected_factor
        } else {
            -amplitude * expected_factor
        };
        assert!((row["phase"].as_f64().unwrap() - expected).abs() < 1e-16);
    }
}

#[test]
fn declared_work_limit_rejects_the_simulation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    write_input(&input, vec![0.0; 16]);
    let output = directory.path().join("result.json");
    run(&input, &output, "1", "0.1", "100")
        .failure()
        .stderr(predicate::str::contains("cell-steps exceed"));
}

#[test]
fn nonfinite_derived_geometry_is_rejected_before_state_publication() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("overflow-input.json");
    // Every supplied scalar and the initial mass/free energy are finite, but ix * spacing_x
    // overflows for ix >= 2 and the inverse-square spatial operator underflows to zero.
    write_input_with_geometry(&input, vec![0.0; 16], 1e308, 1.0, 1e-308);
    let output = directory.path().join("overflow-result.json");
    run(&input, &output, "0.1", "0.1", "100")
        .failure()
        .stderr(predicate::str::contains("derived geometry or operator"));
    assert!(!output.exists());

    let input = directory.path().join("underflow-input.json");
    write_input_with_geometry(&input, vec![0.0; 16], 1e-308, 1e-308, 1.0);
    let output = directory.path().join("underflow-result.json");
    run(&input, &output, "0.1", "0.1", "100")
        .failure()
        .stderr(predicate::str::contains("derived geometry or operator"));
    assert!(!output.exists());
}

#[test]
fn an_existing_output_is_rejected_and_preserved() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    write_input(&input, vec![0.25; 16]);
    let output = directory.path().join("result.json");
    let sentinel = b"existing phase-field result\n";
    fs::write(&output, sentinel).expect("sentinel");

    run(&input, &output, "0.2", "0.1", "100").failure();
    assert_eq!(fs::read(output).expect("preserved sentinel"), sentinel);
}
