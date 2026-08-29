#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_conditional_marks_replay_without_a_second_backend_execution() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let mut csv = String::from("pattern_id,patient_id,group,point_id,x_um,y_um,type_id\n");
    for (group_index, group) in ["MSS", "MSI"].into_iter().enumerate() {
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for slide_index in 0..2 {
                let pattern = format!("{patient}-s{slide_index}");
                for point_index in 0..45 {
                    let type_index = point_index % 3;
                    let type_id = ["A", "B", "C"][type_index];
                    let (x, y) = if group_index == 0 {
                        (
                            type_index as f64 * 100.0 + (point_index / 3 % 5) as f64,
                            (point_index / 15) as f64,
                        )
                    } else {
                        (
                            (point_index / 3 % 5) as f64 * 3.0 + type_index as f64,
                            (point_index / 15) as f64,
                        )
                    };
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},{pattern}-{point_index:03},{x},{y},{type_id}"
                    )
                    .unwrap();
                }
            }
        }
    }
    fs::write(&input, csv).unwrap();

    let arguments = |out: &std::path::Path| -> Vec<String> {
        vec![
            "project".into(),
            "replicated-conditional-multitype-mark".into(),
            "--project".into(),
            project.to_str().unwrap().into(),
            "--input".into(),
            input.to_str().unwrap().into(),
            "--reference-group".into(),
            "MSS".into(),
            "--reference-type".into(),
            "A".into(),
            "--radius-um".into(),
            "1.5".into(),
            "--intercept-prior-sd".into(),
            "2".into(),
            "--interaction-prior-sd".into(),
            "1".into(),
            "--group-effect-prior-sd".into(),
            "1".into(),
            "--patient-sd-prior-scale".into(),
            "0.5".into(),
            "--pattern-sd-prior-scale".into(),
            "0.5".into(),
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
            "--maximum-patients".into(),
            "10".into(),
            "--maximum-patterns".into(),
            "20".into(),
            "--maximum-points".into(),
            "1000".into(),
            "--maximum-types".into(),
            "4".into(),
            "--maximum-neighbor-visits".into(),
            "100000".into(),
            "--maximum-edges".into(),
            "10000".into(),
            "--maximum-draw-parameter-work".into(),
            "1000000".into(),
            "--maximum-working-bytes".into(),
            "16777216".into(),
            "--maximum-tree-depth".into(),
            "12".into(),
            "--timeout-seconds".into(),
            "300".into(),
            "--out".into(),
            out.to_str().unwrap().into(),
        ]
    };

    let miss = Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments(&first))
        .output()
        .expect("miss process");
    assert!(miss.status.success(), "{miss:?}");
    assert!(String::from_utf8_lossy(&miss.stderr).contains("cache_status=miss"));

    let hit = Command::cargo_bin("marklab")
        .expect("binary")
        .args(arguments(&second))
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .output()
        .expect("hit process");
    assert!(hit.status.success(), "{hit:?}");
    assert!(String::from_utf8_lossy(&hit.stderr).contains("cache_status=hit"));
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
