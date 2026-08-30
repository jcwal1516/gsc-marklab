#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn raw_vector_semivariogram_replays_and_matches_the_existing_cli() {
    let directory = tempfile::tempdir().expect("tempdir");
    let points = directory.path().join("points.csv");
    let bins = directory.path().join("bins.csv");
    let weights = directory.path().join("weights.csv");
    let project = directory.path().join("project");
    let direct = directory.path().join("direct.json");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &points,
        "object_id,x_um,y_um,embedding_0,embedding_1\na,0,0,0,0\nb,1,0,2,0\nc,3,0,2,2\n",
    )
    .expect("points");
    fs::write(&bins, "bin_id,lower_um,upper_um\nnear,0,2\nfar,2,4\n").expect("bins");
    fs::write(
        &weights,
        "left_object_id,right_object_id,weight\na,b,1\na,c,1\nb,c,2\n",
    )
    .expect("weights");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "vector-semivariogram",
            "--input",
            points.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--weights",
            weights.to_str().unwrap(),
            "--maximum-pair-visits",
            "3",
            "--out",
            direct.to_str().unwrap(),
        ])
        .assert()
        .success();

    let rejected_project = directory.path().join("rejected-project");
    let rejected_output = directory.path().join("rejected.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "project",
            "vector-semivariogram",
            "--project",
            rejected_project.to_str().unwrap(),
            "--input",
            points.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--out",
            rejected_output.to_str().unwrap(),
            "--maximum-points",
            "2",
            "--maximum-dimension",
            "2",
            "--maximum-pair-visits",
            "3",
            "--memory-budget-mib",
            "16",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "point count or embedding dimension exceeds its declared ceiling",
        ));
    assert!(!rejected_output.exists());
    assert!(!rejected_project.exists());

    let run = |output: &std::path::Path| {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "project",
            "vector-semivariogram",
            "--project",
            project.to_str().unwrap(),
            "--input",
            points.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--weights",
            weights.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--maximum-points",
            "3",
            "--maximum-dimension",
            "2",
            "--maximum-pair-visits",
            "3",
            "--memory-budget-mib",
            "16",
        ]);
        command
    };
    run(&first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    let mut replay = run(&second);
    replay.env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1");
    replay
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(fs::read(&first).unwrap(), fs::read(&direct).unwrap());
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(first).unwrap()).expect("result JSON");
    assert_eq!(result["format"], "marklab.vector_semivariogram");
    assert_eq!(result["embedding_dimension"], 2);
    assert_eq!(result["pair_visits"], 3);
    assert_eq!(result["curve"][0]["semivariance"], 2.0);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
