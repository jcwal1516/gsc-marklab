#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn pymc_and_numpyro_agree_on_repeated_slide_hierarchy() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("slides.csv");
    let output = directory.path().join("agreement.json");
    let mut csv = String::from("slide_id,patient_id,group,gender,successes,trials\n");
    for (group, gender, baseline) in [
        ("MSS", "Male", 36_u64),
        ("MSS", "Female", 68),
        ("MSI", "Male", 98),
        ("MSI", "Female", 130),
    ] {
        for patient in 0..4_u64 {
            for (slide, offset) in [(0, 0_u64), (1, 4)] {
                csv.push_str(&format!(
                    "{group}-{gender}-{patient}-s{slide},{group}-{gender}-{patient},{group},{gender},{},200\n",
                    baseline + patient * 3 + offset
                ));
            }
        }
    }
    fs::write(&input, csv).expect("fixture");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-group-gender-slide-hierarchy-agreement",
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
            "3000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
            "--maximum-standardized-difference",
            "5",
            "--minimum-probability-tolerance",
            "0.03",
            "--minimum-log-odds-tolerance",
            "0.15",
            "--minimum-patient-sd-tolerance",
            "0.2",
            "--minimum-concentration-tolerance",
            "2",
            "--minimum-patient-probability-tolerance",
            "0.03",
            "--minimum-patient-effect-tolerance",
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
        "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy_cross_backend_agreement"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["agreement_status"], "agree_within_monte_carlo_error");
    for parameter in [
        "group_log_odds_effect",
        "gender_log_odds_effect",
        "patient_log_odds_sd",
        "slide_concentration",
        "marginal_probability_difference_comparison_minus_reference",
    ] {
        assert_eq!(
            result["comparison"]["parameters"][parameter]["passes"], true,
            "{parameter}: {result}"
        );
    }
    assert_eq!(
        result["comparison"]["patient_probabilities"]["passes"], true,
        "{result}"
    );
    assert_eq!(
        result["comparison"]["patient_random_effects"]["passes"], true,
        "{result}"
    );
    assert_eq!(result["pymc"]["diagnostics"]["divergences"], 0);
    assert_eq!(result["numpyro"]["diagnostics"]["divergences"], 0);
}
