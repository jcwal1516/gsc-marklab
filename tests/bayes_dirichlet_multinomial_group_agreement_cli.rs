#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_patient_multiclass_group_composition() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patient_classes.csv");
    let output = directory.path().join("agreement.json");
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
            "dirichlet-multinomial-group-agreement",
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
            "50",
            "--chains",
            "2",
            "--tune",
            "1600",
            "--draws",
            "2400",
            "--target-accept",
            "0.97",
            "--seed",
            "20260828",
            "--maximum-standardized-difference",
            "5",
            "--minimum-probability-tolerance",
            "0.03",
            "--minimum-concentration-tolerance",
            "3",
            "--timeout-seconds",
            "240",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_dirichlet_multinomial_group_cross_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    assert_eq!(result["pymc"]["backend"]["name"], "pymc");
    assert_eq!(result["numpyro"]["backend"]["name"], "numpyro");
    let classes = result["comparison"]["classes"]
        .as_array()
        .expect("class agreements");
    assert_eq!(classes.len(), 3);
    for class in classes {
        for parameter in [
            "reference_probability",
            "comparison_probability",
            "difference_comparison_minus_reference",
        ] {
            assert_eq!(class[parameter]["passes"], true, "{parameter}: {result}");
        }
    }
    assert_eq!(result["comparison"]["concentration"]["passes"], true);
    assert_eq!(result["pymc"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["numpyro"]["diagnostics"]["divergences"], 0);
    assert_eq!(
        result["claim_status"],
        "experimental_cross_backend_validation"
    );
}
