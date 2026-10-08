#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use marklab::ClassicalSpatialResultDocument;

fn write_window(path: &Path) {
    fs::write(
        path,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,4],[-1,4],[-1,-1]]]]}"#,
    )
    .expect("window");
}

fn classical_command(cells: &Path, mask: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "classical",
        "--cells",
        cells.to_str().expect("cells path"),
        "--mask",
        mask.to_str().expect("mask path"),
        "--out",
        out.to_str().expect("out path"),
        "--r-max-um",
        "0.5",
        "--r-steps",
        "2",
        "--simulations",
        "19",
        "--seed",
        "43",
        "--alpha",
        "0.05",
        "--memory-budget-mib",
        "4",
        "--max-pair-visits",
        "1000000",
        "--max-csr-draws",
        "100000",
    ]);
    command
}

#[test]
fn classical_cli_writes_one_atomic_claim_bounded_result_bundle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let mask = directory.path().join("window.geojson");
    let out = directory.path().join("classical-out");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0,0,0,classical_cli,baseline,unmarked,true,true\n\
1,0,1,classical_cli,baseline,unmarked,true,true\n\
2,0,0,classical_cli,baseline,unmarked,true,true\n\
3,0,1,classical_cli,baseline,unmarked,true,true\n",
    )
    .expect("cells");
    write_window(&mask);

    classical_command(&cells, &mask, &out).assert().success();

    let mut artifacts = fs::read_dir(&out)
        .expect("output directory")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    assert_eq!(artifacts, ["report.md", "result.json", "run_manifest.json"]);

    let result_text = fs::read_to_string(out.join("result.json")).expect("result");
    let document = ClassicalSpatialResultDocument::from_json(&result_text).expect("strict result");
    assert_eq!(document.analysis().case_id, "classical_cli");
    assert_eq!(document.analysis().timepoint, "baseline");
    assert_eq!(document.analysis().curve.len(), 2);
    assert_eq!(document.analysis().null_design.simulations, 19);
    assert_eq!(document.analysis().null_design.seed, 43);
    let result_value: serde_json::Value = serde_json::from_str(&result_text).expect("result value");
    assert_eq!(result_value["analysis"]["configuration"]["radius_count"], 2);
    assert_eq!(
        result_value["analysis"]["configuration"]["logical_digest"]
            .as_str()
            .expect("configuration digest")
            .len(),
        64
    );
    assert_eq!(result_value["workflow"]["cache_status"], "miss");
    assert_eq!(
        result_value["workflow"]["cache_key"]
            .as_str()
            .expect("cache key")
            .len(),
        64
    );

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out.join("run_manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    assert_eq!(manifest["command"], "classical");
    assert_eq!(
        manifest["inputs"]["cells"],
        cells.to_string_lossy().as_ref()
    );
    assert_eq!(manifest["inputs"]["mask"], mask.to_string_lossy().as_ref());
    assert_eq!(manifest["execution"]["cache_status"], "miss");
    assert_eq!(
        manifest["scientific_design"]["randomization_unit"],
        "whole_location_pattern"
    );

    let report = fs::read_to_string(out.join("report.md")).expect("report");
    for required in [
        "Homogeneous Ripley K and L",
        "standard border",
        "conditional homogeneous CSR",
        "whole location pattern",
        "does not establish a biological mechanism",
        "does not provide patient-level inference",
    ] {
        assert!(report.contains(required), "report missing {required:?}");
    }
}

#[test]
fn classical_cli_preserves_typed_empty_and_singleton_results() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mask = directory.path().join("window.geojson");
    write_window(&mask);

    for (name, x_um, expected_count) in [("singleton", 0.0, 1), ("empty", 10.0, 0)] {
        let cells = directory.path().join(format!("{name}.csv"));
        let out = directory.path().join(format!("{name}-out"));
        fs::write(
            &cells,
            format!(
                "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n{x_um},0,0,{name},baseline,unmarked,true,true\n"
            ),
        )
        .expect("cells");

        classical_command(&cells, &mask, &out).assert().success();
        let document = ClassicalSpatialResultDocument::from_json(
            &fs::read_to_string(out.join("result.json")).expect("result"),
        )
        .expect("document");
        assert_eq!(document.analysis().geometry.point_count, expected_count);
        assert_eq!(
            document.analysis().status,
            marklab::ClassicalSpatialStatus::InsufficientPoints
        );
        assert!(document.analysis().inference.is_none());
    }
}

#[test]
fn classical_cli_failure_never_commits_or_overwrites_output() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cells = directory.path().join("cells.csv");
    let malformed_mask = directory.path().join("malformed.geojson");
    let absent_out = directory.path().join("absent-out");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n0,0,0,failure,baseline,unmarked,true,true\n",
    )
    .expect("cells");
    fs::write(&malformed_mask, "{not-json").expect("malformed mask");
    classical_command(&cells, &malformed_mask, &absent_out)
        .assert()
        .failure();
    assert!(!absent_out.exists());

    let mask = directory.path().join("window.geojson");
    let occupied_out = directory.path().join("occupied-out");
    write_window(&mask);
    fs::create_dir(&occupied_out).expect("occupied output");
    fs::write(occupied_out.join("sentinel.txt"), "preserve me").expect("sentinel");
    classical_command(&cells, &mask, &occupied_out)
        .assert()
        .failure();
    assert_eq!(
        fs::read_to_string(occupied_out.join("sentinel.txt")).expect("sentinel retained"),
        "preserve me"
    );
    assert!(!occupied_out.join("result.json").exists());
}

#[cfg(feature = "parquet")]
#[test]
fn classical_cli_csv_and_parquet_inputs_produce_the_same_analysis() {
    let directory = tempfile::tempdir().expect("tempdir");
    let csv = directory.path().join("cells.csv");
    let parquet = directory.path().join("cells.parquet");
    let mask = directory.path().join("window.geojson");
    let csv_out = directory.path().join("csv-out");
    let parquet_out = directory.path().join("parquet-out");
    write_window(&mask);

    for path in [&csv, &parquet] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "simulate",
                "random-labeling",
                "--n",
                "4",
                "--p",
                "0.5",
                "--seed",
                "47",
                "--out",
                path.to_str().expect("path"),
            ])
            .assert()
            .success();
    }

    classical_command(&csv, &mask, &csv_out).assert().success();
    classical_command(&parquet, &mask, &parquet_out)
        .assert()
        .success();

    let csv_result = ClassicalSpatialResultDocument::from_json(
        &fs::read_to_string(csv_out.join("result.json")).expect("CSV result"),
    )
    .expect("CSV document");
    let parquet_result = ClassicalSpatialResultDocument::from_json(
        &fs::read_to_string(parquet_out.join("result.json")).expect("Parquet result"),
    )
    .expect("Parquet document");
    assert_eq!(csv_result.analysis(), parquet_result.analysis());
}
