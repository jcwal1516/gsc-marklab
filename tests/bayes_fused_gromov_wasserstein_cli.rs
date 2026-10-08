#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn fgw_recovers_feature_reversed_isometry_and_reports_initialization_sensitivity() {
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
            "fused-gromov-wasserstein",
            "--input",
            input.to_str().expect("input path"),
            "--alpha",
            "0.5",
            "--epsilon",
            "0.05",
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
    assert_eq!(result["format"], "marklab.fused_gromov_wasserstein");
    assert_eq!(result["backend"]["name"], "pot");
    assert_eq!(result["backend"]["version"], "0.9.7.post1");
    assert_eq!(
        result["initialization_sensitivity"]
            .as_array()
            .expect("sensitivity")
            .len(),
        3
    );
    let plan = result["best_plan"].as_array().expect("plan");
    let cross_mass = plan
        .iter()
        .filter(|row| {
            (row["source_id"] == "s1" && row["target_id"] == "t2")
                || (row["source_id"] == "s2" && row["target_id"] == "t1")
        })
        .map(|row| row["mass"].as_f64().expect("mass"))
        .sum::<f64>();
    assert!(cross_mass > 0.9, "cross mass was {cross_mass}");
    assert_eq!(
        result["claim_status"],
        "descriptive_alignment_not_correspondence"
    );
}
