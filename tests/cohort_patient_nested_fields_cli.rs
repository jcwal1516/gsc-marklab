#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

fn write_input(path: &Path) {
    let mut rows = String::from("patient_id,specimen_id,group,endpoint,value\n");
    for (patient, group, first, second) in [
        ("a1", "A", 2.0, 0.0),
        ("a2", "A", 1.8, 0.2),
        ("b1", "B", 0.0, 2.0),
        ("b2", "B", 0.2, 1.8),
    ] {
        for (slide, offset) in [("s1", -0.05), ("s2", 0.05)] {
            rows.push_str(&format!(
                "{patient},{patient}-{slide},{group},local_mean_abs,{:.17}\n",
                first + offset
            ));
            rows.push_str(&format!(
                "{patient},{patient}-{slide},{group},spde_factor_sd,{:.17}\n",
                second - offset
            ));
        }
    }
    fs::write(path, rows).expect("write nested field input");
}

#[test]
fn nested_field_inference_keeps_slides_inside_patients() {
    let root = tempfile::tempdir().expect("root");
    let input = root.path().join("fields.csv");
    let output = root.path().join("result.json");
    write_input(&input);

    Command::cargo_bin("marklab")
        .expect("marklab")
        .args([
            "cohort",
            "patient-nested-fields",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "31",
            "--seed",
            "73",
            "--alpha",
            "0.05",
            "--maximum-patients",
            "4",
            "--maximum-specimens",
            "8",
            "--maximum-endpoints",
            "2",
            "--maximum-permutation-endpoint-evaluations",
            "256",
            "--memory-budget-mib",
            "8",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result bytes")).expect("result JSON");
    assert_eq!(result["format"], "marklab.cohort_patient_nested_fields");
    assert_eq!(result["version"], 1);
    assert_eq!(result["population_unit"], "patient");
    assert_eq!(result["specimen_unit"], "specimen_nested_within_patient");
    assert_eq!(result["patient_count"], 4);
    assert_eq!(result["specimen_count"], 8);
    assert_eq!(result["endpoint_count"], 2);
    assert_eq!(result["patient_endpoints"].as_array().unwrap().len(), 4);
    assert_eq!(result["heldout"]["split_unit"], "whole_patient");
    assert_eq!(
        result["heldout"]["training_transform"],
        "fold_internal_z_score"
    );
    assert_eq!(result["heldout"]["balanced_accuracy"], 1.0);
    assert_eq!(result["nested_specimen_stability"]["comparison_count"], 8);
    assert_eq!(
        result["population_inference"]["correction"],
        "step_down_max_t"
    );
    assert_eq!(
        result["population_inference"]["permutation_unit"],
        "whole_patient_label"
    );
    assert_eq!(result["population_inference"]["permutations_completed"], 31);
}

#[test]
fn nested_field_inference_rejects_incomplete_specimen_vectors() {
    let root = tempfile::tempdir().expect("root");
    let input = root.path().join("fields.csv");
    let output = root.path().join("result.json");
    write_input(&input);
    let truncated = fs::read_to_string(&input)
        .expect("input")
        .lines()
        .filter(|line| *line != "b2,b2-s2,B,spde_factor_sd,1.75000000000000000")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&input, format!("{truncated}\n")).expect("truncate input");

    Command::cargo_bin("marklab")
        .expect("marklab")
        .args([
            "cohort",
            "patient-nested-fields",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "31",
            "--seed",
            "73",
            "--alpha",
            "0.05",
            "--maximum-patients",
            "4",
            "--maximum-specimens",
            "8",
            "--maximum-endpoints",
            "2",
            "--maximum-permutation-endpoint-evaluations",
            "256",
            "--memory-budget-mib",
            "8",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "does not contain the exact complete endpoint vector",
        ));
}
