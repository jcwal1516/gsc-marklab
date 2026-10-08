#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn complete_randomization_derives_exact_exposure_probabilities_and_replays() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "design_provenance": "synthetic_line_randomized_v1",
            "graph_provenance": "prespecified_line_before_outcomes",
            "units": [
                unit("u1", true, 3.0),
                unit("u2", true, 3.0),
                unit("u3", false, 1.0),
                unit("u4", false, 0.0)
            ],
            "graph_edges": [
                {"left_unit": "u1", "right_unit": "u2"},
                {"left_unit": "u2", "right_unit": "u3"},
                {"left_unit": "u3", "right_unit": "u4"}
            ],
            "cluster_assignments": [
                {"cluster_id": "cluster", "treated_units": 2}
            ],
            "test_exposure_high": {"own_treated": true, "neighbor_any_treated": true},
            "test_exposure_low": {"own_treated": false, "neighbor_any_treated": false},
            "permutations": 199,
            "seed": 20260825,
            "maximum_assignment_states": 100,
            "maximum_unit_assignment_evaluations": 10000
        }))
        .unwrap(),
    )
    .unwrap();
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    run(&input, &first);
    run(&input, &second);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.randomized_binary_interference");
    assert_eq!(result["analysis_level"], "clustered_units");
    assert_eq!(
        result["null_family"],
        "randomized_interference_fixed_outcomes"
    );
    assert_eq!(
        result["randomization_unit"],
        "complete_cluster_assignment_state"
    );
    assert_eq!(result["unit_count"], 4);
    assert_eq!(result["cluster_count"], 1);
    assert_eq!(
        result["assignment_mechanism"],
        "complete_randomization_within_cluster"
    );
    assert_eq!(result["assignment_states"], 6);
    assert_eq!(result["observed_exposures"][0]["own_treated"], true);
    assert_eq!(
        result["observed_exposures"][0]["neighbor_any_treated"],
        true
    );
    let u1 = &result["exposure_probabilities"][0]["probabilities"];
    assert_close(u1["treated_neighbor_treated"].as_f64().unwrap(), 1.0 / 6.0);
    assert_close(
        u1["untreated_neighbor_untreated"].as_f64().unwrap(),
        1.0 / 6.0,
    );
    assert_eq!(
        result["randomization_test"]["null_values"]
            .as_array()
            .unwrap()
            .len(),
        199
    );
    let p = result["randomization_test"]["p_value"].as_f64().unwrap();
    assert!(p > 0.0 && p <= 1.0);
    assert_eq!(result["claim_status"], "randomized_design_mechanics_only");
}

fn unit(id: &str, treatment: bool, outcome: f64) -> serde_json::Value {
    serde_json::json!({
        "unit_id": id,
        "cluster_id": "cluster",
        "treatment": treatment,
        "treatment_time": 0.0,
        "outcome": outcome,
        "outcome_time": 1.0,
        "eligible": true,
        "baseline_covariates": [
            {"name": "baseline", "value": 0.0, "measurement_time": -1.0}
        ]
    })
}

fn run(input: &std::path::Path, output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "randomized-interference",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}
