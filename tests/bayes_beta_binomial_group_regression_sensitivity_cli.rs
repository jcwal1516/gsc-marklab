#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn beta_binomial_group_regression_reports_one_at_a_time_prior_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("sensitivity.json");
    fs::write(
        &input,
        "patient_id,group,successes,trials\n\
mss-1,MSS,16,100\n\
mss-2,MSS,18,100\n\
mss-3,MSS,20,100\n\
mss-4,MSS,22,100\n\
mss-5,MSS,24,100\n\
mss-6,MSS,26,100\n\
mss-7,MSS,28,100\n\
mss-8,MSS,30,100\n\
msi-1,MSI,50,100\n\
msi-2,MSI,52,100\n\
msi-3,MSI,54,100\n\
msi-4,MSI,56,100\n\
msi-5,MSI,58,100\n\
msi-6,MSI,60,100\n\
msi-7,MSI,62,100\n\
msi-8,MSI,64,100\n",
    )
    .expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-group-regression-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "2",
            "--group-effect-prior-sd",
            "1",
            "--concentration-prior-sd",
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
            "240",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_group_regression_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["scenarios"].as_array().unwrap().len(), 7);
    assert_eq!(result["scenarios"][0]["scenario_id"], "baseline");
    assert!(result["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .all(|scenario| {
            scenario["fit_state"] == "complete"
                && scenario["group_effect_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
                && scenario["probability_difference_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
                && scenario["concentration_standardized_shift"]
                    .as_f64()
                    .unwrap()
                    .is_finite()
                && scenario["patient_probability_rms_standardized_shift"]
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
