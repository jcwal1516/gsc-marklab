#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn replicated_multitype_lgcp_runs_the_fixed_hierarchy_and_kernel_grid() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let pymc = directory.path().join("pymc.json");
    let sensitivity = directory.path().join("sensitivity.json");
    fs::write(&input, input_csv()).unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args(["bayes", "fit-replicated-arbitrary-window-multitype-lgcp"])
        .args(common(&input))
        .args(["--out", pymc.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "replicated-arbitrary-window-multitype-lgcp-sensitivity",
            "--pymc-result",
            pymc.to_str().unwrap(),
        ])
        .args(common(&input))
        .args([
            "--numpyro-maximum-tree-depth",
            "12",
            "--maximum-processes",
            "4",
            "--maximum-total-draw-node-type-work",
            "6144000",
            "--materiality-standard-deviations",
            "0.75",
            "--out",
            sensitivity.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(sensitivity).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.replicated_arbitrary_window_multitype_lgcp_prior_kernel_sensitivity"
    );
    assert_eq!(result["fit_state"], "nonconverged");
    assert_eq!(result["material_sensitivity"], false);
    let scenarios = result["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 8);
    assert_eq!(
        scenarios
            .iter()
            .map(|row| row["scenario"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "patient_scale_half",
            "patient_scale_double",
            "pattern_scale_half",
            "pattern_scale_double",
            "field_amplitude_half",
            "field_amplitude_double",
            "field_length_half",
            "field_length_double",
        ]
    );
    for scenario in scenarios {
        if scenario["scenario"] == "field_length_double" {
            assert_eq!(scenario["fit"]["fit_state"], "nonconverged");
            assert_eq!(scenario["fit"]["diagnostics"]["divergences"], 1);
        } else {
            assert_eq!(scenario["fit"]["fit_state"], "complete", "{scenario}");
        }
        let a = scenario["fit"]["type_posteriors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["type_id"] == "A")
            .unwrap();
        assert!(a["group_effect"]["interval_lower"].as_f64().unwrap() > 0.0);
    }
    assert_eq!(result["statistical_unit"], "patient");
}

fn common(input: &Path) -> Vec<&str> {
    vec![
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
        "--field-amplitude",
        "0.05",
        "--field-length-scale-um",
        "1",
        "--jitter",
        "0.000001",
        "--chains",
        "2",
        "--tune",
        "2000",
        "--draws",
        "2000",
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
        "768000",
        "--timeout-seconds",
        "300",
    ]
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
                    let covariate = ix * 2 - 1;
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
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,4,{covariate},{count},{:064x},{:064x}",
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
    }
    csv
}
