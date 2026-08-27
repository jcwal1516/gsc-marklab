#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn crc_adapter_builds_patient_and_site_held_out_m0_m1_retrieval_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let molecular = directory.path().join("molecular.csv");
    let stages = directory.path().join("stages.csv");
    let spatial = directory.path().join("spatial.csv");
    let microenvironment = directory.path().join("microenvironment.csv");
    let output = directory.path().join("admission");

    let mut molecular_csv = String::from("patient_id,class_name\n");
    let mut stage_csv = String::from(
        "patient_id,submitter_id,project_id,stage,raw_pathologic_stage,diagnosis_ids\n",
    );
    let mut spatial_csv = String::from(
        "patient_id,project_id,tissue_source_site,cell_density_per_mm2,mean_background_fraction\n",
    );
    let mut microenvironment_csv = String::from(
        "patient_id,tumor_cell_count,all_cell_count,inflammatory_cell_count,connective_cell_count,median_inflammatory_count_50um,median_stromal_distance_um,tumor_inflammatory_relative_excess_0_50,tumor_connective_relative_excess_0_50,stromal_context_residual_organization,inflammatory_context_residual_organization,combined_context_residual_organization\n",
    );
    for patient in 0..24_u32 {
        let group = if patient % 3 == 0 { "MSI" } else { "MSS" };
        let project = if patient % 5 == 0 {
            "TCGA-READ"
        } else {
            "TCGA-COAD"
        };
        let stage = ["Stage I", "Stage II", "Stage III", "Stage IV"][patient as usize % 4];
        let site = patient % 4;
        writeln!(molecular_csv, "p{patient},{group}").unwrap();
        writeln!(
            stage_csv,
            "p{patient},TCGA-S{site}-{patient:04},{project},{stage},{stage},d{patient}"
        )
        .unwrap();
        writeln!(
            spatial_csv,
            "p{patient},{project},S{site},{},0",
            1000.0 + patient as f64 * 11.0,
        )
        .unwrap();
        let all = 1000.0 + patient as f64 * 10.0;
        writeln!(
            microenvironment_csv,
            "p{patient},{},{all},{},{},{},{},{},{},{},{},{}",
            400.0 + patient as f64,
            200.0 + patient as f64,
            300.0 - patient as f64,
            2.0 + patient as f64 / 10.0,
            40.0 + patient as f64,
            -0.2 + patient as f64 / 100.0,
            -0.1 + patient as f64 / 200.0,
            0.1 + patient as f64 / 1000.0,
            0.2 + patient as f64 / 1000.0,
            0.3 + patient as f64 / 1000.0,
        )
        .unwrap();
    }
    fs::write(&molecular, molecular_csv).unwrap();
    fs::write(&stages, stage_csv).unwrap();
    fs::write(&spatial, spatial_csv).unwrap();
    fs::write(&microenvironment, microenvironment_csv).unwrap();

    Command::new("python3")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("workers/python/marklab_tcga_crc_fingerprint_adapter.py"),
        )
        .args(["--molecular-labels", molecular.to_str().unwrap()])
        .args(["--stage-labels", stages.to_str().unwrap()])
        .args(["--spatial-metrics", spatial.to_str().unwrap()])
        .args([
            "--microenvironment-metrics",
            microenvironment.to_str().unwrap(),
        ])
        .args(["--out", output.to_str().unwrap()])
        .assert()
        .success();

    let admission: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("admission.json")).unwrap()).unwrap();
    assert_eq!(
        admission["schema_name"],
        "marklab_crc_fingerprint_admission"
    );
    assert_eq!(admission["patient_count"], 24);
    assert_eq!(admission["fold_count"], 96);
    assert_eq!(
        admission["feature_construction"],
        "prespecified_without_molecular_labels"
    );

    let mut folds = csv::Reader::from_path(output.join("folds.csv")).unwrap();
    let headers = folds.headers().unwrap().clone();
    let rows = folds.records().collect::<Result<Vec<_>, _>>().unwrap();
    let selected = rows
        .iter()
        .find(|row| {
            row.get(headers.iter().position(|name| name == "lane").unwrap()) == Some("m1")
                && row.get(
                    headers
                        .iter()
                        .position(|name| name == "holdout_policy")
                        .unwrap(),
                ) == Some("site_held_out")
                && row.get(
                    headers
                        .iter()
                        .position(|name| name == "patient_id")
                        .unwrap(),
                ) == Some("p0")
        })
        .unwrap();
    let training = output.join(
        &selected[headers
            .iter()
            .position(|name| name == "training_path")
            .unwrap()],
    );
    let query = output.join(
        &selected[headers
            .iter()
            .position(|name| name == "query_path")
            .unwrap()],
    );
    let mut training_reader = csv::Reader::from_path(training).unwrap();
    assert!(training_reader
        .headers()
        .unwrap()
        .iter()
        .skip(6)
        .all(|name| name.starts_with("embedding_")));
    let training_rows = training_reader
        .records()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(training_rows
        .iter()
        .all(|row| row.get(1) != Some("p0") && row.get(2) != Some("S0")));
    let mut query_reader = csv::Reader::from_path(query).unwrap();
    let query_rows = query_reader
        .records()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(query_rows.len(), 1);
    assert_eq!(query_rows[0].get(1), Some("p0"));
}
