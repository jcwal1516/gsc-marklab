#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn multimodal_m0_m5_comparison_reuses_nested_patient_heldout_fits() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
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
    fs::write(&input, csv).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "compare-models",
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
            "53",
            "--timeout-seconds",
            "120",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.multimodal_model_comparison");
    assert_eq!(
        result["split_policy"],
        "nested_patient_held_out_preprocessing_and_tuning"
    );
    assert_eq!(result["models"].as_array().unwrap().len(), 6);
    assert_eq!(result["comparisons"].as_array().unwrap().len(), 6);
    let m0 = result["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["model_id"] == "m0")
        .unwrap();
    let m2 = result["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["model_id"] == "m2")
        .unwrap();
    assert!(m2["rmse"].as_f64().unwrap() < m0["rmse"].as_f64().unwrap() * 0.25);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_multimodal_comparison"
    );
}
