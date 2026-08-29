#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn conditional_multitype_mark_replays_without_second_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("points.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let mut csv = String::from("point_id,x_um,y_um,type_id\n");
    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
        for point_index in 0..30 {
            let x = type_index as f64 * 100.0 + (point_index % 6) as f64;
            let y = (point_index / 6) as f64;
            writeln!(csv, "{type_id}-{point_index:02},{x},{y},{type_id}").unwrap();
        }
    }
    fs::write(&input, csv).unwrap();

    let arguments = |out: &std::path::Path| -> Vec<String> {
        vec![
            "project".into(),
            "conditional-multitype-mark".into(),
            "--project".into(),
            project.to_str().unwrap().into(),
            "--input".into(),
            input.to_str().unwrap().into(),
            "--reference-type".into(),
            "A".into(),
            "--radius-um".into(),
            "1.5".into(),
            "--intercept-prior-sd".into(),
            "2".into(),
            "--interaction-prior-sd".into(),
            "1".into(),
            "--chains".into(),
            "2".into(),
            "--tune".into(),
            "750".into(),
            "--draws".into(),
            "1000".into(),
            "--target-accept".into(),
            "0.95".into(),
            "--seed".into(),
            "20260829".into(),
            "--maximum-points".into(),
            "100".into(),
            "--maximum-types".into(),
            "4".into(),
            "--maximum-neighbor-visits".into(),
            "10000".into(),
            "--maximum-edges".into(),
            "1000".into(),
            "--maximum-draw-parameter-work".into(),
            "40000".into(),
            "--maximum-working-bytes".into(),
            "1048576".into(),
            "--maximum-tree-depth".into(),
            "12".into(),
            "--timeout-seconds".into(),
            "300".into(),
            "--out".into(),
            out.to_str().unwrap().into(),
        ]
    };

    let first_run = Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments(&first))
        .output()
        .expect("miss process");
    assert!(first_run.status.success(), "{first_run:?}");
    assert!(String::from_utf8_lossy(&first_run.stderr).contains("cache_status=miss"));

    let second_run = Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments(&second))
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .output()
        .expect("hit process");
    assert!(second_run.status.success(), "{second_run:?}");
    assert!(String::from_utf8_lossy(&second_run.stderr).contains("cache_status=hit"));
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
