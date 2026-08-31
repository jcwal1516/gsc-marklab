#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

fn project_command(project: &Path, input: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-sbc",
        "--project",
        project.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "reference",
        "--comparison-group",
        "comparison",
        "--reference-type",
        "A",
        "--intercept-prior-mean",
        "0.5",
        "--intercept-prior-sd",
        "0.4",
        "--group-effect-prior-sd",
        "0.3",
        "--covariate-effect-prior-sd",
        "0.2",
        "--patient-sd-prior-scale",
        "0.15",
        "--pattern-sd-prior-scale",
        "0.15",
        "--field-amplitude-prior-scale",
        "0.15",
        "--field-length-scale-prior-scale-um",
        "1",
        "--jitter",
        "0.000001",
        "--replicates-per-scenario",
        "1",
        "--chains",
        "2",
        "--tune",
        "100",
        "--draws",
        "100",
        "--target-accept",
        "0.9",
        "--seed",
        "20260831",
        "--maximum-patients",
        "8",
        "--maximum-patterns",
        "16",
        "--maximum-types",
        "3",
        "--maximum-nodes-per-pattern",
        "4",
        "--maximum-total-nodes",
        "64",
        "--maximum-total-node-type-rows",
        "192",
        "--maximum-total-events",
        "5000",
        "--maximum-draw-node-type-work",
        "38400",
        "--maximum-kernel-cube-work",
        "1024",
        "--maximum-tree-depth",
        "10",
        "--maximum-simulated-node-type-work",
        "960",
        "--maximum-total-iterations",
        "2000",
        "--maximum-ppc-pair-work",
        "4096000",
        "--maximum-working-bytes",
        "536870912",
        "--timeout-seconds",
        "600",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn multitype_sbc_replays_without_a_second_numpyro_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(&input, input_csv()).unwrap();

    project_command(&project, &input, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    project_command(&project, &input, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&second).unwrap()).unwrap();
    assert_eq!(value["scenario_dispositions"].as_array().unwrap().len(), 5);
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

fn input_csv() -> String {
    let mut csv = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
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
                    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
                        writeln!(
                            csv,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,4,{},{},{:064x},{:064x}",
                            ix as f64 + 0.5,
                            iy as f64 + 0.5,
                            ix as i32 * 2 - 1,
                            2 + type_index + node_index % 2,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        ).unwrap();
                    }
                }
            }
        }
    }
    csv
}
