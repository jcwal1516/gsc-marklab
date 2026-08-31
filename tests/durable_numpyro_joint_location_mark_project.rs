#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

fn command(project: &Path, location: &Path, marks: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").unwrap();
    command.args([
        "project",
        "joint-replicated-location-mark",
        "--project",
        project.to_str().unwrap(),
        "--location-input",
        location.to_str().unwrap(),
        "--mark-input",
        marks.to_str().unwrap(),
        "--reference-group",
        "reference",
        "--comparison-group",
        "comparison",
        "--reference-type",
        "A",
        "--neighbor-radius-um",
        "1.1",
        "--location-prior-sd",
        "1",
        "--group-prior-sd",
        "1",
        "--patient-ecology-prior-scale",
        "0.5",
        "--mark-intercept-prior-sd",
        "1",
        "--mark-group-prior-sd",
        "1",
        "--mark-loading-prior-sd",
        "0.5",
        "--interaction-prior-sd",
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
        "20260831",
        "--maximum-patients",
        "8",
        "--maximum-patterns",
        "16",
        "--maximum-types",
        "3",
        "--maximum-location-rows",
        "192",
        "--maximum-mark-points",
        "1000",
        "--maximum-neighbor-visits",
        "100000",
        "--maximum-draw-observation-work",
        "1000000",
        "--maximum-working-bytes",
        "536870912",
        "--maximum-tree-depth",
        "10",
        "--timeout-seconds",
        "600",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

#[test]
fn joint_location_mark_fit_replays_without_three_more_backend_fits() {
    let directory = tempfile::tempdir().unwrap();
    let location = directory.path().join("location.csv");
    let marks = directory.path().join("marks.csv");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    let (location_csv, mark_csv) = fixtures();
    fs::write(&location, location_csv).unwrap();
    fs::write(&marks, mark_csv).unwrap();
    command(&project, &location, &marks, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=miss"));
    command(&project, &location, &marks, &second)
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("cache_status=hit"));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        fs::read_to_string(project.join("executions.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

fn fixtures() -> (String, String) {
    let mut location = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    let mut marks = String::from("pattern_id,patient_id,group,point_id,x_um,y_um,type_id\n");
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..2 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..4 {
                    let x = (node_index % 2) as f64 + 0.5;
                    let y = (node_index / 2) as f64 + 0.5;
                    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
                        let count =
                            2 + type_index + usize::from(group_index == 1 && type_id == "A");
                        writeln!(location,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{x},{y},1,4,{}, {count},{:064x},{:064x}",
                            node_index as i32 % 2 * 2 - 1,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        ).unwrap();
                    }
                }
                for point_index in 0..18 {
                    let type_id = ["A", "B", "C"][(point_index + group_index) % 3];
                    let x = (point_index % 6) as f64 * 0.3 + 0.1;
                    let y = (point_index / 6) as f64 * 0.6 + 0.1;
                    writeln!(
                        marks,
                        "{pattern},{patient},{group},{pattern}:{point_index:03},{x},{y},{type_id}"
                    )
                    .unwrap();
                }
            }
        }
    }
    (location, marks)
}
