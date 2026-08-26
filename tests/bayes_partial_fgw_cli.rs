#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn partial_fgw_preserves_declared_mass_and_reports_all_sensitivity_axes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        br#"{"source":[{"id":"s1","mass":1.0,"features":[0.0]}],"target":[{"id":"t1","mass":1.0,"features":[0.0]}],"source_structure_row_major":[0.0],"target_structure_row_major":[0.0]}"#,
    )
    .expect("input");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "partial-fused-gromov-wasserstein",
            "--input",
            input.to_str().expect("input path"),
            "--transported-mass",
            "0.5",
            "--alpha",
            "0.5",
            "--epsilon",
            "0.1",
            "--feature-scale",
            "1",
            "--structure-scale",
            "1",
            "--tolerance",
            "1e-8",
            "--maximum-iterations",
            "100",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.partial_fused_gromov_wasserstein");
    assert!((result["transported_mass"].as_f64().unwrap() - 0.5).abs() < 1e-8);
    assert!((result["unmatched_source_mass"][0].as_f64().unwrap() - 0.5).abs() < 1e-8);
    assert!((result["unmatched_target_mass"][0].as_f64().unwrap() - 0.5).abs() < 1e-8);
    assert_eq!(
        result["initialization_sensitivity"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(result["parameter_sensitivity"].as_array().unwrap().len(), 7);
    assert_eq!(
        result["claim_status"],
        "ensemble_descriptive_alignment_not_correspondence"
    );
}

#[test]
fn partial_fgw_feature_linearization_recovers_reversed_partial_match() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "source": [
                {"id": "s1", "mass": 0.5, "features": [0.0]},
                {"id": "s2", "mass": 0.5, "features": [1.0]}
            ],
            "target": [
                {"id": "t1", "mass": 0.5, "features": [1.0]},
                {"id": "t2", "mass": 0.5, "features": [0.0]}
            ],
            "source_structure_row_major": [0.0, 1.0, 1.0, 0.0],
            "target_structure_row_major": [0.0, 1.0, 1.0, 0.0]
        }))
        .expect("input JSON"),
    )
    .expect("input");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "partial-fused-gromov-wasserstein",
            "--input",
            input.to_str().expect("input path"),
            "--transported-mass",
            "0.5",
            "--alpha",
            "0.5",
            "--epsilon",
            "0.02",
            "--feature-scale",
            "1",
            "--structure-scale",
            "1",
            "--tolerance",
            "1e-9",
            "--maximum-iterations",
            "100",
            "--timeout-seconds",
            "60",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    let cross_mass = result["initialization_sensitivity"][0]["plan"]
        .as_array()
        .expect("plan")
        .iter()
        .filter(|row| {
            (row["source_id"] == "s1" && row["target_id"] == "t2")
                || (row["source_id"] == "s2" && row["target_id"] == "t1")
        })
        .map(|row| row["mass"].as_f64().expect("mass"))
        .sum::<f64>();
    assert!(cross_mass > 0.45, "cross mass was {cross_mass}");
}
