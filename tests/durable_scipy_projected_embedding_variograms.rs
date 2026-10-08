#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn projected_variograms_replay_without_a_second_scipy_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    let direct = directory.path().join("direct.json");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let mut csv = String::from(
        "object_id,biological_unit,split,permutation_stratum,x_um,y_um,embedding_0,embedding_1\n",
    );
    for (unit, split, second_coordinate) in [
        ("training-unit", "train", [0.0, 0.0, 0.0, 0.0]),
        ("held-out-unit", "test", [0.0, 1000.0, -1000.0, 500.0]),
    ] {
        for index in 0..4 {
            writeln!(
                csv,
                "{unit}-{index},{unit},{split},{unit},{},0,{index},{}",
                [0, 1, 3, 6][index],
                second_coordinate[index]
            )
            .unwrap();
        }
    }
    fs::write(&input, csv).expect("input");
    fs::write(
        &bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,2,4\nfar,4,7\n",
    )
    .expect("bins");

    let arguments = |prefix: &str, output: &std::path::Path| {
        vec![
            prefix.to_owned(),
            "projected-embedding-variograms".into(),
            "--input".into(),
            input.display().to_string(),
            "--bins".into(),
            bins.display().to_string(),
            "--components".into(),
            "1".into(),
            "--permutations".into(),
            "20".into(),
            "--seed".into(),
            "731".into(),
            "--maximum-pair-visits".into(),
            "12".into(),
            "--timeout-seconds".into(),
            "120".into(),
            "--out".into(),
            output.display().to_string(),
        ]
    };
    Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments("bayes", &direct))
        .assert()
        .success();

    let run = |output: &std::path::Path| {
        let mut args = arguments("project", output);
        args.splice(2..2, ["--project".into(), project.display().to_string()]);
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args(args);
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

    assert_eq!(fs::read(&direct).unwrap(), fs::read(&first).unwrap());
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
