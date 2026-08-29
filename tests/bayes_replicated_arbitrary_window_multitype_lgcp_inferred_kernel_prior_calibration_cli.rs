#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn inferred_multitype_kernel_prior_generator_matches_analytic_oracles() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("calibration.json");
    fs::write(&input, input_csv()).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-prior-calibration",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
            "--reference-type",
            "A",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "0.25",
            "--group-effect-prior-sd",
            "0.25",
            "--covariate-effect-prior-sd",
            "0.25",
            "--patient-sd-prior-scale",
            "0.1",
            "--pattern-sd-prior-scale",
            "0.1",
            "--field-amplitude-prior-scale",
            "0.2",
            "--field-length-scale-prior-scale-um",
            "2",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "500",
            "--draws",
            "500",
            "--target-accept",
            "0.95",
            "--seed",
            "20260829",
            "--maximum-patients",
            "8",
            "--maximum-patterns",
            "16",
            "--maximum-types",
            "3",
            "--maximum-nodes-per-pattern",
            "4",
            "--maximum-total-nodes",
            "64",
            "--maximum-total-node-type-rows",
            "192",
            "--maximum-total-events",
            "1000",
            "--maximum-draw-node-type-work",
            "192000",
            "--maximum-kernel-cube-work",
            "1024",
            "--maximum-tree-depth",
            "13",
            "--timeout-seconds",
            "300",
            "--replicates",
            "4096",
            "--maximum-simulation-node-type-work",
            "786432",
            "--maximum-kernel-simulation-work",
            "4194304",
            "--maximum-working-bytes",
            "536870912",
            "--maximum-mean-error-sd",
            "0.06",
            "--maximum-relative-sd-error",
            "0.06",
            "--maximum-field-whitened-moment-error",
            "0.06",
            "--maximum-poisson-residual-mean",
            "0.03",
            "--maximum-poisson-residual-second-moment-error",
            "0.08",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["moment_checks"].as_array().unwrap().len(), 7);
    assert!(result["moment_checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["passes"] == true));
    assert_eq!(result["field_oracle"]["passes"], true);
    assert_eq!(result["poisson_oracle"]["passes"], true);
    assert_eq!(result["statistical_unit"], "patient");
}

fn input_csv() -> String {
    let mut csv = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..4 {
                    for type_id in ["A", "B", "C"] {
                        writeln!(
                            csv,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},0,1,4,{},1,{:064x},{:064x}",
                            node_index as f64 + 0.5,
                            node_index * 2 - 3,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    csv
}
