#![cfg(feature = "cli")]

use std::{fs, process::Command};

use tempfile::tempdir;

#[test]
fn exact_max_t_calibration_replays_without_a_second_calculation() {
    let directory = tempdir().expect("temporary directory");
    let input = directory.path().join("patients.csv");
    let project = directory.path().join("project");
    let miss = directory.path().join("miss.json");
    let hit = directory.path().join("hit.json");
    fs::write(
        &input,
        concat!(
            "patient_id,e1,e2,e3\n",
            "p1,0,0,0\n",
            "p2,1,2,4\n",
            "p3,2,1,3\n",
            "p4,3,4,1\n",
            "p5,4,3,7\n",
            "p6,5,7,2\n",
            "p7,6,5,8\n",
            "p8,7,6,5\n",
        ),
    )
    .expect("input");

    let run = |out: &std::path::Path, disabled: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_marklab"));
        command.args([
            "project",
            "max-t-calibration",
            "--project",
            project.to_str().expect("project path"),
            "--input",
            input.to_str().expect("input path"),
            "--group-a-count",
            "4",
            "--family-sizes",
            "1,2",
            "--alpha",
            "0.1",
            "--maximum-assignments",
            "70",
            "--maximum-assignment-endpoint-evaluations",
            "63910",
            "--memory-budget-mib",
            "8",
            "--out",
            out.to_str().expect("output path"),
        ]);
        if disabled {
            command.env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1");
        }
        command.output().expect("project command")
    };

    let first = run(&miss, false);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stderr).contains("cache_status=miss"));
    let second = run(&hit, true);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stderr).contains("cache_status=hit"));
    assert_eq!(fs::read(miss).expect("miss"), fs::read(hit).expect("hit"));
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .expect("ledger")
            .lines()
            .count(),
        1
    );
}
