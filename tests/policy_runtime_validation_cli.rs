#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn runtime_validation_consumes_hmc_fit_parallelizes_calibration_and_benchmarks_equivalent_work() {
    let directory = tempfile::tempdir().expect("tempdir");
    let hmc_input = directory.path().join("hmc.json");
    fs::write(
        &hmc_input,
        serde_json::to_vec_pretty(&serde_json::json!({
            "observations":[1.8,2.1,1.9,2.2,2.0,1.7,2.3,2.0],
            "observation_standard_deviation":0.5,"prior_mean":0.0,"prior_standard_deviation":2.0,
            "initial_state":0.0,"mass":1.0,"step_size":0.05,"leapfrog_steps":20,
            "warmup":300,"draws":1000,"seed":313,"timeout_seconds":60
        }))
        .unwrap(),
    )
    .unwrap();
    let fit = directory.path().join("fit.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "hmc-normal",
            "--input",
            hmc_input.to_str().unwrap(),
            "--out",
            fit.to_str().unwrap(),
        ])
        .assert()
        .success();

    let input = directory.path().join("runtime.json");
    fs::write(&input, serde_json::to_vec_pretty(&serde_json::json!({
        "parallel_partitions":4,"alpha":0.05,
        "calibration_scenarios":[
            {"scenario_id":"null_mean","truth":0.0,"observation_standard_deviation":1.0,"sample_size":32,"repetitions":2000,"seed_namespace":317},
            {"scenario_id":"positive_mean","truth":0.5,"observation_standard_deviation":1.0,"sample_size":32,"repetitions":2000,"seed_namespace":319}
        ],
        "benchmark_sizes":[1000,2000,4000],"benchmark_repetitions":3,"benchmark_seed":331
    })).unwrap()).unwrap();
    let output = directory.path().join("runtime-result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "policy",
            "runtime-validation",
            "--input",
            input.to_str().unwrap(),
            "--fit",
            fit.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.runtime_validation_and_scaling");
    assert_eq!(result["parallel_reduction"]["partition_count"], 4);
    assert_eq!(
        result["parallel_reduction"]["ordering"],
        "fixed_contiguous_partitions_then_partition_index"
    );
    assert_eq!(result["fit_diagnostics"]["fit_kind"], "bayesian_hmc");
    assert!(result["fit_diagnostics"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|check| check["status"] == "passed"));
    for scenario in result["calibration"].as_array().unwrap() {
        assert!(scenario["bias"].as_f64().unwrap().abs() < 0.04);
        assert!((0.92..=0.98).contains(&scenario["interval_coverage"].as_f64().unwrap()));
        assert_eq!(scenario["failure_rate"], 0.0);
    }
    let benchmark = result["benchmark"].as_array().unwrap();
    assert_eq!(benchmark.len(), 3);
    assert!(benchmark.iter().all(|row| row["checksum_verified"] == true));
    assert!(benchmark
        .iter()
        .all(|row| row["median_full_inference_nanoseconds"].as_u64().unwrap() > 0));
    assert_eq!(
        result["claim_status"],
        "synthetic_runtime_validation_and_smoke_scaling_only"
    );
}
