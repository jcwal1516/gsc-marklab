#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn ordinal_group_recovers_patient_level_ordered_shift() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_stage.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("patient_id,group,outcome\n");
    for index in 0..12 {
        writeln!(
            csv,
            "mss-{index},MSS,{}",
            if index < 9 { "I" } else { "II" }
        )
        .expect("reference row");
    }
    for index in 0..12 {
        writeln!(
            csv,
            "msi-{index},MSI,{}",
            if index < 3 { "III" } else { "IV" }
        )
        .expect("comparison row");
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "ordinal-group",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--ordered-levels",
            "I,II,III,IV",
            "--cutpoint-prior-sd",
            "2",
            "--group-effect-prior-sd",
            "2",
            "--chains",
            "4",
            "--tune",
            "400",
            "--draws",
            "400",
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
    assert_eq!(result["format"], "marklab.bayesian_ordinal_group");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["model"]["biological_unit"], "patient");
    assert_eq!(result["input"]["patient_count"], 24);
    assert_eq!(result["input"]["level_count"], 4);
    assert!(
        result["posterior"]["group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 2.0
    );
    assert!(
        result["posterior"]["comparison_expected_code"]["mean"]
            .as_f64()
            .unwrap()
            > result["posterior"]["reference_expected_code"]["mean"]
                .as_f64()
                .unwrap()
                + 1.0
    );
    assert_eq!(
        result["posterior_predictive"]["levels"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_patient_ordinal_group_association"
    );
}
