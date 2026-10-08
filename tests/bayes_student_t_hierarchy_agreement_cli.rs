#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_the_same_student_t_hierarchy() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    let output = directory.path().join("agreement.json");
    fs::write(&input, "patient_id,observation\np-1,0\np-1,0.1\np-1,0.2\np-1,8\np-2,1\np-2,1.1\np-2,1.2\np-2,1.3\np-3,2\np-3,2.1\np-3,2.2\np-3,2.3\np-4,3\np-4,3.1\np-4,3.2\np-4,3.3\np-5,4\np-5,4.1\np-5,4.2\np-5,4.3\np-6,5\np-6,5.1\np-6,5.2\np-6,5.3\n").expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "student-t-hierarchy-agreement",
            "--input",
            input.to_str().unwrap(),
            "--global-prior-mean",
            "2.5",
            "--global-prior-sd",
            "3",
            "--between-patient-sd-prior",
            "2",
            "--observation-sd-prior",
            "1",
            "--degrees-of-freedom-excess-rate",
            "0.1",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "2000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
            "--maximum-standardized-difference",
            "5",
            "--minimum-location-scale-tolerance",
            "0.1",
            "--minimum-degrees-of-freedom-tolerance",
            "3",
            "--minimum-patient-tolerance",
            "0.1",
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
        "marklab.bayesian_student_t_cross_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    for parameter in [
        "global_mean",
        "between_patient_sd",
        "observation_sd",
        "degrees_of_freedom",
    ] {
        assert_eq!(
            result["comparison"][parameter]["passes"], true,
            "{parameter}: {result}"
        );
    }
    assert_eq!(
        result["comparison"]["patient_means"]["passes"], true,
        "{result}"
    );
    assert_eq!(result["pymc"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["numpyro"]["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
