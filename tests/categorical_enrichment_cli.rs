#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;
use serde_json::{json, Value};

fn request(labels: &[&str], radius_um: f64) -> Value {
    json!({
        "format": "marklab.categorical_enrichment",
        "version": 1,
        "coordinate_frame_id": "tissue-um",
        "coordinate_unit": "micrometer",
        "window": {"type": "MultiPolygon", "coordinates": [[[
            [-1.0, -1.0], [5.0, -1.0], [5.0, 1.0], [-1.0, 1.0], [-1.0, -1.0]
        ]]]},
        "channel": {
            "id": "cell_type", "label": "Cell type", "kind": "categorical",
            "unit": "categorical", "measurement_status": "classified",
            "provenance": "hand-counted path", "classes": ["a", "b"]
        },
        "cells": labels.iter().enumerate().map(|(index, label)| json!({
            "id": format!("cell-{index}"), "x_um": index as f64,
            "y_um": 0.0, "class_id": label
        })).collect::<Vec<_>>(),
        "radius_um": radius_um,
        "permutations": 199,
        "seed": 43,
        "limits": {
            "maximum_input_bytes": 1_048_576,
            "maximum_points": 100,
            "maximum_classes": 8,
            "maximum_edges": 100,
            "maximum_permutations": 999,
            "maximum_permutation_work": 100_000,
            "maximum_retained_bytes": 8_388_608
        }
    })
}

fn run_request(directory: &std::path::Path, name: &str, input: &Value) -> Value {
    let input_path = directory.join(format!("{name}-input.json"));
    let output_path = directory.join(format!("{name}-result.json"));
    fs::write(&input_path, serde_json::to_vec(input).unwrap()).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["numerics", "categorical-enrichment", "--input"])
        .arg(input_path)
        .arg("--out")
        .arg(&output_path)
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output_path).unwrap()).unwrap()
}

fn pair<'a>(result: &'a Value, a: &str, b: &str) -> &'a Value {
    result["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["monte_carlo"]["label_a"] == a && row["monte_carlo"]["label_b"] == b)
        .unwrap()
}

fn close(actual: &Value, expected: f64) {
    let actual = actual.as_f64().unwrap();
    assert!((actual - expected).abs() <= 1e-12, "{actual} != {expected}");
}

#[test]
fn fixed_margin_enrichment_matches_path_oracles_and_deterministic_graphs() {
    let directory = tempfile::tempdir().unwrap();
    let input = request(&["a", "a", "b", "b"], 1.0);
    let result = run_request(directory.path(), "path-four", &input);
    assert_eq!(result["format"], "marklab.categorical_enrichment_result");
    assert_eq!(result["version"], 1);
    assert_eq!(result["undirected_edge_count"], 3);
    assert_eq!(result["adjacent_edge_pair_count"], 2);
    assert_eq!(result["disjoint_edge_pair_count"], 1);
    let cross = pair(&result, "a", "b");
    assert_eq!(cross["monte_carlo"]["observed_edges"], 1);
    close(&cross["analytic_null_mean"], 2.0);
    close(&cross["analytic_null_variance"], 2.0 / 3.0);
    close(&cross["analytic_z_score"], -(1.5_f64).sqrt());
    let same = pair(&result, "a", "a");
    close(&same["analytic_null_mean"], 0.5);
    close(&same["analytic_null_variance"], 0.25);
    close(&same["analytic_z_score"], 1.0);
    assert_eq!(result, run_request(directory.path(), "replay", &input));

    let three = run_request(
        directory.path(),
        "path-three",
        &request(&["a", "b", "b"], 1.0),
    );
    close(&pair(&three, "a", "b")["analytic_null_mean"], 4.0 / 3.0);
    close(&pair(&three, "a", "b")["analytic_null_variance"], 2.0 / 9.0);

    for (name, labels, radius) in [
        ("complete", vec!["a", "a", "b", "b"], 4.0),
        ("two", vec!["a", "b"], 1.0),
        ("no-edges", vec!["a", "b", "b"], 0.5),
    ] {
        let deterministic = run_request(directory.path(), name, &request(&labels, radius));
        for row in deterministic["pairs"].as_array().unwrap() {
            close(&row["analytic_null_variance"], 0.0);
            assert!(row["analytic_z_score"].is_null());
            assert_eq!(
                row["analytic_z_score_unavailable_reason"],
                "zero_null_variance"
            );
            assert_eq!(
                row["monte_carlo"]["z_score_unavailable_reason"],
                "zero_null_variance"
            );
            close(&row["monte_carlo"]["p_value"], 1.0);
        }
    }
}

#[test]
fn enrichment_rejects_excess_work_before_publishing_and_preserves_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("input.json");
    let output_path = directory.path().join("result.json");
    let mut input = request(&["a", "a", "b", "b"], 1.0);
    input["limits"]["maximum_permutation_work"] = json!(1392);
    fs::write(&input_path, serde_json::to_vec(&input).unwrap()).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["numerics", "categorical-enrichment", "--input"])
        .arg(&input_path)
        .arg("--out")
        .arg(&output_path)
        .assert()
        .failure();
    assert!(!output_path.exists());

    fs::write(
        &input_path,
        serde_json::to_vec(&request(&["a", "b"], 1.0)).unwrap(),
    )
    .unwrap();
    fs::write(&output_path, b"existing result").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["numerics", "categorical-enrichment", "--input"])
        .arg(input_path)
        .arg("--out")
        .arg(&output_path)
        .assert()
        .failure();
    assert_eq!(fs::read(output_path).unwrap(), b"existing result");
}
