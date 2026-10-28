#![cfg(feature = "cli")]

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

/// Preprocess a text spectrum of 2,000–2,009 Da into five 2-Da bins, with a peak at 2,006 Da
/// when `peak` is set and isolate-specific noise in the first bin.
fn write_spectrum(dir: &Path, id: &str, index: usize, peak: bool) {
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
    let config = dir.join("preprocess.json");
    if !config.exists() {
        let binning = json!({"steps": [], "binning": {"min_mz": 2000, "max_mz": 2010, "bins": 5}});
        fs::write(&config, binning.to_string()).unwrap();
    }
    let out = dir.join("spectra").join(format!("{id}.json"));
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

fn ast_config() -> Value {
    json!({
        "isolate_column": "code",
        "species_column": "species",
        "species": "Escherichia coli",
        "antibiotic_column": "Ceftriaxone",
        "resistant": ["R", "I"],
        "susceptible": ["S"],
        "model": {
            "c_values": [0.1, 1.0, 10.0],
            "class_weight": "balanced",
            "folds": 2,
            "inner_folds": 2,
            "seed": 3
        }
    })
}

fn cross_validate(dir: &Path, labels: &str, config: &Value) -> assert_cmd::assert::Assert {
    let table = dir.join("labels.csv");
    fs::write(&table, labels).unwrap();
    let config_path = dir.join("ast.json");
    fs::write(&config_path, config.to_string()).unwrap();
    marklab(&[
        "microbio",
        "cross-validate-ast",
        "--labels",
        path(&table),
        "--spectra",
        path(&dir.join("spectra")),
        "--config",
        path(&config_path),
        "--out",
        path(&dir.join("result.json")),
    ])
}

#[test]
fn cross_validates_resistance_from_preprocessed_spectra() {
    let dir = tempfile::tempdir().unwrap();
    let mut labels = String::from("species,code,Ceftriaxone\n");
    for index in 0..16 {
        let id = format!("ec-{index:02}");
        let category = match index % 2 {
            _ if index == 15 => "I",
            0 => "S",
            _ => "R",
        };
        write_spectrum(dir.path(), &id, index, category != "S");
        labels.push_str(&format!("Escherichia coli,{id},{category}\n"));
    }
    // Other species and missing categories are counted and skipped; no spectra exist for them.
    labels.push_str("Klebsiella pneumoniae,kp-01,R\nKlebsiella pneumoniae,kp-02,S\n");
    labels.push_str("Escherichia coli,ec-untested,\n");

    cross_validate(dir.path(), &labels, &ast_config()).success();

    let result: Value =
        serde_json::from_slice(&fs::read(dir.path().join("result.json")).unwrap()).unwrap();
    assert_eq!(
        result["labels"],
        json!({
            "rows": 19,
            "other_species": 2,
            "missing_category": 1,
            "isolates": 16,
            "resistant": 8,
            "susceptible": 8,
            "groups": 16
        })
    );
    assert_eq!(
        result["features"],
        json!({"min_mz": 2000.0, "max_mz": 2010.0, "bins": 5})
    );
    assert_eq!(result["folds"].as_array().unwrap().len(), 2);
    assert_eq!(result["mean_fold_auroc"], json!(1.0));
    let predictions = result["predictions"].as_array().unwrap();
    assert_eq!(predictions.len(), 16);
    assert_eq!(predictions[15]["isolate"], json!("ec-15"));
    assert_eq!(predictions[15]["resistant"], json!(true));
    assert!(predictions[15]["probability_resistant"].as_f64().unwrap() > 0.5);
}

#[test]
fn rejects_unusable_label_tables_and_spectra_without_writing_a_result() {
    let header = "species,code,Ceftriaxone\n";
    let cases = [
        (
            "Escherichia coli,ec-01,X\n",
            "`Ceftriaxone` value `X` is listed as neither resistant nor susceptible",
        ),
        (
            "Escherichia coli,../ec-01,R\n",
            "isolate ID `../ec-01` must be 1 to 128 letters",
        ),
        ("Escherichia coli,ec-02,R\n", "ec-02.json"),
        ("Escherichia coli,unbinned,R\n", "no binned intensities"),
    ];
    for (rows, message) in cases {
        let dir = tempfile::tempdir().unwrap();
        let spectra = dir.path().join("spectra");
        fs::create_dir_all(&spectra).unwrap();
        fs::write(spectra.join("unbinned.json"), r#"{"binned": null}"#).unwrap();
        cross_validate(dir.path(), &format!("{header}{rows}"), &ast_config())
            .failure()
            .stderr(predicate::str::contains(message));
        assert!(!dir.path().join("result.json").exists());
    }
}
