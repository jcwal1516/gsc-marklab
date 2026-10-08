#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn student_t_hierarchy_reports_one_at_a_time_prior_and_tail_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    let output = directory.path().join("sensitivity.json");
    fs::write(&input, "patient_id,observation\np-1,0\np-1,0.1\np-1,0.2\np-1,8\np-2,1\np-2,1.1\np-2,1.2\np-2,1.3\np-3,2\np-3,2.1\np-3,2.2\np-3,2.3\np-4,3\np-4,3.1\np-4,3.2\np-4,3.3\np-5,4\np-5,4.1\np-5,4.2\np-5,4.3\np-6,5\np-6,5.1\np-6,5.2\np-6,5.3\n").expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "student-t-hierarchy-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--global-prior-mean",
            "2.5",
            "--global-prior-sd",
            "3",
            "--between-patient-sd-prior",
            "1",
            "--observation-sd-prior",
            "1",
            "--degrees-of-freedom-excess-rate",
            "0.1",
            "--lower-scale-multiplier",
            "0.5",
            "--upper-scale-multiplier",
            "2",
            "--material-standardized-shift",
            "0.75",
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
            "240",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_student_t_hierarchy_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["scenarios"].as_array().unwrap().len(), 9);
    assert_eq!(result["scenarios"][0]["scenario_id"], "baseline");
    assert!(result["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .all(|scenario| scenario["fit_state"] == "complete"));
    assert!(matches!(
        result["sensitivity_state"].as_str(),
        Some("stable_within_declared_grid" | "sensitive_within_declared_grid")
    ));
    assert_eq!(
        result["claim_status"],
        "experimental_prior_tail_sensitivity"
    );
}
