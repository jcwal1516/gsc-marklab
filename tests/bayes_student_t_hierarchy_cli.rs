#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn student_t_hierarchy_recovers_patient_structure_with_an_outlier() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    let output = directory.path().join("fit.json");
    fs::write(
        &input,
        "patient_id,observation\n\
p-1,0.0\np-1,0.1\np-1,0.2\np-1,8.0\n\
p-2,0.8\np-2,1.0\np-2,1.1\np-2,1.2\n\
p-3,1.8\np-3,2.0\np-3,2.1\np-3,2.2\n\
p-4,2.8\np-4,3.0\np-4,3.1\np-4,3.2\n\
p-5,3.8\np-5,4.0\np-5,4.1\np-5,4.2\n\
p-6,4.8\np-6,5.0\np-6,5.1\np-6,5.2\n",
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "student-t-hierarchy",
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
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.bayesian_student_t_hierarchy");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert!((1.5..3.5).contains(&result["posterior"]["global_mean"]["mean"].as_f64().unwrap()));
    assert!(
        result["posterior"]["between_patient_sd"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert!(
        result["posterior"]["observation_sd"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["posterior"]["degrees_of_freedom"]["mean"]
            .as_f64()
            .unwrap()
            > 2.0
    );
    assert_eq!(result["partial_pooling"].as_array().unwrap().len(), 6);
    assert!(
        result["posterior_predictive"]["observed_maximum_absolute_residual"]
            .as_f64()
            .unwrap()
            > 5.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["claim_status"], "experimental_robust_hierarchy");
}
