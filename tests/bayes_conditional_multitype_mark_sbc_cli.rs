#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn conditional_multitype_mark_sbc_uses_exact_finite_state_generation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("points.csv");
    let output = directory.path().join("sbc.json");
    let mut csv = String::from("point_id,x_um,y_um,type_id\n");
    for point_index in 0..9 {
        let type_id = ["A", "B", "C"][point_index % 3];
        writeln!(csv, "p-{point_index:02},{point_index},0,{type_id}").unwrap();
    }
    fs::write(&input, csv).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "conditional-multitype-mark-sbc",
            "--input",
            input.to_str().unwrap(),
            "--reference-type",
            "A",
            "--radius-um",
            "1.1",
            "--intercept-prior-sd",
            "0.5",
            "--interaction-prior-sd",
            "0.25",
            "--replicates",
            "20",
            "--chains",
            "2",
            "--tune",
            "750",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260829",
            "--maximum-points",
            "10",
            "--maximum-types",
            "3",
            "--maximum-neighbor-visits",
            "45",
            "--maximum-edges",
            "20",
            "--maximum-draw-parameter-work",
            "20000",
            "--maximum-working-bytes",
            "1048576",
            "--maximum-tree-depth",
            "12",
            "--maximum-states",
            "20000",
            "--maximum-enumeration-work",
            "400000",
            "--maximum-enumeration-bytes",
            "8388608",
            "--maximum-total-iterations",
            "70000",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.65",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "600",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.conditional_multitype_mark_sbc");
    assert_eq!(result["enumeration_oracle"]["state_count"], 19_683);
    assert!(
        (result["enumeration_oracle"]["zero_parameter_log_normalizer"]
            .as_f64()
            .unwrap()
            - 9.0 * 3.0_f64.ln())
        .abs()
            < 1e-12
    );
    assert_eq!(
        result["replicates"].as_array().unwrap().len()
            + result["failures"].as_array().unwrap().len(),
        20
    );
    assert_eq!(result["diagnostics"].as_array().unwrap().len(), 10);
    assert_eq!(result["statistical_unit"], "one_fixed_location_pattern");
}
