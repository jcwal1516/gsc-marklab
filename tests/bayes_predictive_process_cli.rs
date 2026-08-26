#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn predictive_process_has_zero_knot_residual_and_exact_corrected_diagonal() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("coordinates.csv");
    fs::write(
        &input,
        "coordinate_id,x_um\n\
c-0,0\n\
c-1,1\n\
c-2,2\n",
    )
    .expect("coordinates");
    let knots = directory.path().join("knots.csv");
    fs::write(&knots, "knot_id,x_um\nk-0,0\nk-1,2\n").expect("knots");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "predictive-process",
            "--input",
            input.to_str().expect("input path"),
            "--knots",
            knots.to_str().expect("knot path"),
            "--amplitude",
            "1",
            "--length-scale-um",
            "1",
            "--jitter",
            "0.000000000001",
            "--diagonal-correction",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.predictive_process");
    assert_eq!(result["version"], 1);
    assert_eq!(result["kernel"], "matern_3_2");
    assert_eq!(result["coordinate_unit"], "micrometre");
    assert_eq!(result["diagonal_correction"], true);
    let residuals = result["residuals"].as_array().expect("residuals");
    assert_eq!(residuals.len(), 3);
    assert!(residuals[0]["residual_variance"].as_f64().unwrap() <= 1e-9);
    assert!(residuals[1]["residual_variance"].as_f64().unwrap() > 0.0);
    assert!(residuals[2]["residual_variance"].as_f64().unwrap() <= 1e-9);
    assert!(residuals
        .iter()
        .all(|row| { (row["corrected_variance"].as_f64().unwrap() - 1.0).abs() <= 1e-10 }));
    assert!(
        result["summary"]["trace_fraction_retained"]
            .as_f64()
            .unwrap()
            < 1.0
    );
    assert_eq!(
        result["claim_status"],
        "experimental_approximation_diagnostic"
    );
}
