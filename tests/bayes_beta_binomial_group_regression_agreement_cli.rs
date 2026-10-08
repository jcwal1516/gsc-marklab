#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_the_same_beta_binomial_group_regression() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("agreement.json");
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
            "beta-binomial-group-regression-agreement",
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
            "2000",
            "--draws",
            "3000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
            "--maximum-standardized-difference",
            "5",
            "--minimum-probability-tolerance",
            "0.02",
            "--minimum-log-odds-tolerance",
            "0.1",
            "--minimum-concentration-tolerance",
            "2",
            "--minimum-patient-tolerance",
            "0.02",
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
        "marklab.bayesian_beta_binomial_group_cross_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    for parameter in [
        "intercept_log_odds",
        "group_log_odds_effect",
        "reference_probability",
        "comparison_probability",
        "probability_difference",
        "odds_ratio",
        "concentration",
    ] {
        assert_eq!(
            result["comparison"][parameter]["passes"], true,
            "{parameter}: {result}"
        );
    }
    assert_eq!(
        result["comparison"]["patient_probabilities"]["passes"], true,
        "{result}"
    );
    assert_eq!(result["pymc"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["numpyro"]["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
