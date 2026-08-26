#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn rejection_abc_sbc_has_near_uniform_ranks_and_replays() {
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
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.growth_front_rejection_abc_sbc");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 100);
    assert_eq!(result["failed_replicates"], 0);
    let mean_rank = result["rank_diagnostics"]["mean_normalized_rank"]
        .as_f64()
        .unwrap();
    assert!((mean_rank - 0.5).abs() < 0.1, "{mean_rank}");
    let coverage = result["coverage"]["empirical"].as_f64().unwrap();
    assert!((0.8..=1.0).contains(&coverage), "{coverage}");
    assert_eq!(
        result["random_seed_namespace"],
        "growth_front_rejection_abc_sbc_v1_chacha20"
    );
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "growth-front-rejection-abc-sbc",
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
            "--mass-scale",
            "0.02",
            "--growth-rate-prior-min",
            "0.5",
            "--growth-rate-prior-max",
            "1.5",
            "--epsilon",
            "1",
            "--posterior-draws",
            "30",
            "--maximum-proposals-per-replicate",
            "1000",
            "--replicates",
            "100",
            "--coverage-probability",
            "0.9",
            "--maximum-cell-steps-per-simulation",
            "1000",
            "--seed",
            "20260825",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
