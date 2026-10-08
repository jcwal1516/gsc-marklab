#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write, fs, path::Path};
fn command(project: &Path, input: &Path, out: &Path, seed: &str) -> Command {
    let mut c = Command::cargo_bin("marklab").unwrap();
    c.args([
        "project",
        "nonproportional-ordinal-group-site",
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
        "--cutpoint-prior-sd",
        "2",
        "--site-intercept-sd-prior-sd",
        "1",
        "--chains",
        "4",
        "--tune",
        "800",
        "--draws",
        "800",
        "--target-accept",
        "0.95",
        "--seed",
        seed,
        "--timeout-seconds",
        "180",
        "--out",
        out.to_str().unwrap(),
    ]);
    c
}
#[test]
fn nonproportional_ordinal_replays_without_pymc() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("sites.csv");
    let mut csv = String::from("patient_id,site_id,group,outcome\n");
    let a = ["I", "I", "I", "I", "II", "III", "IV", "IV"];
    let b = ["I", "II", "II", "II", "II", "II", "III", "IV"];
    for site in 0..8 {
        for (i, y) in a.iter().enumerate() {
            writeln!(csv, "s{site}-a-{i},s{site},MSS,{y}").unwrap()
        }
        for (i, y) in b.iter().enumerate() {
            writeln!(csv, "s{site}-b-{i},s{site},MSI,{y}").unwrap()
        }
    }
    fs::write(&input, csv).unwrap();
    let project = d.path().join("project");
    let first = d.path().join("first.json");
    let second = d.path().join("second.json");
    command(&project, &input, &first, "20260831")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "20260831")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    command(&project, &input, &d.path().join("changed.json"), "20260901")
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
}
