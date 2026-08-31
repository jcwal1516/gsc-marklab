#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "causal-randomized-interference",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn randomized_interference_replays_with_exact_assignment_probabilities() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &input,
        br#"{"design_provenance":"synthetic_line_randomized_v1","graph_provenance":"prespecified_line_before_outcomes","units":[{"unit_id":"u1","cluster_id":"cluster","treatment":true,"treatment_time":0.0,"outcome":3.0,"outcome_time":1.0,"eligible":true,"baseline_covariates":[{"name":"baseline","value":0.0,"measurement_time":-1.0}]},{"unit_id":"u2","cluster_id":"cluster","treatment":true,"treatment_time":0.0,"outcome":3.0,"outcome_time":1.0,"eligible":true,"baseline_covariates":[{"name":"baseline","value":0.0,"measurement_time":-1.0}]},{"unit_id":"u3","cluster_id":"cluster","treatment":false,"treatment_time":0.0,"outcome":1.0,"outcome_time":1.0,"eligible":true,"baseline_covariates":[{"name":"baseline","value":0.0,"measurement_time":-1.0}]},{"unit_id":"u4","cluster_id":"cluster","treatment":false,"treatment_time":0.0,"outcome":0.0,"outcome_time":1.0,"eligible":true,"baseline_covariates":[{"name":"baseline","value":0.0,"measurement_time":-1.0}]}],"graph_edges":[{"left_unit":"u1","right_unit":"u2"},{"left_unit":"u2","right_unit":"u3"},{"left_unit":"u3","right_unit":"u4"}],"cluster_assignments":[{"cluster_id":"cluster","treated_units":2}],"test_exposure_high":{"own_treated":true,"neighbor_any_treated":true},"test_exposure_low":{"own_treated":false,"neighbor_any_treated":false},"permutations":199,"seed":20260825,"maximum_assignment_states":100,"maximum_unit_assignment_evaluations":10000}"#,
    )
    .expect("fixture");

    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.randomized_binary_interference");
    assert_eq!(result["assignment_states"], 6);
    let endpoint_probability = result["exposure_probabilities"][0]["probabilities"]
        ["treated_neighbor_treated"]
        .as_f64()
        .unwrap();
    assert!((endpoint_probability - 1.0 / 6.0).abs() <= 1.0e-12);
    assert_eq!(
        result["randomization_test"]["null_values"]
            .as_array()
            .unwrap()
            .len(),
        199
    );
    assert_eq!(result["claim_status"], "randomized_design_mechanics_only");
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
