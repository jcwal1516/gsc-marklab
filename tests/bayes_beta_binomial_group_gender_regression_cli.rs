#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn beta_binomial_group_gender_regression_recovers_both_patient_unit_effects() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("fit.json");
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
            "beta-binomial-group-gender-regression",
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
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "2000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260827",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_group_gender_regression"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["model"]["biological_unit"], "patient");
    assert_eq!(result["model"]["reference_group"], "MSS");
    assert_eq!(result["model"]["reference_gender"], "Male");
    assert!(
        result["posterior"]["group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert!(
        result["posterior"]["gender_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.1
    );
    assert!(
        result["posterior"]["marginal_probability_difference_comparison_minus_reference"]["mean"]
            .as_f64()
            .unwrap()
            > 0.15
    );
    assert_eq!(result["patients"].as_array().unwrap().len(), 16);
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_patient_group_composition_adjusted_for_gender"
    );
}
