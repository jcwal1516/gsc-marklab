#![cfg(feature = "cli")]

use std::{fmt::Write, fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, out: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "gaussian-crossed-nested-hierarchy",
        "--project",
        project.to_str().expect("project path"),
        "--input",
        input.to_str().expect("input path"),
        "--intercept-prior-sd",
        "3",
        "--slope-prior-sd",
        "2",
        "--component-prior-sd",
        "0.5",
        "--chains",
        "2",
        "--tune",
        "100",
        "--draws",
        "100",
        "--target-accept",
        "0.9",
        "--seed",
        seed,
        "--timeout-seconds",
        "180",
        "--out",
        out.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn crossed_nested_hierarchy_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hierarchy.csv");
    let mut csv = String::from("patient_id,slide_id,roi_id,batch_id,cohort_id,exposure,outcome\n");
    for cohort in 0..3 {
        for patient_within in 0..4 {
            let patient = cohort * 4 + patient_within;
            for slide in 0..2 {
                for roi in 0..2 {
                    for replicate in 0..2 {
                        let exposure = if replicate == 0 { -1.0 } else { 1.0 };
                        let batch = (patient + slide + roi + replicate) % 4;
                        let outcome = cohort as f64 * 0.4
                            + patient_within as f64 * 0.1
                            + slide as f64 * 0.15
                            + roi as f64 * 0.08
                            + batch as f64 * 0.03
                            + exposure * (1.0 + cohort as f64 * 0.1)
                            + if replicate == 0 { -0.2 } else { 0.2 };
                        writeln!(
                            csv,
                            "p{patient},p{patient}-s{slide},p{patient}-s{slide}-r{roi},b{batch},c{cohort},{exposure},{outcome}"
                        )
                        .expect("CSV row");
                    }
                }
            }
        }
    }
    fs::write(&input, csv).expect("fixture");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &input, &first, "20260901")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "20260901")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(
        fs::read(&first).expect("miss"),
        fs::read(&second).expect("hit")
    );
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&first).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_gaussian_crossed_nested_hierarchy"
    );

    command(
        &project,
        &input,
        &directory.path().join("changed.json"),
        "20260902",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );
}
