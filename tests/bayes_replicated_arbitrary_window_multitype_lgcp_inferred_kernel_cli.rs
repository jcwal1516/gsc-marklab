#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_multitype_lgcp_infers_one_shared_physical_kernel_at_the_patient_unit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("fit.json");
    fs::write(&input, input_csv()).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-replicated-arbitrary-window-multitype-lgcp-inferred-kernel",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
            "--reference-type",
            "A",
            "--intercept-prior-mean",
            "1",
            "--intercept-prior-sd",
            "1",
            "--group-effect-prior-sd",
            "1",
            "--covariate-effect-prior-sd",
            "1",
            "--patient-sd-prior-scale",
            "0.5",
            "--pattern-sd-prior-scale",
            "0.5",
            "--field-amplitude-prior-scale",
            "0.5",
            "--field-length-scale-prior-scale-um",
            "2",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "1500",
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
            "5000",
            "--maximum-draw-node-type-work",
            "576000",
            "--maximum-kernel-cube-work",
            "1024",
            "--maximum-tree-depth",
            "13",
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
        "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["patient_count"], 8);
    assert_eq!(result["pattern_count"], 16);
    assert_eq!(result["type_count"], 3);
    assert_eq!(result["total_node_count"], 64);
    assert_eq!(result["total_node_type_row_count"], 192);
    assert!(
        result["kernel_posterior"]["field_amplitude"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["kernel_posterior"]["field_length_scale_um"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    let a = result["type_posteriors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["type_id"] == "A")
        .unwrap();
    assert!(a["group_effect"]["mean"].as_f64().unwrap() > 0.5);
    assert_eq!(
        result["node_type_posteriors"].as_array().unwrap().len(),
        192
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_inferred_shared_multitype_kernel"
    );
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
                    let ix = node_index % 2;
                    let iy = node_index / 2;
                    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
                        let base = match (group_index, type_index) {
                            (0, 0) => 3,
                            (1, 0) => 12,
                            (_, 1) => 6,
                            _ => 4,
                        };
                        let count = base + ix + pattern_index + patient_index % 2;
                        writeln!(
                            csv,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,4,{},{count},{:064x},{:064x}",
                            ix as f64 + 0.5,
                            iy as f64 + 0.5,
                            ix * 2 - 1,
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
