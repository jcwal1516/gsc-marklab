#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab::InhomogeneousCategoricalCrossPairCorrelationResult;

fn command(
    project: &Path,
    cells: &Path,
    window: &Path,
    output: &Path,
    correction: &str,
) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "inhomogeneous-categorical-cross-pair-correlation",
        "--project",
        project.to_str().expect("project"),
        "--cells",
        cells.to_str().expect("cells"),
        "--mask",
        window.to_str().expect("window"),
        "--out",
        output.to_str().expect("output"),
        "--source-level",
        "Neoplastic",
        "--target-level",
        "Inflammatory",
        "--radii-um",
        "5",
        "--intensity-bandwidth-um",
        "2",
        "--edge-correction",
        correction,
        "--pair-bandwidth-um",
        "1",
        "--grid-x",
        "20",
        "--grid-y",
        "20",
        "--simulations",
        "19",
        "--seed",
        "20260901",
        "--alpha",
        "0.05",
        "--minimum-intensity-per-um2",
        "1e-12",
        "--memory-budget-mib",
        "16",
        "--max-probes",
        "400",
        "--max-intensity-evaluations",
        "1000000",
        "--max-pair-visits",
        "100000",
        "--max-null-draws",
        "10000",
    ]);
    match correction {
        "translation" => {
            command.args([
                "--max-overlap-evaluations",
                "100000",
                "--max-overlap-candidate-work",
                "100000000",
                "--max-overlap-output-vertices",
                "10000",
            ]);
        }
        "isotropic" => {
            command.args([
                "--max-visible-arc-evaluations",
                "100000",
                "--max-arc-segment-tests",
                "100000000",
                "--max-arc-membership-queries",
                "100000000",
            ]);
        }
        other => panic!("unsupported correction {other}"),
    }
    command
}

#[test]
fn robust_inhomogeneous_multitype_corrections_execute_once_and_replay() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let window = directory.path().join("window.geojson");
    fs::write(
        &cells,
        "cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment\n\
slide-1:000000000,9,10,1,case-1,baseline,cellvit,true,true,slide-1,Neoplastic\n\
slide-1:000000001,10,10,0,case-1,baseline,cellvit,true,true,slide-1,Neoplastic\n\
slide-1:000000002,14,10,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n\
slide-1:000000003,15,10,0,case-1,baseline,cellvit,true,true,slide-1,Inflammatory\n",
    )
    .expect("cells");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[20,0],[20,20],[0,20],[0,0]]]]}"#,
    )
    .expect("window");

    for correction in ["translation", "isotropic"] {
        let project = directory.path().join(format!("{correction}-project"));
        let first = directory.path().join(format!("{correction}-first.json"));
        let second = directory.path().join(format!("{correction}-second.json"));
        command(&project, &cells, &window, &first, correction)
            .assert()
            .success()
            .stderr(predicates::str::contains("cache_status=miss"));
        command(&project, &cells, &window, &second, correction)
            .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
            .assert()
            .success()
            .stderr(predicates::str::contains("cache_status=hit"));
        assert_eq!(
            fs::read(&first).expect("miss"),
            fs::read(&second).expect("hit")
        );
        let result: InhomogeneousCategoricalCrossPairCorrelationResult =
            marklab::exact_float_json::decode(&fs::read(first).expect("result"))
                .expect("typed exact-float result");
        assert_eq!(
            result.edge_work.as_ref().expect("edge work").correction,
            correction
        );
        assert!(result.curve[0].edge_measure_sum.expect("edge measure") > 0.0);
        assert_eq!(
            fs::read_to_string(project.join("executions.jsonl"))
                .expect("ledger")
                .lines()
                .count(),
            1
        );
    }
}
