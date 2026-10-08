#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn inferred_kernel_replicated_lgcp_replays_without_second_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let project = directory.path().join("project");
    let input = directory.path().join("patterns.csv");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let mut csv = String::from(
        "pattern_id,patient_id,group,cohort,node_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..4 {
                    let ix = node_index % 2;
                    let iy = node_index / 2;
                    let base = if group_index == 0 { 3 } else { 12 };
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},synthetic,q-{node_index},{},{},1,4,{},{},{:064x},{:064x}",
                        ix as f64 + 0.5,
                        iy as f64 + 0.5,
                        ix * 2 - 1,
                        base + ix + pattern_index + patient_index * 2,
                        group_index * 100 + patient_index * 10 + pattern_index + 1,
                        group_index * 1000 + patient_index * 100 + pattern_index + 1,
                    )
                    .unwrap();
                }
            }
        }
    }
    fs::write(&input, csv).unwrap();

    command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicate::str::contains("cache_status=miss"));
    let mut replay = command(&project, &input, &second);
    replay.env("MARKLAB_BACKEND_EXECUTION_DISABLED", "1");
    replay
        .assert()
        .success()
        .stderr(predicate::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    let result: serde_json::Value = serde_json::from_slice(&fs::read(first).unwrap()).unwrap();
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["statistical_unit"], "patient");
    assert!(
        result["posterior"]["field_length_scale_um"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
}

fn command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "replicated-arbitrary-window-lgcp-inferred-kernel",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "reference",
        "--comparison-group",
        "comparison",
        "--intercept-prior-mean",
        "1",
        "--intercept-prior-sd",
        "1",
        "--group-effect-prior-sd",
        "1",
        "--covariate-effect-prior-sd",
        "1",
        "--patient-sd-prior-scale",
        "0.5",
        "--pattern-sd-prior-scale",
        "0.5",
        "--field-amplitude-prior-scale",
        "0.5",
        "--field-length-scale-prior-scale-um",
        "2",
        "--jitter",
        "0.000001",
        "--chains",
        "2",
        "--tune",
        "1500",
        "--draws",
        "1500",
        "--target-accept",
        "0.95",
        "--seed",
        "16211",
        "--maximum-patients",
        "8",
        "--maximum-patterns",
        "16",
        "--maximum-nodes-per-pattern",
        "4",
        "--maximum-total-nodes",
        "64",
        "--maximum-total-events",
        "1000",
        "--maximum-draw-node-work",
        "192000",
        "--maximum-kernel-cube-work",
        "1024",
        "--maximum-tree-depth",
        "13",
        "--timeout-seconds",
        "300",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}
