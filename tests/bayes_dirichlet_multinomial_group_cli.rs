#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn dirichlet_multinomial_recovers_patient_unit_multiclass_group_shift() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_classes.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("patient_id,group,class_id,count\n");
    for index in 0..8 {
        for (class_id, count) in [
            ("Neoplastic", 68 + index),
            ("Inflammatory", 22 - index / 2),
            ("Connective", 10 - index / 2),
        ] {
            writeln!(csv, "mss-{index},MSS,{class_id},{count}").expect("row");
        }
    }
    for index in 0..8 {
        for (class_id, count) in [
            ("Neoplastic", 38 - index / 2),
            ("Inflammatory", 42 + index),
            ("Connective", 20 - index / 2),
        ] {
            writeln!(csv, "msi-{index},MSI,{class_id},{count}").expect("row");
        }
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "dirichlet-multinomial-group",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--logit-prior-sd",
            "1.5",
            "--group-effect-prior-sd",
            "1.0",
            "--concentration-prior-sd",
            "50",
            "--chains",
            "2",
            "--tune",
            "1200",
            "--draws",
            "1600",
            "--target-accept",
            "0.95",
            "--seed",
            "20260828",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_dirichlet_multinomial_group"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["model"]["biological_unit"], "patient");
    assert_eq!(result["input"]["patient_count"], 16);
    assert_eq!(result["input"]["class_count"], 3);
    let classes = result["posterior"]["classes"]
        .as_array()
        .expect("class summaries");
    let neoplastic = classes
        .iter()
        .find(|class| class["class_id"] == "Neoplastic")
        .expect("Neoplastic");
    let inflammatory = classes
        .iter()
        .find(|class| class["class_id"] == "Inflammatory")
        .expect("Inflammatory");
    let predictive = result["posterior_predictive"]["classes"]
        .as_array()
        .expect("predictive class summaries");
    let neoplastic_predictive = predictive
        .iter()
        .find(|class| class["class_id"] == "Neoplastic")
        .expect("Neoplastic predictive");
    let inflammatory_predictive = predictive
        .iter()
        .find(|class| class["class_id"] == "Inflammatory")
        .expect("Inflammatory predictive");
    let reference_neoplastic = (0..8)
        .map(|index| {
            let counts = [68 + index, 22 - index / 2, 10 - index / 2];
            counts[0] as f64 / counts.iter().sum::<usize>() as f64
        })
        .sum::<f64>()
        / 8.0;
    let comparison_neoplastic = (0..8)
        .map(|index| {
            let counts = [38 - index / 2, 42 + index, 20 - index / 2];
            counts[0] as f64 / counts.iter().sum::<usize>() as f64
        })
        .sum::<f64>()
        / 8.0;
    let reference_inflammatory = (0..8)
        .map(|index| {
            let counts = [68 + index, 22 - index / 2, 10 - index / 2];
            counts[1] as f64 / counts.iter().sum::<usize>() as f64
        })
        .sum::<f64>()
        / 8.0;
    let comparison_inflammatory = (0..8)
        .map(|index| {
            let counts = [38 - index / 2, 42 + index, 20 - index / 2];
            counts[1] as f64 / counts.iter().sum::<usize>() as f64
        })
        .sum::<f64>()
        / 8.0;
    for (observed, expected) in [
        (
            neoplastic_predictive["observed_reference_mean_proportion"]
                .as_f64()
                .unwrap(),
            reference_neoplastic,
        ),
        (
            neoplastic_predictive["observed_comparison_mean_proportion"]
                .as_f64()
                .unwrap(),
            comparison_neoplastic,
        ),
        (
            inflammatory_predictive["observed_reference_mean_proportion"]
                .as_f64()
                .unwrap(),
            reference_inflammatory,
        ),
        (
            inflammatory_predictive["observed_comparison_mean_proportion"]
                .as_f64()
                .unwrap(),
            comparison_inflammatory,
        ),
    ] {
        assert!((observed - expected).abs() < 1e-12);
    }
    assert!(
        neoplastic["difference_comparison_minus_reference"]["mean"]
            .as_f64()
            .unwrap()
            < -0.15
    );
    assert!(
        inflammatory["difference_comparison_minus_reference"]["mean"]
            .as_f64()
            .unwrap()
            > 0.15
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_patient_multiclass_group_composition"
    );
}
