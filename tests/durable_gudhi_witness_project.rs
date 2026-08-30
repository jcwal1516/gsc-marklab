#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab_workflow::ContentDigest;

fn command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "witness-persistence",
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
fn witness_persistence_replays_without_a_second_gudhi_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("witness.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let mut spec = serde_json::json!({
        "points":[
            {"id":"a","coordinates_um":[0.0]},
            {"id":"b","coordinates_um":[1.0]},
            {"id":"c","coordinates_um":[2.0]},
            {"id":"d","coordinates_um":[3.0]},
            {"id":"e","coordinates_um":[4.0]}
        ],
        "landmark_method":"farthest_point",
        "landmark_count":3,
        "maximum_dimension":1,
        "nu":0,
        "max_scale_um":1.0,
        "coefficient_field":2,
        "maximum_simplices":100,
        "timeout_seconds":30
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");
    let input_digest = ContentDigest::from_bytes(&fs::read(&input).unwrap());

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let first_bytes = fs::read(&first).expect("first result");
    assert_eq!(first_bytes, fs::read(&second).expect("second result"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format"], "marklab.witness_persistence");
    assert_eq!(result["backend"]["name"], "gudhi");
    assert_eq!(result["backend"]["version"], "3.13.0");
    assert_eq!(result["backend"]["python_version"], "3.12");
    assert_eq!(result["landmark_ids"], serde_json::json!(["a", "e", "c"]));

    spec["max_scale_um"] = serde_json::json!(1.5);
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("changed fixture");
    command(&project, &input, &directory.path().join("changed.json"))
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "external backend execution is disabled",
        ));

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "a replay must not append an execution");
    let record: serde_json::Value = serde_json::from_str(records[0]).expect("ledger JSON");
    assert_eq!(
        record["identity"]["node"]["id"],
        "gudhi-witness-persistence"
    );
    assert_eq!(
        record["identity"]["inputs"][0]["digest"],
        input_digest.to_string()
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.gudhi_witness_persistence_result"
    );
}

#[test]
fn dense_witness_result_above_one_mib_replays_without_a_second_gudhi_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("dense-witness.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let points = (0..512)
        .map(|index| {
            let column = index % 32;
            let row = index / 32;
            serde_json::json!({
                "id":format!("dense-field:{index:09}"),
                "coordinates_um":[
                    column as f64 * 7.0 + (row % 3) as f64 * 0.37,
                    row as f64 * 7.0 + (column % 5) as f64 * 0.23
                ]
            })
        })
        .collect::<Vec<_>>();
    let spec = serde_json::json!({
        "points":points,
        "landmark_method":"farthest_point",
        "landmark_count":64,
        "maximum_dimension":2,
        "nu":0,
        "max_scale_um":200.0,
        "coefficient_field":2,
        "maximum_simplices":500000,
        "timeout_seconds":180
    });
    fs::write(&input, serde_json::to_vec(&spec).unwrap()).expect("fixture");

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    assert!(
        fs::metadata(&first).unwrap().len() > 1024 * 1024,
        "the regression must exercise a result above the former one-MiB ceiling"
    );
    command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
