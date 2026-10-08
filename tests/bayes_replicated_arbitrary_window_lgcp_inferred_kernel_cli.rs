#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_lgcp_infers_shared_physical_kernel_without_losing_patient_unit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from(
        "pattern_id,patient_id,group,cohort,node_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
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
                    let base = if group_index == 0 { 3 } else { 12 };
                    let count = base + ix + pattern_index + patient_index * 2;
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},synthetic,q-{node_index},{},{},1,4,{},{count},{:064x},{:064x}",
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
    fs::write(&input, csv).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-replicated-arbitrary-window-lgcp-inferred-kernel",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
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
            "16211",
            "--maximum-patients",
            "8",
            "--maximum-patterns",
            "16",
            "--maximum-nodes-per-pattern",
            "4",
            "--maximum-total-nodes",
            "64",
            "--maximum-total-events",
            "1000",
            "--maximum-draw-node-work",
            "192000",
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
        "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["patient_count"], 8);
    assert_eq!(result["pattern_count"], 16);
    assert_eq!(result["total_node_count"], 64);
    assert!(
        result["posterior"]["group_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert!(
        result["posterior"]["field_amplitude"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["posterior"]["field_length_scale_um"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["nodes"].as_array().unwrap().len(), 64);
    assert_eq!(
        result["pattern_posterior_predictive"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(result["resources"]["maximum_tree_depth"], 13);
    assert_eq!(
        result["claim_status"],
        "experimental_inferred_shared_kernel"
    );
}
