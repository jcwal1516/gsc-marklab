#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn base_args<'a>(input: &'a str, output: &'a str) -> Vec<&'a str> {
    vec![
        "simulate",
        "spatial-competition",
        "--input",
        input,
        "--diffusion-a-um2-per-time",
        "0",
        "--diffusion-b-um2-per-time",
        "0",
        "--growth-a-per-time",
        "0",
        "--growth-b-per-time",
        "0",
        "--carrying-a",
        "1",
        "--carrying-b",
        "1",
        "--competition-a-from-b",
        "0",
        "--competition-b-from-a",
        "0",
        "--treatment-a-per-time",
        "0.6931471805599453",
        "--treatment-b-per-time",
        "0",
        "--final-time",
        "1",
        "--time-step",
        "0.1",
        "--extinction-threshold-fraction",
        "0.1",
        "--record-every-steps",
        "2",
        "--maximum-cell-species-steps",
        "1000",
        "--out",
        output,
    ]
}

#[test]
fn constant_treatment_matches_the_exact_exponential_decay() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("initial.csv");
    fs::write(
        &input,
        "position_um,density_a,density_b\n0,1,0.25\n1,1,0.25\n2,1,0.25\n",
    )
    .expect("input");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args(base_args(input.to_str().unwrap(), output.to_str().unwrap()))
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    assert_eq!(result["format"], "marklab.spatial_competition");
    for row in result["final_state"].as_array().expect("state") {
        assert!((row["density_a"].as_f64().unwrap() - 0.5).abs() < 1e-12);
        assert!((row["density_b"].as_f64().unwrap() - 0.25).abs() < 1e-12);
    }
    assert_eq!(result["outcome"], "coexistence");
    assert_eq!(result["solver"]["positivity_violations"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_ecological_simulation_not_causal_treatment_effect"
    );
}

#[test]
fn asymmetric_competition_records_species_a_exclusion() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("initial.csv");
    fs::write(
        &input,
        "position_um,density_a,density_b\n0,0.4,0.4\n1,0.4,0.4\n2,0.4,0.4\n",
    )
    .expect("input");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "spatial-competition",
            "--input",
            input.to_str().unwrap(),
            "--diffusion-a-um2-per-time",
            "0",
            "--diffusion-b-um2-per-time",
            "0",
            "--growth-a-per-time",
            "1",
            "--growth-b-per-time",
            "1",
            "--carrying-a",
            "1",
            "--carrying-b",
            "1",
            "--competition-a-from-b",
            "2",
            "--competition-b-from-a",
            "0.25",
            "--treatment-a-per-time",
            "0",
            "--treatment-b-per-time",
            "0",
            "--final-time",
            "10",
            "--time-step",
            "0.01",
            "--extinction-threshold-fraction",
            "0.1",
            "--record-every-steps",
            "100",
            "--maximum-cell-species-steps",
            "10000",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("output")).expect("JSON");
    assert_eq!(result["outcome"], "species_a_excluded");
    assert!(result["final_maximum_density_a"].as_f64().unwrap() < 0.1);
    assert!(result["final_maximum_density_b"].as_f64().unwrap() > 0.8);
    assert!(result["extinction_events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["species"] == "a"));
}
