#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn group_gender_regression_reports_nine_fit_prior_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("sensitivity.json");
    fs::write(
        &input,
        "patient_id,group,gender,successes,trials\n\
mss-m-1,MSS,Male,18,100\n\
mss-m-2,MSS,Male,20,100\n\
mss-m-3,MSS,Male,22,100\n\
mss-m-4,MSS,Male,24,100\n\
mss-f-1,MSS,Female,28,100\n\
mss-f-2,MSS,Female,30,100\n\
mss-f-3,MSS,Female,32,100\n\
mss-f-4,MSS,Female,34,100\n\
msi-m-1,MSI,Male,48,100\n\
msi-m-2,MSI,Male,50,100\n\
msi-m-3,MSI,Male,52,100\n\
msi-m-4,MSI,Male,54,100\n\
msi-f-1,MSI,Female,58,100\n\
msi-f-2,MSI,Female,60,100\n\
msi-f-3,MSI,Female,62,100\n\
msi-f-4,MSI,Female,64,100\n",
    )
    .expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-group-gender-regression-sensitivity",
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
            "300",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_group_gender_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["scenarios"].as_array().unwrap().len(), 9);
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
        }));
    assert!(matches!(
        result["sensitivity_status"].as_str().unwrap(),
        "stable_within_declared_grid" | "sensitive_within_declared_grid"
    ));
}
