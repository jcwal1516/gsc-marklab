#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_exact_window_lgcp_keeps_patient_and_pattern_as_hierarchy_levels() {
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
            "fit-replicated-arbitrary-window-lgcp",
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
            "--maximum-draw-node-work",
            "192000",
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
        "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["patient_count"], 8);
    assert_eq!(result["pattern_count"], 16);
    assert_eq!(result["total_node_count"], 64);
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["pattern_unit"], "slide_pattern_nested_in_patient");
    assert!(
        result["posterior"]["group_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.5
    );
    assert!(result["posterior"]["patient_sd"]["mean"].as_f64().unwrap() > 0.0);
    assert!(result["posterior"]["pattern_sd"]["mean"].as_f64().unwrap() > 0.0);
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["assumptions"][0],
        "patients_are_independent_population_units"
    );
    assert_eq!(
        result["claim_status"],
        "experimental_replicated_pattern_hierarchy"
    );
}
