#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::{fmt::Write as _, fs};

#[test]
fn complementarity_is_nested_patient_held_out_and_detects_patch_increment() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("output.json");
    let mut csv = String::from(
        "patient_id,outer_fold,inner_fold,target,technical_0,clinical_0,compartment_0,acquisition_0,cell_0,patch_0,neighbor_0,measured_0\n",
    );
    for index in 0..24 {
        let target = index as f64 * 0.75 - 4.0;
        writeln!(
            csv,
            "p{index},{},{},{target},{},{},{},{},{},{target},{},{}",
            index % 4,
            (index / 4) % 3,
            index % 2,
            index % 3,
            index % 4,
            index % 5,
            (index as f64 * 0.37).sin(),
            (index as f64 * 0.23).cos(),
            (index as f64 * 0.19).sin(),
        )
        .unwrap();
    }
    fs::write(&input, csv).unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "test-cell-patch-complementarity",
            "--input",
            input.to_str().unwrap(),
            "--outer-folds",
            "4",
            "--inner-folds",
            "3",
            "--ridge-alphas",
            "0,0.01,0.1,1,10",
            "--permutations",
            "99",
            "--seed",
            "1409",
            "--timeout-seconds",
            "120",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.cell_patch_complementarity");
    assert_eq!(result["version"], 1);
    assert_eq!(result["backend"]["name"], "scipy");
    assert_eq!(result["backend"]["version"], "1.18.1");
    assert_eq!(result["patient_count"], 24);
    assert_eq!(result["outer_folds"], 4);
    assert_eq!(result["inner_folds"], 3);
    assert_eq!(result["models"].as_array().unwrap().len(), 6);
    assert_eq!(result["comparisons"].as_array().unwrap().len(), 6);
    assert_eq!(
        result["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| model["model_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "baseline",
            "baseline_plus_cell",
            "baseline_plus_patch",
            "baseline_plus_cell_patch",
            "baseline_plus_cell_patch_neighbor",
            "baseline_plus_cell_patch_neighbor_measured",
        ]
    );
    let baseline = result["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["model_id"] == "baseline")
        .unwrap();
    let patch = result["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["model_id"] == "baseline_plus_patch")
        .unwrap();
    assert!(patch["rmse"].as_f64().unwrap() < 0.1);
    assert!(patch["rmse"].as_f64().unwrap() < baseline["rmse"].as_f64().unwrap() * 0.25);
    assert_eq!(patch["predictions"].as_array().unwrap().len(), 24);
    let increment = result["comparisons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|comparison| comparison["comparison_id"] == "baseline_plus_patch_minus_baseline")
        .unwrap();
    assert!(
        increment["mean_absolute_error_effect_expanded_minus_base"]
            .as_f64()
            .unwrap()
            < 0.0
    );
    assert_eq!(increment["pair_count"], 24);
    assert_eq!(increment["permutations"], 99);
}
