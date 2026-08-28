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

#[test]
fn multisite_patient_contrast_estimates_sites_before_pooling() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    fs::write(
        &input,
        "patient_id,site_id,group,endpoint\n\
a-1,site-a,A,3\n\
a-2,site-a,A,5\n\
a-3,site-a,B,1\n\
a-4,site-a,B,1\n\
b-1,site-b,A,4\n\
b-2,site-b,A,6\n\
b-3,site-b,B,2\n\
b-4,site-b,B,2\n\
c-1,site-c,A,5\n\
c-2,site-c,A,7\n\
c-3,site-c,B,3\n\
c-4,site-c,B,3\n",
    )
    .expect("fixture");
    let output = directory.path().join("patient-result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "multisite-patient-contrast",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--model",
            "fixed-effect",
            "--alpha",
            "0.05",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.cohort_multisite_patient_contrast"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["site_effect"], "group_a_minus_group_b");
    assert_eq!(result["groups"]["group_a"], "A");
    assert_eq!(result["groups"]["group_b"], "B");
    assert_eq!(result["sites"].as_array().expect("sites").len(), 3);
    assert_eq!(result["sites"][0]["site_id"], "site-a");
    assert_eq!(result["sites"][0]["group_a_patients"], 2);
    assert_eq!(result["sites"][0]["group_b_patients"], 2);
    assert_eq!(result["sites"][0]["effect"], 3.0);
    assert_eq!(result["sites"][0]["standard_error"], 1.0);
    assert_eq!(result["pooled"]["total_patient_count"], 12);
    assert_eq!(result["pooled"]["pooled_effect"], 3.0);
}
