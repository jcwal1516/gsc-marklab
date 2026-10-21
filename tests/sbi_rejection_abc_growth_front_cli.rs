#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn rejection_abc_cli_publishes_byte_identical_replays_and_claims() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("initial.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "initial": [
                {"position_um": 0.0, "density": 0.25},
                {"position_um": 1.0, "density": 0.25},
                {"position_um": 2.0, "density": 0.25}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    run(&input, &first);
    run(&input, &second);
    // The library replays in memory; this pins the published bytes across processes.
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.rejection_abc_growth_front");
    assert_eq!(result["accepted"].as_array().unwrap().len(), 30);
    assert_eq!(
        result["random_seed_namespace"],
        "rejection_abc_growth_front_v1_chacha20"
    );
    assert_eq!(
        result["claim_status"],
        "synthetic_abc_specialization_not_biological_calibration"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "rejection-abc-growth-front",
            "--input",
            input.to_str().unwrap(),
            "--diffusion-um2-per-time",
            "0",
            "--carrying-capacity",
            "1",
            "--final-time",
            "1.0986122886681098",
            "--time-step",
            "0.1",
            "--front-threshold-fraction",
            "0.5",
            "--observed-final-mass",
            "1",
            "--mass-scale",
            "0.02",
            "--growth-rate-prior-min",
            "0.5",
            "--growth-rate-prior-max",
            "1.5",
            "--epsilon",
            "1",
            "--accepted-draws",
            "30",
            "--maximum-proposals",
            "10000",
            "--maximum-cell-steps-per-proposal",
            "1000",
            "--seed",
            "20260825",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
