#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_patient_lgcp_sbc_calibrates_population_hierarchy_and_field() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("sbc.json");
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
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},synthetic,q-{node_index},{},{},1,4,{},2,{:064x},{:064x}",
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
            "replicated-arbitrary-window-lgcp-sbc",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
            "--intercept-prior-mean",
            "1",
            "--intercept-prior-sd",
            "0.5",
            "--group-effect-prior-sd",
            "0.5",
            "--covariate-effect-prior-sd",
            "0.5",
            "--patient-sd-prior-scale",
            "0.3",
            "--pattern-sd-prior-scale",
            "0.3",
            "--field-amplitude",
            "0.1",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--replicates",
            "20",
            "--chains",
            "4",
            "--tune",
            "750",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "20260829",
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
            "256000",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.65",
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
        "marklab.replicated_arbitrary_window_lgcp_sbc"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert!(
        result["failures"].as_array().unwrap().is_empty(),
        "{result}"
    );
    assert_eq!(result["physical_latent_pattern_id"], "comparison-p0-s0");
    assert_eq!(result["physical_latent_node_id"], "q-0");
    assert_eq!(result["backend"]["name"], "numpyro");
    for parameter in ["group_effect", "patient_sd", "pattern_sd", "latent_node"] {
        assert_eq!(
            result["diagnostics"][parameter]["rank_histogram"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap())
                .sum::<u64>(),
            20
        );
    }
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(
        result["claim_status"],
        "experimental_simulation_calibration"
    );
}
