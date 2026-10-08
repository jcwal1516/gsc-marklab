#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn conditional_multitype_mark_fit_recovers_known_segregation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("points.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("point_id,x_um,y_um,type_id\n");
    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
        for point_index in 0..30 {
            let x = type_index as f64 * 100.0 + (point_index % 6) as f64;
            let y = (point_index / 6) as f64;
            writeln!(csv, "{type_id}-{point_index:02},{x},{y},{type_id}").unwrap();
        }
    }
    fs::write(&input, csv).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-conditional-multitype-mark",
            "--input",
            input.to_str().unwrap(),
            "--reference-type",
            "A",
            "--radius-um",
            "1.5",
            "--intercept-prior-sd",
            "2",
            "--interaction-prior-sd",
            "1",
            "--chains",
            "2",
            "--tune",
            "750",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260829",
            "--maximum-points",
            "100",
            "--maximum-types",
            "4",
            "--maximum-neighbor-visits",
            "10000",
            "--maximum-edges",
            "1000",
            "--maximum-draw-parameter-work",
            "40000",
            "--maximum-working-bytes",
            "1048576",
            "--maximum-tree-depth",
            "12",
            "--timeout-seconds",
            "300",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.conditional_multitype_mark_fit");
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["point_count"], 90);
    assert_eq!(result["type_count"], 3);
    assert_eq!(result["statistical_unit"], "one_fixed_location_pattern");
    assert_eq!(
        result["null_model"],
        "fixed_location_independent_categorical_labels_intercept_only"
    );
    assert!(
        result["comparison"]["mean_composite_log_score_improvement"]
            .as_f64()
            .unwrap()
            > 20.0,
        "{result}"
    );
    let affinities = result["pair_affinity_contrasts"].as_array().unwrap();
    assert_eq!(affinities.len(), 3);
    for row in affinities {
        assert!(
            row["contrast"]["interval_upper"].as_f64().unwrap() < 0.0,
            "{row}"
        );
    }
    assert_eq!(
        result["claim_status"],
        "experimental_conditional_mark_pseudolikelihood"
    );
}
