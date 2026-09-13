#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

const CHAINS: u64 = 2;
const DRAWS_PER_CHAIN: u64 = 2000;

#[test]
fn numpyro_hierarchy_sbc_has_bounded_ranks_coverage_and_failures() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = directory.path().join("sbc.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "hierarchical-normal-sbc",
            "--global-prior-mean",
            "0",
            "--global-prior-sd",
            "1",
            "--between-patient-sd-prior",
            "0.5",
            "--known-sigma",
            "1",
            "--patients",
            "6",
            "--observations-per-patient",
            "4",
            "--replicates",
            "20",
            "--chains",
            &CHAINS.to_string(),
            "--tune",
            "1000",
            "--draws",
            &DRAWS_PER_CHAIN.to_string(),
            "--target-accept",
            "0.99",
            "--seed",
            "20260826",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.7",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "300",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_hierarchical_sbc");
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["fit_state"], "complete",
        "failures: {}\ndiagnostics: {}",
        result["failures"], result["diagnostics"]
    );
    assert_eq!(
        result["replicates"].as_array().expect("replicates").len(),
        20
    );
    assert!(result["failures"].as_array().expect("failures").is_empty());
    assert!(result["replicates"]
        .as_array()
        .expect("replicates")
        .iter()
        .all(|replicate| {
            replicate["global_mean_rank"].as_u64().expect("global rank") <= CHAINS * DRAWS_PER_CHAIN
                && replicate["between_patient_sd_rank"]
                    .as_u64()
                    .expect("scale rank")
                    <= CHAINS * DRAWS_PER_CHAIN
        }));
    for parameter in ["global_mean", "between_patient_sd"] {
        assert_eq!(
            result["diagnostics"][parameter]["rank_histogram"]
                .as_array()
                .expect("histogram")
                .iter()
                .map(|value| value.as_u64().expect("count"))
                .sum::<u64>(),
            20
        );
        assert!(
            result["diagnostics"][parameter]["rank_uniformity_p_value"]
                .as_f64()
                .expect("rank p")
                >= 0.001
        );
        assert!((0.7..=1.0).contains(
            &result["diagnostics"][parameter]["coverage_90"]
                .as_f64()
                .expect("coverage")
        ));
    }
    assert_eq!(
        result["claim_status"],
        "experimental_simulation_calibration"
    );
}
