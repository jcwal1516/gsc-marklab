#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn dirichlet_multinomial_group_sbc_calibrates_every_free_and_composition_parameter() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("shape.csv");
    let output = directory.path().join("sbc.json");
    let mut csv = String::from("patient_id,group,class_id,count\n");
    for (group, prefix) in [("MSS", "mss"), ("MSI", "msi")] {
        for patient in 0..4 {
            for (class_id, count) in [("Neoplastic", 34), ("Inflammatory", 33), ("Connective", 33)]
            {
                writeln!(csv, "{prefix}-{patient},{group},{class_id},{count}").expect("row");
            }
        }
    }
    fs::write(&input, csv).expect("shape");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "dirichlet-multinomial-group-sbc",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--logit-prior-sd",
            "1.5",
            "--group-effect-prior-sd",
            "1",
            "--concentration-prior-sd",
            "20",
            "--replicates",
            "20",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1600",
            "--target-accept",
            "0.97",
            "--seed",
            "20260828",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.55",
            "--maximum-coverage-90",
            "1",
            "--timeout-seconds",
            "600",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_dirichlet_multinomial_group_sbc"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert!(result["failures"].as_array().unwrap().is_empty());
    assert_eq!(
        result["diagnostics"]["baseline_logits"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        result["diagnostics"]["group_log_ratio_effects"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        result["diagnostics"]["class_probability_differences"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    for diagnostic in result["diagnostics"]["baseline_logits"]
        .as_array()
        .unwrap()
        .iter()
        .chain(
            result["diagnostics"]["group_log_ratio_effects"]
                .as_array()
                .unwrap(),
        )
        .chain(
            result["diagnostics"]["class_probability_differences"]
                .as_array()
                .unwrap(),
        )
        .chain(std::iter::once(&result["diagnostics"]["concentration"]))
    {
        assert_eq!(
            diagnostic["rank_histogram"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap())
                .sum::<u64>(),
            20
        );
        assert!(diagnostic["rank_uniformity_p_value"].as_f64().unwrap() >= 0.001);
        assert!((0.55..=1.0).contains(&diagnostic["coverage_90"].as_f64().unwrap()));
    }
    assert_eq!(
        result["claim_status"],
        "experimental_simulation_calibration"
    );
}
