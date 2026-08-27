#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn slide_hierarchy_recovers_group_gender_and_patient_variation() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("slides.csv");
    let output = directory.path().join("result.json");
    let mut csv = String::from("slide_id,patient_id,group,gender,successes,trials\n");
    for (group, gender, baseline) in [
        ("MSS", "Male", 36_u64),
        ("MSS", "Female", 68),
        ("MSI", "Male", 98),
        ("MSI", "Female", 130),
    ] {
        for patient in 0..4_u64 {
            let patient_successes = baseline + patient * 3;
            for (slide, offset) in [(0, 0_u64), (1, 4)] {
                csv.push_str(&format!(
                    "{group}-{gender}-{patient}-s{slide},{group}-{gender}-{patient},{group},{gender},{},200\n",
                    patient_successes + offset
                ));
            }
        }
    }
    fs::write(&input, csv).expect("input");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-group-gender-slide-hierarchy",
            "--input",
            input.to_str().unwrap(),
            "--reference-group",
            "MSS",
            "--comparison-group",
            "MSI",
            "--reference-gender",
            "Male",
            "--comparison-gender",
            "Female",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "2",
            "--group-effect-prior-sd",
            "1",
            "--gender-effect-prior-sd",
            "1",
            "--patient-log-odds-sd-prior-sd",
            "1",
            "--slide-concentration-prior-sd",
            "20",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "4000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
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
        "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["patients"].as_array().unwrap().len(), 16);
    assert_eq!(result["slides"].as_array().unwrap().len(), 32);
    assert!(
        result["posterior"]["group_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.8
    );
    assert!(
        result["posterior"]["gender_log_odds_effect"]["mean"]
            .as_f64()
            .unwrap()
            > 0.4
    );
    assert!(
        result["posterior"]["patient_log_odds_sd"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["posterior"]["slide_concentration"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(
        result["posterior_predictive"]["observed_total_successes"],
        2864
    );
}
