#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_the_same_beta_binomial_hierarchy() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("agreement.json");
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
            "beta-binomial-hierarchy-agreement",
            "--input",
            input.to_str().unwrap(),
            "--population-alpha",
            "2",
            "--population-beta",
            "2",
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
        "marklab.bayesian_beta_binomial_cross_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    for parameter in ["population_probability", "concentration"] {
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
