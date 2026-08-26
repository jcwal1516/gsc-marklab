#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn maturity_only_downgrades_and_terminal_causal_failure_wins() {
    let directory = tempfile::tempdir().unwrap();
    let first = run(
        &directory,
        "soft",
        serde_json::json!({
            "method_maturity": "validated",
            "mode": "approximate",
            "provenance_complete": true,
            "converged": true,
            "severe_diagnostic_failure": false,
            "approximation_error_validated": false,
            "predictive_clinical_claim": true,
            "external_validation_complete": false,
            "causal_claim": false,
            "causal_identification_supported": false,
            "bayesian": true,
            "sbc_complete": false,
            "posterior_predictive_complete": true
        }),
    );
    assert_eq!(first["result_maturity"], "research_only");
    assert_eq!(
        first["reasons"],
        serde_json::json!([
            "approximate_mode_without_validated_error_or_calibration",
            "predictive_clinical_claim_without_external_validation",
            "bayesian_requirements_incomplete"
        ])
    );

    let terminal = run(
        &directory,
        "terminal",
        serde_json::json!({
            "method_maturity": "validated",
            "mode": "exact",
            "provenance_complete": true,
            "converged": true,
            "severe_diagnostic_failure": false,
            "approximation_error_validated": true,
            "predictive_clinical_claim": false,
            "external_validation_complete": false,
            "causal_claim": true,
            "causal_identification_supported": false,
            "bayesian": false,
            "sbc_complete": false,
            "posterior_predictive_complete": false
        }),
    );
    assert_eq!(terminal["result_maturity"], "unsupported_for_claim");
    assert_eq!(
        terminal["reasons"],
        serde_json::json!(["causal_identification_unsupported"])
    );
}

fn run(
    directory: &tempfile::TempDir,
    name: &str,
    input_value: serde_json::Value,
) -> serde_json::Value {
    let input = directory.path().join(format!("{name}.json"));
    let output = directory.path().join(format!("{name}-out.json"));
    fs::write(&input, serde_json::to_vec(&input_value).unwrap()).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "policy",
            "determine-maturity",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}
