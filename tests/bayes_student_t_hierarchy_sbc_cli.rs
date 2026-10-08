#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn student_t_hierarchy_sbc_has_complete_ranks_coverage_and_disposition() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("shape.csv");
    let output = directory.path().join("sbc.json");
    let mut shape = String::from("patient_id,observation\n");
    for patient in 1..=12 {
        for _ in 0..8 {
            shape.push_str(&format!("p-{patient},0\n"));
        }
    }
    fs::write(&input, shape).expect("shape");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "student-t-hierarchy-sbc",
            "--input",
            input.to_str().unwrap(),
            "--global-prior-mean",
            "0",
            "--global-prior-sd",
            "1",
            "--between-patient-sd-prior",
            "0.5",
            "--observation-sd-prior",
            "0.5",
            "--degrees-of-freedom-excess-rate",
            "0.2",
            "--replicates",
            "20",
            "--chains",
            "4",
            "--tune",
            "2000",
            "--draws",
            "4000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.7",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "600",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.bayesian_student_t_hierarchy_sbc");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert!(result["failures"].as_array().unwrap().is_empty());
    for parameter in [
        "global_mean",
        "between_patient_sd",
        "observation_sd",
        "degrees_of_freedom",
    ] {
        let diagnostic = &result["diagnostics"][parameter];
        assert_eq!(
            diagnostic["rank_histogram"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap())
                .sum::<u64>(),
            20
        );
        assert!(diagnostic["rank_uniformity_p_value"].as_f64().unwrap() >= 0.001);
        assert!((0.7..=1.0).contains(&diagnostic["coverage_90"].as_f64().unwrap()));
    }
    assert_eq!(
        result["claim_status"],
        "experimental_simulation_calibration"
    );
}
