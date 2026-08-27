#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn beta_binomial_hierarchy_sbc_has_complete_ranks_coverage_and_disposition() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("shape.csv");
    let output = directory.path().join("sbc.json");
    fs::write(
        &input,
        "patient_id,successes,trials\n\
p-1,0,100\n\
p-2,0,100\n\
p-3,0,100\n\
p-4,0,100\n\
p-5,0,100\n\
p-6,0,100\n\
p-7,0,100\n\
p-8,0,100\n",
    )
    .expect("shape");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-hierarchy-sbc",
            "--input",
            input.to_str().unwrap(),
            "--population-alpha",
            "2",
            "--population-beta",
            "2",
            "--concentration-prior-sd",
            "20",
            "--replicates",
            "20",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "3000",
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
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_hierarchy_sbc"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert!(result["failures"].as_array().unwrap().is_empty());
    for parameter in [
        "population_probability",
        "concentration",
        "patient_probability_0",
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
