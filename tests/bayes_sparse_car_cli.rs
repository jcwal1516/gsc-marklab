#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "sparse-car-fit",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

fn sparse_car_spec(seed: u64, maximum_work: u64) -> serde_json::Value {
    serde_json::json!({
        "graph_definition": "symmetric three-region chain with preserved unit weights",
        "coordinate_frame": "synthetic_region_index",
        "observations": [
            {"region_id": "r1", "prior_mean": 0.2, "value": 1.2, "noise_sd": 0.5},
            {"region_id": "r2", "prior_mean": -0.1, "value": null, "noise_sd": 0.8},
            {"region_id": "r3", "prior_mean": 0.3, "value": -0.4, "noise_sd": 0.7}
        ],
        "edges": [
            {"source_region": "r1", "target_region": "r2", "weight": 1.0},
            {"source_region": "r2", "target_region": "r1", "weight": 1.0},
            {"source_region": "r2", "target_region": "r3", "weight": 1.0},
            {"source_region": "r3", "target_region": "r2", "weight": 1.0}
        ],
        "rho": 0.6,
        "tau": 1.4,
        "draws": 4096,
        "seed": seed,
        "solve_tolerance": 1.0e-10,
        "maximum_iterations": 100,
        "maximum_work": maximum_work,
        "memory_budget_bytes": 4194304
    })
}

#[test]
fn sparse_car_direct_fit_and_durable_replay_share_the_native_result() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("sparse-car.json");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let changed_seed = directory.path().join("changed-seed.json");
    fs::write(
        &input,
        serde_json::to_vec(&sparse_car_spec(20260912, 1_000_000)).unwrap(),
    )
    .expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "sparse-car-fit",
            "--input",
            input.to_str().unwrap(),
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();
    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&direct).unwrap(), fs::read(&first).unwrap());
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.sparse_car_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete");
    let regions = result["regions"].as_array().expect("region results");
    assert_eq!(regions.len(), 3);
    assert!(regions.iter().any(|region| region["region_id"] == "r2"));
    for region in regions {
        for field in [
            "posterior_mean",
            "posterior_sd",
            "lower_95",
            "upper_95",
            "predictive_sd",
            "sd_mcse",
        ] {
            assert!(region[field].as_f64().is_some(), "finite {field}");
        }
    }

    fs::write(
        &input,
        serde_json::to_vec(&sparse_car_spec(20260913, 1_000_000)).unwrap(),
    )
    .expect("changed seed fixture");
    project_command(&project, &input, &changed_seed)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        2
    );

    let rejected_input = directory.path().join("rejected.json");
    let rejected_project = directory.path().join("rejected-project");
    let rejected_output = directory.path().join("rejected-output.json");
    let mut rejected_spec = sparse_car_spec(20260912, 1_000_000);
    rejected_spec["memory_budget_bytes"] = serde_json::json!(1);
    fs::write(&rejected_input, serde_json::to_vec(&rejected_spec).unwrap())
        .expect("rejected fixture");
    project_command(&rejected_project, &rejected_input, &rejected_output)
        .assert()
        .failure();
    assert!(!rejected_project.exists());
    assert!(!rejected_output.exists());
}
