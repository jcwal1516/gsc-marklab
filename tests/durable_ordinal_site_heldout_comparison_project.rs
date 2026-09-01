#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write, fs, path::Path};
fn command(project: &Path, input: &Path, out: &Path, smoothing: &str) -> Command {
    let mut c = Command::cargo_bin("marklab").unwrap();
    c.args([
        "project",
        "ordinal-site-heldout-comparison",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--comparison-group",
        "MSI",
        "--ordered-levels",
        "I,II,III,IV",
        "--smoothing",
        smoothing,
        "--optimizer-tolerance",
        "0.000001",
        "--maximum-optimizer-evaluations",
        "200000",
        "--out",
        out.to_str().unwrap(),
    ]);
    c
}
#[test]
fn heldout_comparison_replays_natively() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("sites.csv");
    let mut csv = String::from("patient_id,site_id,group,outcome\n");
    let a = ["I", "I", "I", "I", "II", "III", "IV", "IV"];
    let b = ["I", "II", "II", "II", "II", "II", "III", "IV"];
    for s in 0..8 {
        for (i, y) in a.iter().enumerate() {
            writeln!(csv, "s{s}-a-{i},s{s},MSS,{y}").unwrap()
        }
        for (i, y) in b.iter().enumerate() {
            writeln!(csv, "s{s}-b-{i},s{s},MSI,{y}").unwrap()
        }
    }
    fs::write(&input, csv).unwrap();
    let project = d.path().join("project");
    let first = d.path().join("first.json");
    let second = d.path().join("second.json");
    command(&project, &input, &first, "0.5")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "0.5")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    command(&project, &input, &d.path().join("changed.json"), "0.6")
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
}
