#![cfg(all(feature = "cli", feature = "parquet"))]

use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

fn marklab(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args(args)
        .assert()
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Preprocess a text spectrum of 2,000–2,009 Da into `bins` bins of `spectra`, with a peak at
/// 2,006 Da when `peak` is set and isolate-specific noise in the first two points.
fn write_spectrum(dir: &Path, spectra: &str, id: &str, index: usize, peak: bool, bins: usize) {
    let text = dir.join(format!("{id}.txt"));
    let lines: String = (0..10)
        .map(|offset| {
            let wobble = if offset < 2 {
                ((index * 7 + offset * 3) % 5) as f64
            } else {
                0.0
            };
            let intensity = 10.0 + wobble + if peak && offset == 6 { 40.0 } else { 0.0 };
            format!("{} {intensity}\n", 2000 + offset)
        })
        .collect();
    fs::write(&text, lines).unwrap();
    let config = dir.join(format!("preprocess-{bins}.json"));
    let binning = json!({"steps": [], "binning": {"min_mz": 2000, "max_mz": 2010, "bins": bins}});
    fs::write(&config, binning.to_string()).unwrap();
    let out = dir.join(spectra).join(format!("{id}.json"));
    marklab(&[
        "microbio",
        "preprocess",
        "--text",
        path(&text),
        "--config",
        path(&config),
        "--out",
        path(&out),
    ])
    .success();
}

/// Twenty-four labelled E. coli isolates (twelve resistant, one of them `I`) and two unlabelled ones,
/// collected into `table.parquet`; returns the label table.
fn write_isolates(dir: &Path) -> String {
    let mut labels = String::from("species,code,Ceftriaxone\n");
    for index in 0..24 {
        let id = format!("ec-{index:02}");
        let category = match index % 2 {
            _ if index == 23 => "I",
            0 => "S",
            _ => "R",
        };
        write_spectrum(dir, "spectra", &id, index, category != "S", 5);
        labels.push_str(&format!("Escherichia coli,{id},{category}\n"));
    }
    write_spectrum(dir, "spectra", "new-resistant", 24, true, 5);
    write_spectrum(dir, "spectra", "new-susceptible", 25, false, 5);
    marklab(&[
        "microbio",
        "feature-table",
        "--spectra",
        path(&dir.join("spectra")),
        "--out",
        path(&dir.join("table.parquet")),
    ])
    .success();
    labels
}

fn fit_config() -> Value {
    json!({
        "target": {
            "id_column": "code",
            "species_column": "species",
            "species": "Escherichia coli",
            "antibiotic_column": "Ceftriaxone",
            "resistant": ["R", "I"],
            "susceptible": ["S"]
        },
        "model": {
            "c_values": [0.1, 1.0, 10.0],
            "class_weight": "balanced",
            "folds": 2,
            "inner_folds": 2,
            "seed": 3,
            "significance": 0.2
        }
    })
}

fn fit(dir: &Path, labels: &str) -> assert_cmd::assert::Assert {
    let table = dir.join("labels.csv");
    fs::write(&table, labels).unwrap();
    let config = dir.join("fit.json");
    fs::write(&config, fit_config().to_string()).unwrap();
    marklab(&[
        "microbio",
        "fit-ast",
        "--features",
        path(&dir.join("table.parquet")),
        "--labels",
        path(&table),
        "--config",
        path(&config),
        "--model",
        path(&dir.join("model.json")),
        "--report",
        path(&dir.join("report.json")),
    ])
}

fn predict(dir: &Path, table: &str) -> assert_cmd::assert::Assert {
    marklab(&[
        "microbio",
        "predict-ast",
        "--model",
        path(&dir.join("model.json")),
        "--features",
        path(&dir.join(table)),
        "--out",
        path(&dir.join("predictions.json")),
    ])
}

#[test]
fn fits_a_calibrated_model_and_predicts_new_isolates_from_preprocessed_spectra() {
    let dir = tempfile::tempdir().unwrap();
    let mut labels = write_isolates(dir.path());
    // Other species and missing categories are counted and skipped.
    labels.push_str("Klebsiella pneumoniae,kp-01,R\nEscherichia coli,ec-untested,\n");
    fit(dir.path(), &labels).success();

    let report = read_json(&dir.path().join("report.json"));
    assert_eq!(
        report["labels"],
        json!({
            "rows": 26,
            "other_species": 1,
            "missing_category": 1,
            "labelled": 24,
            "resistant": 12,
            "susceptible": 12,
            "groups": 24,
            "unlabelled_feature_rows": 2
        })
    );
    assert_eq!(report["features"]["modality"], json!("maldi_tof"));
    assert_eq!(report["features"]["unit"], json!("isolate"));
    assert_eq!(report["features"]["columns"].as_array().unwrap().len(), 5);
    let evaluation = &report["evaluation"];
    assert_eq!(evaluation["mean_fold_auroc"], json!(1.0));
    assert_eq!(evaluation["predictions"].as_array().unwrap().len(), 24);
    assert_eq!(evaluation["conformal"]["significance"], json!(0.2));
    assert_eq!(report["final_ensemble"]["members"], json!(2));

    predict(dir.path(), "table.parquet").success();
    let output = read_json(&dir.path().join("predictions.json"));
    assert_eq!(output["modality"], json!("maldi_tof"));
    let predictions = output["predictions"].as_array().unwrap();
    assert_eq!(predictions.len(), 26);
    let find = |id: &str| predictions.iter().find(|p| p["id"] == json!(id)).unwrap();
    assert_eq!(find("new-resistant")["call"], json!("resistant"));
    assert_eq!(find("new-susceptible")["call"], json!("susceptible"));
    assert!(
        find("new-resistant")["probability_resistant"]
            .as_f64()
            .unwrap()
            > find("new-susceptible")["probability_resistant"]
                .as_f64()
                .unwrap()
    );
}

#[test]
fn rejects_unusable_labels_tables_and_spectra_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let labels = write_isolates(dir.path());
    for (rows, message) in [
        (
            "Escherichia coli,ec-00,X\n",
            "`Ceftriaxone` value `X` is listed as neither resistant nor susceptible",
        ),
        (
            "Escherichia coli,ec-99,R\n",
            "1 labelled IDs are not in the feature table, such as `ec-99`",
        ),
    ] {
        fit(dir.path(), &format!("{labels}{rows}"))
            .failure()
            .stderr(predicate::str::contains(message));
        assert!(!dir.path().join("model.json").exists());
        assert!(!dir.path().join("report.json").exists());
    }

    fit(dir.path(), &labels).success();
    // Spectra binned differently from the training table are refused.
    write_spectrum(dir.path(), "other-bins", "ec-00", 0, false, 10);
    marklab(&[
        "microbio",
        "feature-table",
        "--spectra",
        path(&dir.path().join("other-bins")),
        "--out",
        path(&dir.path().join("other-bins.parquet")),
    ])
    .success();
    predict(dir.path(), "other-bins.parquet")
        .failure()
        .stderr(predicate::str::contains("provenance"));
    assert!(!dir.path().join("predictions.json").exists());

    let unbinned = dir.path().join("unbinned");
    fs::create_dir_all(&unbinned).unwrap();
    fs::write(unbinned.join("x.json"), r#"{"steps": [], "binned": null}"#).unwrap();
    marklab(&[
        "microbio",
        "feature-table",
        "--spectra",
        path(&unbinned),
        "--out",
        path(&dir.path().join("unbinned.parquet")),
    ])
    .failure()
    .stderr(predicate::str::contains("no binned intensities"));
}
