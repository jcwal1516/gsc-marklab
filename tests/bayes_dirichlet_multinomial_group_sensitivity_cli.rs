#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn dirichlet_multinomial_group_reports_one_at_a_time_prior_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_classes.csv");
    let output = directory.path().join("sensitivity.json");
    let mut csv = String::from("patient_id,group,class_id,count\n");
    for index in 0..8 {
        for (class_id, count) in [
            ("Neoplastic", 68 + index),
            ("Inflammatory", 22 - index / 2),
            ("Connective", 10 - index / 2),
        ] {
            writeln!(csv, "mss-{index},MSS,{class_id},{count}").expect("row");
        }
    }
    for index in 0..8 {
        for (class_id, count) in [
            ("Neoplastic", 38 - index / 2),
            ("Inflammatory", 42 + index),
            ("Connective", 20 - index / 2),
        ] {
            writeln!(csv, "msi-{index},MSI,{class_id},{count}").expect("row");
        }
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "dirichlet-multinomial-group-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--logit-prior-sd",
            "1.5",
            "--group-effect-prior-sd",
            "1",
            "--concentration-prior-sd",
            "50",
            "--lower-scale-multiplier",
            "0.5",
            "--upper-scale-multiplier",
            "2",
            "--material-standardized-shift",
            "0.75",
            "--chains",
            "2",
            "--tune",
            "1200",
            "--draws",
            "1600",
            "--target-accept",
            "0.97",
            "--seed",
            "20260828",
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
        "marklab.bayesian_dirichlet_multinomial_group_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    let scenarios = result["scenarios"].as_array().expect("scenarios");
    assert_eq!(scenarios.len(), 7);
    assert_eq!(scenarios[0]["scenario_id"], "baseline");
    assert!(scenarios.iter().all(|scenario| {
        scenario["fit_state"] == "complete"
            && scenario["maximum_absolute_standardized_shift"]
                .as_f64()
                .unwrap()
                .is_finite()
            && scenario["class_probability_rms_standardized_shift"]
                .as_f64()
                .unwrap()
                .is_finite()
            && scenario["concentration_standardized_shift"]
                .as_f64()
                .unwrap()
                .is_finite()
    }));
    assert!(matches!(
        result["sensitivity_status"].as_str().unwrap(),
        "stable_within_declared_grid" | "sensitive_within_declared_grid"
    ));
    assert_eq!(result["claim_status"], "experimental_prior_sensitivity");
}
