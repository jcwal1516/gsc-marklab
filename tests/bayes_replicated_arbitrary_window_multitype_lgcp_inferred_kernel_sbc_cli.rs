#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn multitype_exact_window_sbc_retains_five_scenarios_ranks_and_spatial_ppc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("sbc.json");
    fs::write(&input, input_csv()).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-sbc",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
            "--reference-type",
            "A",
            "--intercept-prior-mean",
            "0.5",
            "--intercept-prior-sd",
            "0.4",
            "--group-effect-prior-sd",
            "0.3",
            "--covariate-effect-prior-sd",
            "0.2",
            "--patient-sd-prior-scale",
            "0.15",
            "--pattern-sd-prior-scale",
            "0.15",
            "--field-amplitude-prior-scale",
            "0.15",
            "--field-length-scale-prior-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--replicates-per-scenario",
            "1",
            "--chains",
            "2",
            "--tune",
            "100",
            "--draws",
            "100",
            "--target-accept",
            "0.9",
            "--seed",
            "20260831",
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
            "38400",
            "--maximum-kernel-cube-work",
            "1024",
            "--maximum-tree-depth",
            "10",
            "--maximum-simulated-node-type-work",
            "960",
            "--maximum-total-iterations",
            "2000",
            "--maximum-ppc-pair-work",
            "4096000",
            "--maximum-working-bytes",
            "536870912",
            "--timeout-seconds",
            "600",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc"
    );
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["scenario_dispositions"].as_array().unwrap().len(), 5);
    assert!(
        result["scenario_dispositions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["status"] == "complete")
            .count()
            >= 4,
        "{result}"
    );
    let scenarios = result["scenario_dispositions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["scenario"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        scenarios,
        [
            "positive",
            "null",
            "weak_identification",
            "boundary",
            "misspecified"
        ]
    );
    assert_eq!(
        result["scenario_dispositions"][1]["truth"]["group_effect_difference"],
        0.0
    );
    for row in result["scenario_dispositions"].as_array().unwrap() {
        assert!(row["status"] == "complete" || row["status"] == "failed");
        if row["status"] == "complete" {
            assert!(row["ranks"]["field_length_scale_um"].as_u64().unwrap() <= 200);
            for diagnostic in [
                "counts",
                "cross_type_enrichment",
                "mark_proportions",
                "clustering",
            ] {
                assert!(
                    row["posterior_predictive"][diagnostic]["two_sided_tail_probability"]
                        .as_f64()
                        .unwrap()
                        .is_finite()
                );
            }
        } else {
            assert!(!row["failure_reason"].as_str().unwrap().is_empty());
        }
    }
    assert_eq!(
        result["cross_type_dependence_estimand"],
        "not_estimated_current_model_uses_conditionally_independent_type_fields"
    );
    assert_eq!(
        result["pattern_representation"],
        "complete_quadrature_resolved_counting_measure_on_exact_window"
    );
    assert_eq!(result["backend"]["name"], "numpyro");
    assert_eq!(
        result["claim_status"],
        "experimental_multitype_sbc_diagnostic"
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
                        writeln!(
                            csv,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,4,{},{},{:064x},{:064x}",
                            ix as f64 + 0.5,
                            iy as f64 + 0.5,
                            ix as i32 * 2 - 1,
                            2 + type_index + node_index % 2,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        ).unwrap();
                    }
                }
            }
        }
    }
    csv
}
