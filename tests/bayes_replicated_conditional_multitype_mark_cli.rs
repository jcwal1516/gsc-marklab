#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_conditional_marks_recover_a_patient_level_interaction_shift() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("pattern_id,patient_id,group,point_id,x_um,y_um,type_id\n");

    for (group_index, group) in ["MSS", "MSI"].into_iter().enumerate() {
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for slide_index in 0..2 {
                let pattern = format!("{patient}-s{slide_index}");
                for point_index in 0..45 {
                    let type_index = point_index % 3;
                    let type_id = ["A", "B", "C"][type_index];
                    let (x, y) = if group_index == 0 {
                        (
                            type_index as f64 * 100.0 + (point_index / 3 % 5) as f64,
                            (point_index / 15) as f64,
                        )
                    } else {
                        (
                            (point_index / 3 % 5) as f64 * 3.0 + type_index as f64,
                            (point_index / 15) as f64,
                        )
                    };
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},{pattern}-{point_index:03},{x},{y},{type_id}"
                    )
                    .unwrap();
                }
            }
        }
    }
    fs::write(&input, csv).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-replicated-conditional-multitype-mark",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--reference-type",
            "A",
            "--radius-um",
            "1.5",
            "--intercept-prior-sd",
            "2",
            "--interaction-prior-sd",
            "1",
            "--group-effect-prior-sd",
            "1",
            "--patient-sd-prior-scale",
            "0.5",
            "--pattern-sd-prior-scale",
            "0.5",
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
            "--maximum-patients",
            "10",
            "--maximum-patterns",
            "20",
            "--maximum-points",
            "1000",
            "--maximum-types",
            "4",
            "--maximum-neighbor-visits",
            "100000",
            "--maximum-edges",
            "10000",
            "--maximum-draw-parameter-work",
            "1000000",
            "--maximum-working-bytes",
            "16777216",
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
    assert_eq!(
        result["format"],
        "marklab.replicated_conditional_multitype_mark_fit"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["patient_count"], 8);
    assert_eq!(result["pattern_count"], 16);
    assert_eq!(result["point_count"], 720);
    assert_eq!(result["type_count"], 3);
    assert_eq!(result["edge_count"], 1_936);
    assert_eq!(result["group_ids"], serde_json::json!(["MSS", "MSI"]));
    assert_eq!(
        result["null_model"],
        "zero_patient_population_group_shift_in_fixed_location_conditional_mark_affinity"
    );
    let shifts = result["group_affinity_shifts"].as_array().unwrap();
    assert_eq!(shifts.len(), 3);
    assert!(
        shifts
            .iter()
            .all(|row| row["contrast"]["interval_lower"].as_f64().unwrap() > 0.0),
        "{shifts:?}"
    );
    for check in result["pattern_checks"].as_array().unwrap() {
        let pattern = check["pattern_id"].as_str().unwrap();
        let expected_same = if pattern.starts_with("MSS-") { 114 } else { 30 };
        assert_eq!(
            check["observed_same_type_edges"].as_u64().unwrap(),
            expected_same,
            "the independent 5x3/15x3 grid edge oracle differs for {pattern}"
        );
    }
    assert_eq!(
        result["claim_status"],
        "experimental_patient_population_conditional_mark_pseudolikelihood"
    );
}
