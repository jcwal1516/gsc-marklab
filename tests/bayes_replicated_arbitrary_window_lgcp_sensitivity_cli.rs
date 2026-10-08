#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_lgcp_runs_prespecified_hierarchy_and_kernel_sensitivity_grid() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("sensitivity.json");
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
                    let covariate = ix * 2 - 1;
                    let base = if group_index == 0 { 3 } else { 12 };
                    let count = base + ix + pattern_index + patient_index * 2;
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},synthetic,q-{node_index},{},{},1,4,{covariate},{count},{:064x},{:064x}",
                        ix as f64 + 0.5,
                        iy as f64 + 0.5,
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
            "replicated-arbitrary-window-lgcp-sensitivity",
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
            "--field-amplitude",
            "0.1",
            "--field-length-scale-um",
            "1",
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
            "16201",
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
            "--maximum-total-draw-node-work",
            "1728000",
            "--material-standardized-shift",
            "0.75",
            "--timeout-seconds",
            "240",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.replicated_arbitrary_window_lgcp_sensitivity"
    );
    let scenarios = result["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 9);
    assert_eq!(result["all_fits_complete"], true, "{result}");
    assert_eq!(result["total_draw_node_work"], 1_728_000);
    assert_eq!(scenarios[0]["name"], "baseline");
    assert_eq!(scenarios[0]["maximum_standardized_shift"], 0.0);
    assert!(scenarios.iter().all(|scenario| {
        scenario["fit"]["backend"]["name"] == "pymc"
            && scenario["fit"]["request_sha256"].as_str().unwrap().len() == 64
            && scenario["fit"]["pattern_posterior_predictive"]
                .as_array()
                .is_some_and(|rows| rows.len() == 16)
            && scenario["maximum_standardized_shift"]
                .as_f64()
                .is_some_and(f64::is_finite)
    }));
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(
        result["claim_status"],
        "experimental_hierarchy_and_kernel_sensitivity"
    );
}
