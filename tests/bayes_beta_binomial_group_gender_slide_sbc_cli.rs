#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn repeated_slide_hierarchy_sbc_has_complete_rank_coverage_and_disposition() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("shape.csv");
    let output = directory.path().join("sbc.json");
    let mut csv = String::from("slide_id,patient_id,group,gender,successes,trials\n");
    for (group, gender) in [
        ("MSS", "Male"),
        ("MSS", "Female"),
        ("MSI", "Male"),
        ("MSI", "Female"),
    ] {
        for patient in 0..4_u64 {
            for slide in 0..2_u64 {
                csv.push_str(&format!(
                    "{group}-{gender}-{patient}-s{slide},{group}-{gender}-{patient},{group},{gender},0,100\n"
                ));
            }
        }
    }
    fs::write(&input, csv).expect("shape");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "beta-binomial-group-gender-slide-hierarchy-sbc",
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
            "--replicates",
            "20",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "3000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260827",
            "--minimum-rank-uniformity-p-value",
            "0.001",
            "--minimum-coverage-90",
            "0.7",
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
        "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy_sbc"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["replicates"].as_array().unwrap().len(), 20);
    assert!(result["failures"].as_array().unwrap().is_empty());
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let mut worker_identity =
        b"marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_identity.v1\0"
            .to_vec();
    worker_identity.extend(
        fs::read(
            worker_directory
                .join("marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker.py"),
        )
        .unwrap(),
    );
    worker_identity.push(0);
    worker_identity.extend(
        fs::read(
            worker_directory
                .join("marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_worker.py"),
        )
        .unwrap(),
    );
    assert_eq!(
        result["backend"]["worker_sha256"],
        marklab_bayes::sha256_hex(&worker_identity)
    );
    for parameter in [
        "intercept_log_odds",
        "group_log_odds_effect",
        "gender_log_odds_effect",
        "patient_log_odds_sd",
        "slide_concentration",
        "marginal_probability_difference",
        "patient_probability_0",
        "patient_random_effect_0",
    ] {
        let diagnostic = &result["diagnostics"][parameter];
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
        assert!((0.7..=1.0).contains(&diagnostic["coverage_90"].as_f64().unwrap()));
    }
}
