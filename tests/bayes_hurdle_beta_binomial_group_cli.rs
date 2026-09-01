#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn hurdle_beta_binomial_separates_presence_and_positive_abundance_shifts() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_counts.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("patient_id,group,successes,trials\n");
    for index in 0..12 {
        let successes = if index < 8 { 0 } else { 2 + index % 3 };
        writeln!(csv, "mss-{index},MSS,{successes},100").expect("reference row");
    }
    for index in 0..12 {
        let successes = if index < 2 { 0 } else { 25 + index % 6 };
        writeln!(csv, "msi-{index},MSI,{successes},100").expect("comparison row");
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "hurdle-beta-binomial-group",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--presence-intercept-prior-mean",
            "0",
            "--presence-intercept-prior-sd",
            "2",
            "--presence-group-effect-prior-sd",
            "2",
            "--abundance-intercept-prior-mean",
            "-2",
            "--abundance-intercept-prior-sd",
            "2",
            "--abundance-group-effect-prior-sd",
            "2",
            "--concentration-prior-sd",
            "50",
            "--chains",
            "4",
            "--tune",
            "600",
            "--draws",
            "600",
            "--target-accept",
            "0.95",
            "--seed",
            "20260831",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_hurdle_beta_binomial_group"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["model"]["biological_unit"], "patient");
    assert_eq!(result["input"]["patient_count"], 24);
    assert_eq!(result["input"]["zero_count"], 10);
    assert!(
        result["posterior"]["presence_group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 1.0
    );
    assert!(
        result["posterior"]["positive_abundance_group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 1.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_patient_hurdle_group_association"
    );
}
