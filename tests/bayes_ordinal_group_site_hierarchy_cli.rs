#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn ordinal_site_hierarchy_recovers_patient_group_shift_with_site_variation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_stage_sites.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("patient_id,site_id,group,outcome\n");
    for site in 0..8 {
        for patient in 0..4 {
            let outcome = if (site + patient) % 4 == 0 { "II" } else { "I" };
            writeln!(csv, "s{site}-mss-{patient},s{site},MSS,{outcome}").expect("reference row");
        }
        for patient in 0..4 {
            let outcome = if (site + patient) % 4 == 0 {
                "III"
            } else {
                "IV"
            };
            writeln!(csv, "s{site}-msi-{patient},s{site},MSI,{outcome}").expect("comparison row");
        }
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "ordinal-group-site-hierarchy",
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
            "--site-intercept-sd-prior-sd",
            "1",
            "--site-group-slope-sd-prior-sd",
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
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_ordinal_group_site_hierarchy"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["model"]["biological_unit"], "patient");
    assert_eq!(result["input"]["patient_count"], 64);
    assert_eq!(result["input"]["site_count"], 8);
    assert!(
        result["posterior"]["global_group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 2.0
    );
    assert!(
        result["posterior"]["site_intercept_sd"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["posterior"]["site_group_slope_sd"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_patient_ordinal_site_varying_group_association"
    );
}
