#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "reaction-diffusion",
            "--input",
            input.to_str().unwrap(),
            "--final-time",
            "1",
            "--time-step",
            "0.1",
            "--record-every-steps",
            "2",
            "--maximum-cell-steps",
            "1000",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn linear_reaction_only_matches_exact_exponential_growth() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "grid_x": 2,
            "grid_y": 2,
            "spacing_x_um": 1.0,
            "spacing_y_um": 1.0,
            "initial_row_major": [0.25, 0.5, 0.75, 1.0],
            "diffusion_um2_per_time": 0.0,
            "reaction": {"model": "linear", "rate_per_time": std::f64::consts::LN_2}
        }))
        .unwrap(),
    )
    .expect("input");
    let output = directory.path().join("result.json");
    run(&input, &output);

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    assert_eq!(result["format"], "marklab.reaction_diffusion");
    let expected = [0.5, 1.0, 1.5, 2.0];
    for (row, expected) in result["final_state"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected)
    {
        assert!((row["value"].as_f64().unwrap() - expected).abs() < 1e-12);
    }
    assert_eq!(result["solver"]["positivity_violations"], 0);
    assert_eq!(result["solver"]["completed_steps"], 10);
    assert_eq!(result["solver"]["cell_steps"], 40);
    assert_eq!(
        result["pattern"]["linear_instability_status"],
        "reference_not_homogeneous_equilibrium"
    );
    assert!(result["pattern"]["unstable_wavelength_min_um"].is_null());
    assert!(result["pattern"]["unstable_wavelength_max_um"].is_null());
    assert_eq!(
        result["claim_status"],
        "experimental_reaction_diffusion_not_biological_mechanism_proof"
    );
}

#[test]
fn fourier_diagnostic_recovers_a_known_two_micrometre_mode() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let initial = (0..4)
        .flat_map(|_| [1.1, 0.9, 1.1, 0.9])
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "grid_x": 4,
            "grid_y": 4,
            "spacing_x_um": 1.0,
            "spacing_y_um": 1.0,
            "initial_row_major": initial,
            "diffusion_um2_per_time": 0.0,
            "reaction": {"model": "linear", "rate_per_time": 0.0}
        }))
        .unwrap(),
    )
    .expect("input");
    let output = directory.path().join("result.json");
    run(&input, &output);

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    assert!(
        (result["pattern"]["dominant_wavelength_um"]
            .as_f64()
            .unwrap()
            - 2.0)
            .abs()
            < 1e-12
    );
    assert!(
        (result["pattern"]["dominant_power_fraction"]
            .as_f64()
            .unwrap()
            - 1.0)
            .abs()
            < 1e-12
    );
    assert_eq!(
        result["pattern"]["spectrum_basis"],
        "periodic_2d_discrete_fourier"
    );
    assert_eq!(
        result["pattern"]["linear_instability_status"],
        "nonzero_modes_stable"
    );
}

#[test]
fn periodic_diffusion_matches_the_checkerboard_eigenmode_step() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let initial = (0..4)
        .flat_map(|_| [1.1, 0.9, 1.1, 0.9])
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "grid_x": 4,
            "grid_y": 4,
            "spacing_x_um": 1.0,
            "spacing_y_um": 1.0,
            "initial_row_major": initial,
            "diffusion_um2_per_time": 0.1,
            "reaction": {"model": "linear", "rate_per_time": 0.0}
        }))
        .unwrap(),
    )
    .expect("input");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "reaction-diffusion",
            "--input",
            input.to_str().unwrap(),
            "--final-time",
            "0.1",
            "--time-step",
            "0.1",
            "--record-every-steps",
            "1",
            "--maximum-cell-steps",
            "100",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    for (index, row) in result["final_state"].as_array().unwrap().iter().enumerate() {
        let expected = if (index % 4).is_multiple_of(2) {
            1.096
        } else {
            0.904
        };
        assert!((row["value"].as_f64().unwrap() - expected).abs() < 1e-12);
    }
    assert!((result["solver"]["maximum_cfl_number"].as_f64().unwrap() - 0.02).abs() < 1e-12);
}
