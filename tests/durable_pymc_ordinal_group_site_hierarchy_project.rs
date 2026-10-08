#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write, fs, path::Path};

fn command(project: &Path, input: &Path, out: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").unwrap();
    command.args([
        "project",
        "ordinal-group-site-hierarchy",
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
        "--group-effect-prior-sd",
        "2",
        "--site-intercept-sd-prior-sd",
        "1",
        "--site-group-slope-sd-prior-sd",
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
    command
}

#[test]
fn ordinal_site_hierarchy_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("sites.csv");
    let mut csv = String::from("patient_id,site_id,group,outcome\n");
    for site in 0..8 {
        for patient in 0..4 {
            writeln!(
                csv,
                "s{site}-mss-{patient},s{site},MSS,{}",
                if (site + patient) % 4 == 0 { "II" } else { "I" }
            )
            .unwrap();
            writeln!(
                csv,
                "s{site}-msi-{patient},s{site},MSI,{}",
                if (site + patient) % 4 == 0 {
                    "III"
                } else {
                    "IV"
                }
            )
            .unwrap();
        }
    }
    fs::write(&input, csv).unwrap();
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
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
    command(
        &project,
        &input,
        &directory.path().join("changed.json"),
        "20260901",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    let records = fs::read_to_string(project.join("executions.jsonl")).unwrap();
    assert_eq!(records.lines().count(), 1);
}
