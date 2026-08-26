#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn uniform_logistic_growth_matches_the_closed_form() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("initial.csv");
    fs::write(&input, "position_um,density\n0,0.25\n1,0.25\n2,0.25\n").expect("input");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "growth-front",
            "--input",
            input.to_str().expect("input path"),
            "--diffusion-um2-per-time",
            "0",
            "--growth-rate-per-time",
            "1",
            "--carrying-capacity",
            "1",
            "--final-time",
            "1.0986122886681098",
            "--time-step",
            "0.1",
            "--front-threshold-fraction",
            "0.5",
            "--record-every-steps",
            "2",
            "--maximum-cell-steps",
            "1000",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    assert_eq!(result["format"], "marklab.fisher_kpp_growth_front");
    for row in result["final_state"].as_array().expect("final state") {
        assert!((row["density"].as_f64().expect("density") - 0.5).abs() < 1e-12);
    }
    assert_eq!(result["front_status"], "unavailable_no_threshold_crossing");
    assert_eq!(result["solver"]["positivity_violations"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_mechanistic_simulation_not_tumor_forecast"
    );
}

#[test]
fn fisher_kpp_step_profile_advances_a_bounded_front() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("initial.csv");
    let mut csv = String::from("position_um,density\n");
    for index in 0..=100 {
        let position = index as f64 * 0.5;
        let density = if position <= 10.0 { 1.0 } else { 0.0 };
        csv.push_str(&format!("{position},{density}\n"));
    }
    fs::write(&input, csv).expect("input");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "growth-front",
            "--input",
            input.to_str().expect("input path"),
            "--diffusion-um2-per-time",
            "0.25",
            "--growth-rate-per-time",
            "1",
            "--carrying-capacity",
            "1",
            "--final-time",
            "10",
            "--time-step",
            "0.1",
            "--front-threshold-fraction",
            "0.5",
            "--record-every-steps",
            "10",
            "--maximum-cell-steps",
            "20000",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    let initial_front = result["initial_front_position_um"].as_f64().unwrap();
    let final_front = result["final_front_position_um"].as_f64().unwrap();
    assert!(final_front > initial_front + 3.0);
    assert_eq!(result["theoretical_planar_speed_um_per_time"], 1.0);
    assert!(
        result["final_total_density_mass"].as_f64().unwrap()
            > result["initial_total_density_mass"].as_f64().unwrap()
    );
    assert!(result["solver"]["maximum_cfl_number"].as_f64().unwrap() <= 0.5);
    assert!(result["solver"]["cell_steps"].as_u64().unwrap() <= 20_000);
    assert_eq!(result["front_status"], "available_single_threshold_front");
}

#[test]
fn growth_front_rejects_a_time_step_above_the_diffusion_cfl_limit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("initial.csv");
    fs::write(&input, "position_um,density\n0,1\n1,0.5\n2,0\n").expect("input");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "growth-front",
            "--input",
            input.to_str().expect("input path"),
            "--diffusion-um2-per-time",
            "1",
            "--growth-rate-per-time",
            "1",
            "--carrying-capacity",
            "1",
            "--final-time",
            "1",
            "--time-step",
            "1",
            "--front-threshold-fraction",
            "0.5",
            "--record-every-steps",
            "1",
            "--maximum-cell-steps",
            "100",
            "--out",
            directory.path().join("result.json").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("CFL 1 exceeds 0.5"));
}
