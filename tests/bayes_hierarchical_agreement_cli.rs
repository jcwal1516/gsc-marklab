#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_the_same_typed_patient_hierarchy() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "patient_id,observation\n\
p-1,0.2\n\
p-1,0.4\n\
p-1,0.6\n\
p-1,0.8\n\
p-2,0.8\n\
p-2,1.0\n\
p-2,1.2\n\
p-2,1.4\n\
p-3,1.4\n\
p-3,1.6\n\
p-3,1.8\n\
p-3,2.0\n\
p-4,2.0\n\
p-4,2.2\n\
p-4,2.4\n\
p-4,2.6\n\
p-5,2.6\n\
p-5,2.8\n\
p-5,3.0\n\
p-5,3.2\n\
p-6,3.2\n\
p-6,3.4\n\
p-6,3.6\n\
p-6,3.8\n",
    )
    .expect("fixture");
    let output = directory.path().join("agreement.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "hierarchical-normal-agreement",
            "--input",
            input.to_str().expect("input path"),
            "--global-prior-mean",
            "0",
            "--global-prior-sd",
            "5",
            "--between-patient-sd-prior",
            "2",
            "--known-sigma",
            "1",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "8000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260825",
            "--maximum-standardized-difference",
            "4",
            "--minimum-absolute-tolerance",
            "0.05",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_hierarchical_cross_backend_agreement"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["fit_state"], "complete",
        "PyMC: {}\nNumPyro: {}",
        result["pymc"]["diagnostics"], result["numpyro"]["diagnostics"]
    );
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    assert_eq!(result["numpyro"]["backend"]["version"], "0.21.0");
    assert_eq!(result["pymc"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["numpyro"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["comparison"]["global_mean"]["passes"], true);
    assert_eq!(result["comparison"]["between_patient_sd"]["passes"], true);
    assert!(
        result["comparison"]["global_mean"]["absolute_difference"]
            .as_f64()
            .expect("global difference")
            <= 0.05
    );
    assert!(
        result["comparison"]["between_patient_sd"]["absolute_difference"]
            .as_f64()
            .expect("scale difference")
            <= 0.1
    );
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
