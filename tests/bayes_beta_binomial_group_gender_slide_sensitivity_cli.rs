#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn repeated_slide_hierarchy_reports_eleven_fit_prior_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("slides.csv");
    let output = directory.path().join("sensitivity.json");
    let mut csv = String::from("slide_id,patient_id,group,gender,successes,trials\n");
    for (group, gender, baseline) in [
        ("MSS", "Male", 36_u64),
        ("MSS", "Female", 68),
        ("MSI", "Male", 98),
        ("MSI", "Female", 130),
    ] {
        for patient in 0..4_u64 {
            for (slide, offset) in [(0, 0_u64), (1, 4)] {
                csv.push_str(&format!(
                    "{group}-{gender}-{patient}-s{slide},{group}-{gender}-{patient},{group},{gender},{},200\n",
                    baseline + patient * 3 + offset
                ));
            }
        }
    }
    fs::write(&input, csv).expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-group-gender-slide-hierarchy-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--reference-gender",
            "Male",
            "--comparison-gender",
            "Female",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "2",
            "--group-effect-prior-sd",
            "1",
            "--gender-effect-prior-sd",
            "1",
            "--patient-log-odds-sd-prior-sd",
            "1",
            "--slide-concentration-prior-sd",
            "20",
            "--lower-scale-multiplier",
            "0.5",
            "--upper-scale-multiplier",
            "2",
            "--material-standardized-shift",
            "0.75",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "2000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
            "--timeout-seconds",
            "300",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["scenarios"].as_array().unwrap().len(), 11);
    assert_eq!(result["scenarios"][0]["scenario_id"], "baseline");
    assert!(result["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .all(|scenario| {
            scenario["fit_state"] == "complete"
                && scenario["maximum_absolute_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
                && scenario["patient_probability_rms_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
                && scenario["patient_random_effect_rms_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
        }));
    assert!(matches!(
        result["sensitivity_status"].as_str().unwrap(),
        "stable_within_declared_grid" | "sensitive_within_declared_grid"
    ));
}
