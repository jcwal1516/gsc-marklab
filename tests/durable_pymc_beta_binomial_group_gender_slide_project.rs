#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, input: &Path, output: &Path, seed: &str) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "beta-binomial-group-gender-slide-hierarchy",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--comparison-group",
        "MSI",
        "--reference-gender",
        "Male",
        "--comparison-gender",
        "Female",
        "--intercept-prior-mean",
        "0",
        "--intercept-prior-sd",
        "2",
        "--group-effect-prior-sd",
        "1",
        "--gender-effect-prior-sd",
        "1",
        "--patient-log-odds-sd-prior-sd",
        "1",
        "--slide-concentration-prior-sd",
        "20",
        "--chains",
        "2",
        "--tune",
        "1500",
        "--draws",
        "2000",
        "--target-accept",
        "0.99",
        "--seed",
        seed,
        "--timeout-seconds",
        "300",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn slide_hierarchy_runs_once_then_replays_without_starting_pymc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("slides.csv");
    let mut csv = String::from("slide_id,patient_id,group,gender,successes,trials\n");
    for (group, gender, baseline) in [
        ("MSS", "Male", 36_u64),
        ("MSS", "Female", 68),
        ("MSI", "Male", 98),
        ("MSI", "Female", 130),
    ] {
        for patient in 0..4_u64 {
            for (slide, offset) in [(0, 0_u64), (1, 4)] {
                csv.push_str(&format!(
                    "{group}-{gender}-{patient}-s{slide},{group}-{gender}-{patient},{group},{gender},{},200\n",
                    baseline + patient * 3 + offset
                ));
            }
        }
    }
    fs::write(&input, csv).expect("fixture");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");

    command(&project, &input, &first, "20260827")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &input, &second, "20260827")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");

    command(
        &project,
        &input,
        &directory.path().join("changed.json"),
        "20260828",
    )
    .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
        "external backend execution is disabled",
    ));
    let records = fs::read_to_string(project.join("executions.jsonl")).unwrap();
    assert_eq!(records.lines().count(), 1);
    let record: serde_json::Value = serde_json::from_str(records.lines().next().unwrap()).unwrap();
    assert_eq!(
        record["identity"]["node"]["id"],
        "pymc-beta-binomial-group-gender-slide-hierarchy"
    );
    assert_eq!(
        record["identity"]["result_schema"]["id"],
        "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result"
    );
}
