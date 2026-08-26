#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn run_tensor_case(decomposition: &str, ranks: serde_json::Value) -> serde_json::Value {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("tensor.json");
    let left = [-1.0, 0.5, 1.5];
    let middle = [0.75, -1.25, 1.0];
    let right = [1.0, 0.4, -0.8];
    let entries = (0..3)
        .flat_map(|i| {
            (0..3).flat_map(move |j| {
                (0..3).map(move |k| {
                    let observed = !matches!((i, j, k), (0, 0, 0) | (1, 2, 1) | (2, 1, 2));
                    serde_json::json!({
                        "indices":[i,j,k],
                        "value":left[i] * middle[j] * right[k],
                        "observed":observed
                    })
                })
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "tensor_id":"three_mode_oracle",
        "mode_names":["patient","feature","time"],
        "shape":[3,3,3],
        "entries":entries,
        "decomposition":decomposition,
        "ranks":ranks,
        "prior_precision":0.1,
        "noise_standard_deviation":0.05,
        "maximum_iterations":500,
        "seed":31,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "tensor-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON")
}

#[test]
fn bayesian_cp_factorization_recovers_masked_rank_one_tensor_entries() {
    let result = run_tensor_case("cp", serde_json::json!([1]));
    assert_eq!(result["format"], "marklab.bayesian_cp_factorization");
    assert_eq!(result["backend"]["scipy_version"], "1.18.1");
    assert_eq!(result["masked_predictions"].as_array().unwrap().len(), 3);
    assert!(result["masked_rmse"].as_f64().unwrap() < 0.25);
    assert_eq!(result["alignment"]["order"], "descending_component_energy");
    assert_eq!(result["fit_state"], "approximate_only");
}

#[test]
fn bayesian_tucker_factorization_recovers_masked_rank_one_tensor_entries() {
    let result = run_tensor_case("tucker", serde_json::json!([1, 1, 1]));
    assert_eq!(result["format"], "marklab.bayesian_tucker_factorization");
    assert_eq!(result["backend"]["jax_version"], "0.11.1");
    assert_eq!(result["masked_predictions"].as_array().unwrap().len(), 3);
    assert!(result["masked_rmse"].as_f64().unwrap() < 0.25);
    assert_eq!(result["alignment"]["sign"], "max_mode_loading_positive");
    assert_eq!(result["fit_state"], "approximate_only");
}
