#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_replicated_lgcp_inferred_kernel() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("agreement.json");
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
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},synthetic,q-{node_index},{},{},1,4,{},{},{:064x},{:064x}",
                        ix as f64 + 0.5,
                        iy as f64 + 0.5,
                        ix * 2 - 1,
                        base + ix + pattern_index + patient_index * 2,
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
            "replicated-arbitrary-window-lgcp-inferred-kernel-agreement",
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
            "--pymc-maximum-tree-depth",
            "13",
            "--numpyro-maximum-tree-depth",
            "13",
            "--maximum-standardized-difference",
            "5",
            "--minimum-parameter-tolerance",
            "0.12",
            "--minimum-field-tolerance",
            "0.2",
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
        "marklab.replicated_arbitrary_window_lgcp_inferred_kernel_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    for name in [
        "intercept",
        "group_effect",
        "covariate_effect",
        "patient_sd",
        "pattern_sd",
        "field_amplitude",
        "field_length_scale_um",
        "patient_effects",
        "pattern_effects",
        "latent_effect",
        "expected_count",
    ] {
        assert_eq!(
            result["comparison"][name]["passes"], true,
            "{name}: {result}"
        );
    }
    assert_eq!(result["comparison"]["patient_count"], 8);
    assert_eq!(result["comparison"]["pattern_count"], 16);
    assert_eq!(result["comparison"]["node_count"], 64);
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
