#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn beta_binomial_group_regression_recovers_a_patient_unit_probability_contrast() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("fit.json");
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
            "beta-binomial-group-regression",
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
        "marklab.bayesian_beta_binomial_group_regression"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["model"]["biological_unit"], "patient");
    assert_eq!(result["model"]["reference_group"], "MSS");
    assert_eq!(result["model"]["comparison_group"], "MSI");
    assert!(
        result["posterior"]["group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert!(
        result["posterior"]["probability_difference_comparison_minus_reference"]["mean"]
            .as_f64()
            .unwrap()
            > 0.2
    );
    assert!(result["posterior"]["odds_ratio"]["mean"].as_f64().unwrap() > 1.0);
    assert_eq!(result["patients"].as_array().unwrap().len(), 16);
    for field in [
        "probability_replicated_reference_successes_at_least_observed",
        "probability_replicated_comparison_successes_at_least_observed",
        "probability_replicated_difference_at_least_observed",
    ] {
        assert!((0.0..=1.0).contains(&result["posterior_predictive"][field].as_f64().unwrap()));
    }
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_patient_group_composition"
    );
}
