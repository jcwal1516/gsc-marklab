#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn patient_hierarchy_reports_one_at_a_time_prior_sensitivity() {
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
    let output = directory.path().join("sensitivity.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "hierarchical-normal-prior-sensitivity",
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
            "--lower-scale-multiplier",
            "0.75",
            "--upper-scale-multiplier",
            "1.5",
            "--material-standardized-shift",
            "0.5",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "2000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260825",
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
        "marklab.bayesian_hierarchical_prior_sensitivity"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["base_scenario"], "baseline");
    let scenarios = result["scenarios"].as_array().expect("scenarios");
    assert_eq!(scenarios.len(), 5);
    assert_eq!(scenarios[0]["scenario_id"], "baseline");
    assert_eq!(scenarios[1]["scenario_id"], "between_patient_sd_lower");
    assert_eq!(scenarios[2]["scenario_id"], "between_patient_sd_upper");
    assert_eq!(scenarios[3]["scenario_id"], "global_sd_lower");
    assert_eq!(scenarios[4]["scenario_id"], "global_sd_upper");
    assert!(scenarios
        .iter()
        .all(|scenario| scenario["fit_state"] == "complete"));
    assert!(scenarios.iter().all(|scenario| {
        scenario["diagnostics"]["divergences"] == 0
            && scenario["global_mean_shift_standardized"]
                .as_f64()
                .expect("global shift")
                .is_finite()
            && scenario["between_patient_sd_shift_standardized"]
                .as_f64()
                .expect("scale shift")
                .is_finite()
    }));
    assert_eq!(result["claim_status"], "experimental_prior_sensitivity");
}
