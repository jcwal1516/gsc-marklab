#![cfg(feature = "cli")]

use std::{fs, process::Command};

use approx::assert_abs_diff_eq;
use serde_json::{json, Value};
use tempfile::tempdir;

fn request(maximum_directed_pair_visits: usize) -> Value {
    json!({
        "format": "marklab.categorical_cooccurrence",
        "version": 1,
        "coordinate_frame_id": "fixture-local-um",
        "coordinate_unit": "micrometer",
        "window": {
            "type": "MultiPolygon",
            "coordinates": [[[[-1.0, -1.0], [3.0, -1.0], [3.0, 1.0], [-1.0, 1.0], [-1.0, -1.0]]]]
        },
        "channel": {
            "id": "cell_type",
            "label": "Cell type",
            "kind": "categorical",
            "unit": "categorical",
            "measurement_status": "classified",
            "provenance": "fixed hand-computed co-occurrence fixture",
            "classes": ["a", "b", "c"]
        },
        "cells": [
            {"id": "a-zero", "x_um": 0.0, "y_um": 0.0, "class_id": "a"},
            {"id": "b-zero", "x_um": 0.0, "y_um": 0.0, "class_id": "b"},
            {"id": "a-one", "x_um": 1.0, "y_um": 0.0, "class_id": "a"},
            {"id": "c-boundary", "x_um": 3.0, "y_um": 0.0, "class_id": "c"}
        ],
        "radii_um": [0.5, 2.0],
        "limits": {
            "maximum_input_bytes": 16384,
            "maximum_points": 4,
            "maximum_classes": 3,
            "maximum_radii": 2,
            "maximum_output_matrix_bytes": 16384,
            "maximum_directed_pair_visits": maximum_directed_pair_visits,
            "maximum_retained_bytes": 1048576
        }
    })
}

fn entry<'a>(matrix: &'a Value, source: &str, target: &str) -> &'a Value {
    matrix["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|entry| entry["source_class_id"] == source && entry["target_class_id"] == target)
        .expect("class-pair entry")
}

#[test]
fn duplicate_coordinates_and_inclusive_radii_produce_the_known_matrices() {
    let directory = tempdir().expect("temporary directory");
    let input = directory.path().join("request.json");
    let output = directory.path().join("result.json");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&request(8)).expect("request JSON"),
    )
    .expect("request");

    let status = Command::new(env!("CARGO_BIN_EXE_marklab"))
        .args([
            "numerics",
            "categorical-co-occurrence",
            "--input",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .status()
        .expect("run categorical co-occurrence");
    assert!(status.success());

    let result: Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.categorical_cooccurrence_result");
    assert_eq!(result["version"], 1);
    assert_eq!(result["coordinate_frame_id"], "fixture-local-um");
    assert_eq!(result["channel_id"], "cell_type");
    assert_eq!(result["classes"], json!(["a", "b", "c"]));
    assert_eq!(result["point_count"], 4);
    assert_eq!(result["class_count"], 3);
    assert_eq!(result["maximum_radius_directed_pair_visits"], 8);

    let matrices = result["matrices"].as_array().expect("matrices");
    assert_eq!(matrices.len(), 2);
    assert_eq!(matrices[0]["radius_um"], 0.5);
    assert_eq!(matrices[0]["total_directed_pairs"], 2);
    assert_eq!(matrices[1]["radius_um"], 2.0);
    assert_eq!(matrices[1]["total_directed_pairs"], 8);

    let small_ab = entry(&matrices[0], "a", "b");
    assert_eq!(small_ab["directed_pair_count"], 1);
    assert_eq!(small_ab["source_edge_incidence"], 1);
    assert_eq!(small_ab["target_edge_incidence"], 1);
    assert_eq!(small_ab["status"], "available");
    assert_eq!(small_ab["score"], 2.0);
    let small_aa = entry(&matrices[0], "a", "a");
    assert_eq!(small_aa["directed_pair_count"], 0);
    assert_eq!(small_aa["status"], "available");
    assert_eq!(small_aa["score"], 0.0);
    let small_ac = entry(&matrices[0], "a", "c");
    assert_eq!(small_ac["directed_pair_count"], 0);
    assert_eq!(small_ac["source_edge_incidence"], 1);
    assert_eq!(small_ac["target_edge_incidence"], 0);
    assert_eq!(small_ac["status"], "unavailable_zero_edge_incidence");
    assert_eq!(small_ac["score"], Value::Null);

    let expected_counts = [[2_u64, 2, 1], [2, 0, 0], [1, 0, 0]];
    let expected_incidences = [5_u64, 2, 1];
    let classes = ["a", "b", "c"];
    for (source_index, source) in classes.iter().enumerate() {
        for (target_index, target) in classes.iter().enumerate() {
            let observed = entry(&matrices[1], source, target);
            assert_eq!(
                observed["directed_pair_count"],
                expected_counts[source_index][target_index]
            );
            assert_eq!(
                observed["source_edge_incidence"],
                expected_incidences[source_index]
            );
            assert_eq!(
                observed["target_edge_incidence"],
                expected_incidences[target_index]
            );
            assert_eq!(observed["status"], "available");
            let expected_score = expected_counts[source_index][target_index] as f64 * 8.0
                / (expected_incidences[source_index] * expected_incidences[target_index]) as f64;
            assert_abs_diff_eq!(
                observed["score"].as_f64().expect("available score"),
                expected_score,
                epsilon = 1e-15
            );
        }
    }
}

#[test]
fn maximum_radius_directed_pair_limit_fails_before_writing_output() {
    let directory = tempdir().expect("temporary directory");
    let input = directory.path().join("request.json");
    let output = directory.path().join("result.json");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&request(7)).expect("request JSON"),
    )
    .expect("request");

    let status = Command::new(env!("CARGO_BIN_EXE_marklab"))
        .args([
            "numerics",
            "categorical-co-occurrence",
            "--input",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .status()
        .expect("run categorical co-occurrence");
    assert!(!status.success());
    assert!(!output.exists());
}
