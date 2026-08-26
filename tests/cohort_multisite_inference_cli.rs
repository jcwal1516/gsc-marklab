#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn multisite_fixed_effect_matches_inverse_variance_and_leave_one_out_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("sites.csv");
    fs::write(
        &input,
        "site_id,effect,standard_error,patient_count\n\
site-a,1,1,20\n\
site-b,2,1,25\n\
site-c,3,1,30\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "multisite-inference",
            "--input",
            input.to_str().unwrap(),
            "--model",
            "fixed-effect",
            "--alpha",
            "0.05",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_multisite_inference");
    assert_eq!(result["model"], "fixed_effect");
    assert_eq!(result["pooled_effect"], 2.0);
    assert!(
        (result["pooled_standard_error"].as_f64().unwrap() - 1.0 / 3.0_f64.sqrt()).abs() < 1e-12
    );
    assert_eq!(result["heterogeneity"]["q"], 2.0);
    assert!(
        (result["heterogeneity"]["p_value"].as_f64().unwrap() - (-1.0_f64).exp()).abs() < 1e-12
    );
    let loo = result["leave_one_site_out"].as_array().unwrap();
    assert_eq!(loo.len(), 3);
    assert_eq!(loo[0]["omitted_site_id"], "site-a");
    assert_eq!(loo[0]["pooled_effect"], 2.5);
    assert_eq!(result["total_patient_count"], 75);
}
