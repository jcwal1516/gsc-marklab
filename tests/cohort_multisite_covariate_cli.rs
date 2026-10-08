#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn multisite_covariate_cli_pools_adjusted_within_site_patient_effects() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patients.csv");
    let mut rows = String::from("patient_id,site_id,group,endpoint,covariate,value\n");
    let noise = [-1.0, 0.2, 0.8, -0.4];
    for (site_index, site) in ["site-a", "site-b", "site-c"].into_iter().enumerate() {
        for group in ["A", "B"] {
            for (index, noise) in noise.into_iter().enumerate() {
                let batch = (index % 2) as f64;
                let endpoint = site_index as f64
                    + 2.0 * index as f64
                    + 0.5 * batch
                    + noise
                    + if group == "A" { 3.0 } else { 0.0 };
                let patient = format!("{site}-{group}-{}", index + 1);
                rows.push_str(&format!(
                    "{patient},{site},{group},{endpoint},age,{index}\n"
                ));
                rows.push_str(&format!(
                    "{patient},{site},{group},{endpoint},batch,{batch}\n"
                ));
            }
        }
    }
    fs::write(&input, rows).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "multisite-covariate-contrast",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--model",
            "fixed-effect",
            "--alpha",
            "0.05",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.cohort_multisite_covariate_contrast"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["population_unit"], "patient");
    assert_eq!(
        result["design"]["site_effect"],
        "adjusted_group_a_indicator"
    );
    assert_eq!(
        result["covariates"]["names"],
        serde_json::json!(["age", "batch"])
    );
    assert_eq!(result["sites"].as_array().expect("sites").len(), 3);
    for site in result["sites"].as_array().expect("sites") {
        assert!((site["effect"].as_f64().expect("effect") - 3.0).abs() < 1e-12);
        assert_eq!(site["group_a_patients"], 4);
        assert_eq!(site["group_b_patients"], 4);
        assert_eq!(site["residual_degrees_of_freedom"], 4);
    }
    assert!((result["pooled"]["pooled_effect"].as_f64().expect("pooled") - 3.0).abs() < 1e-12);
    assert_eq!(result["pooled"]["total_patient_count"], 24);
}
