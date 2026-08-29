#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_conditional_marks_use_exact_pattern_state_calibration() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("sbc.json");
    let mut csv = String::from("pattern_id,patient_id,group,point_id,x_um,y_um,type_id\n");
    for group in ["MSS", "MSI"] {
        for patient_index in 0..2 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for point_index in 0..6 {
                    let type_id = ["A", "B", "C"][point_index % 3];
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},{pattern}-{point_index:02},{point_index},0,{type_id}"
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
            "replicated-conditional-multitype-mark-sbc",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--reference-type",
            "A",
            "--radius-um",
            "1.1",
            "--intercept-prior-sd",
            "0.5",
            "--interaction-prior-sd",
            "0.25",
            "--group-effect-prior-sd",
            "0.25",
            "--patient-sd-prior-scale",
            "0.2",
            "--pattern-sd-prior-scale",
            "0.2",
            "--replicates",
            "20",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1500",
            "--target-accept",
            "0.95",
            "--seed",
            "20260829",
            "--maximum-patients",
            "4",
            "--maximum-patterns",
            "8",
            "--maximum-points",
            "48",
            "--maximum-types",
            "3",
            "--maximum-neighbor-visits",
            "120",
            "--maximum-edges",
            "40",
            "--maximum-draw-parameter-work",
            "500000",
            "--maximum-working-bytes",
            "16777216",
            "--maximum-tree-depth",
            "12",
            "--maximum-states-per-pattern",
            "1000",
            "--maximum-enumeration-work",
            "2000000",
            "--maximum-enumeration-bytes",
            "8388608",
            "--maximum-total-iterations",
            "100000",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.5",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "900",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.replicated_conditional_multitype_mark_sbc"
    );
    assert_eq!(result["enumeration_oracle"]["states_per_pattern"], 729);
    assert!(
        (result["enumeration_oracle"]["zero_parameter_log_normalizer_per_pattern"]
            .as_f64()
            .unwrap()
            - 6.0 * 3.0_f64.ln())
        .abs()
            < 1e-12
    );
    assert_eq!(
        result["replicates"].as_array().unwrap().len()
            + result["failures"].as_array().unwrap().len(),
        20
    );
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["diagnostics"].as_array().unwrap().len(), 10);
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(
        result["generative_model"],
        "exact_normalized_finite_state_patient_pattern_hierarchical_gibbs"
    );
}
