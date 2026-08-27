#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn beta_binomial_hierarchy_reports_one_at_a_time_prior_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("sensitivity.json");
    fs::write(
        &input,
        "patient_id,successes,trials\n\
p-1,12,100\n\
p-2,18,100\n\
p-3,22,100\n\
p-4,27,100\n\
p-5,31,100\n\
p-6,35,100\n\
p-7,40,100\n\
p-8,45,100\n",
    )
    .expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-hierarchy-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--population-alpha",
            "2",
            "--population-beta",
            "2",
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
        "marklab.bayesian_beta_binomial_hierarchy_sensitivity"
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
                && scenario["population_probability_standardized_shift"]
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
