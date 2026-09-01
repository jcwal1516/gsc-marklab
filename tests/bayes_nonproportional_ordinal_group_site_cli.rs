#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn nonproportional_ordinal_recovers_threshold_specific_group_shifts() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_stage_sites.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("patient_id,site_id,group,outcome\n");
    let reference = ["I", "I", "I", "I", "II", "III", "IV", "IV"];
    let comparison = ["I", "II", "II", "II", "II", "II", "III", "IV"];
    for site in 0..8 {
        for (patient, outcome) in reference.iter().enumerate() {
            writeln!(csv, "s{site}-mss-{patient},s{site},MSS,{outcome}").expect("reference");
        }
        for (patient, outcome) in comparison.iter().enumerate() {
            writeln!(csv, "s{site}-msi-{patient},s{site},MSI,{outcome}").expect("comparison");
        }
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "nonproportional-ordinal-group-site",
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
            "--site-intercept-sd-prior-sd",
            "1",
            "--chains",
            "4",
            "--tune",
            "800",
            "--draws",
            "800",
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
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_nonproportional_ordinal_group_site"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    let effects = result["posterior"]["threshold_group_log_odds_effects"]
        .as_array()
        .unwrap();
    assert_eq!(effects.len(), 3);
    assert!(effects[0]["summary"]["mean"].as_f64().unwrap() > 0.5);
    assert!(effects[1]["summary"]["mean"].as_f64().unwrap() < -0.5);
    assert_eq!(result["input"]["patient_count"], 128);
    assert_eq!(result["input"]["site_count"], 8);
    assert_eq!(result["diagnostics"]["divergences"], 0);
}
