#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command() -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.env(
        "MARKLAB_PYTHON",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("workers/python/.venv/bin/python"),
    );
    command
}

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = command();
    command.args([
        "project",
        "negative-binomial-hierarchy",
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
fn negative_binomial_patient_slopes_fit_and_replay_with_real_backend() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let input = directory.path().join("counts.json");
    fs::write(
        &input,
        include_bytes!("fixtures/negative_binomial_hierarchy.json"),
    )
    .unwrap();
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let project = directory.path().join("project");
    command()
        .args([
            "bayes",
            "negative-binomial-hierarchy",
            "--input",
            input.to_str().unwrap(),
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(&direct).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.negative_binomial_hierarchy");
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["patients"].as_array().unwrap().len(), 16);
    assert_eq!(result["slides"].as_array().unwrap().len(), 32);
    assert_eq!(result["rois"].as_array().unwrap().len(), 128);
    for (parameter, truth) in [("alpha", -1.0), ("beta", 0.7)] {
        let summary = &result["posterior"][parameter];
        let mean = summary["mean"].as_f64().unwrap();
        let sd = summary["sd"].as_f64().unwrap();
        assert!(
            (mean - truth).abs() <= 4.0 * sd + 0.1,
            "{parameter}: {summary}"
        );
    }
    assert!(
        result["posterior"]["beta"]["interval_lower"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(
        result["posterior_predictive"]["patients"]
            .as_array()
            .unwrap()
            .len(),
        16
    );

    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(
        fs::read(&direct).unwrap(),
        fs::read(&first).unwrap(),
        "deterministic backend rerun"
    );
    assert_eq!(
        fs::read(&first).unwrap(),
        fs::read(&second).unwrap(),
        "exact durable replay"
    );
    let mut changed: serde_json::Value =
        serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    changed["sampling"]["seed"] = serde_json::json!(20260914);
    fs::write(&input, serde_json::to_vec(&changed).unwrap()).unwrap();
    project_command(&project, &input, &directory.path().join("changed.json"))
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "external backend execution is disabled",
        ));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );

    changed["maximum_draw_observation_products"] = serde_json::json!(1);
    fs::write(&input, serde_json::to_vec(&changed).unwrap()).unwrap();
    let rejected_project = directory.path().join("rejected");
    let rejected_output = directory.path().join("rejected.json");
    project_command(&rejected_project, &input, &rejected_output)
        .assert()
        .failure();
    assert!(!rejected_project.exists());
    assert!(!rejected_output.exists());
}
