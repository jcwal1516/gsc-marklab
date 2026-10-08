#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, training: &Path, query: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "region-retrieval",
        "--project",
        project.to_str().unwrap(),
        "--training",
        training.to_str().unwrap(),
        "--query",
        query.to_str().unwrap(),
        "--k",
        "2",
        "--leakage-policy",
        "exclude_same_patient_and_site",
        "--maximum-component-candidate-visits",
        "32",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn region_retrieval_misses_once_then_replays_the_exact_typed_result() {
    let directory = tempfile::tempdir().unwrap();
    let training = directory.path().join("training.csv");
    let query = directory.path().join("query.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let provenance = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fs::write(
        &training,
        format!(
            "region_id,patient_id,site_id,split,domain,provenance_sha256,embedding_0,embedding_1\n\
             r1,p1,s1,train,tumor,{provenance},0,0\n\
             r2,p2,s2,train,tumor,{provenance},1,0\n\
             r3,p3,s3,train,tumor,{provenance},3,4\n\
             r4,p4,s4,train,tumor,{provenance},5,4\n"
        ),
    )
    .unwrap();
    fs::write(
        &query,
        format!(
            "region_id,patient_id,site_id,domain,provenance_sha256,embedding_0,embedding_1\n\
             q,p9,s9,tumor,{provenance},0.9,0\n"
        ),
    )
    .unwrap();

    command(&project, &training, &query, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &training, &query, &second)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    let first_bytes = fs::read(&first).unwrap();
    assert_eq!(first_bytes, fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).unwrap();
    assert_eq!(result["format"], "marklab.region_retrieval");
    assert_eq!(result["index"]["search"], "exact");
    assert_eq!(result["leakage_policy"], "exclude_same_patient_and_site");
    assert_eq!(result["matches"][0]["region_id"], "r2");
    assert_eq!(result["matches"][1]["region_id"], "r1");

    let ledger = fs::read_to_string(project.join("executions.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(ledger.lines().next().unwrap()).unwrap();
    assert_eq!(record["identity"]["node"]["id"], "region-retrieval");
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.region_retrieval"
    );
}
