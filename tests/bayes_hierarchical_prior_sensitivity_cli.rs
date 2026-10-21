#![cfg(feature = "cli")]

use std::{collections::HashSet, fs};

use assert_cmd::Command;

#[test]
fn patient_hierarchy_reports_one_at_a_time_prior_sensitivity() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "patient_id,observation\n\
p-1,0.2\n\
p-1,0.4\n\
p-1,0.6\n\
p-1,0.8\n\
p-2,0.8\n\
p-2,1.0\n\
p-2,1.2\n\
p-2,1.4\n\
p-3,1.4\n\
p-3,1.6\n\
p-3,1.8\n\
p-3,2.0\n\
p-4,2.0\n\
p-4,2.2\n\
p-4,2.4\n\
p-4,2.6\n\
p-5,2.6\n\
p-5,2.8\n\
p-5,3.0\n\
p-5,3.2\n\
p-6,3.2\n\
p-6,3.4\n\
p-6,3.6\n\
p-6,3.8\n",
    )
    .expect("fixture");
    let output = directory.path().join("sensitivity.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "hierarchical-normal-prior-sensitivity",
            "--input",
            input.to_str().expect("input path"),
            "--global-prior-mean",
            "0",
            "--global-prior-sd",
            "5",
            "--between-patient-sd-prior",
            "2",
            "--known-sigma",
            "1",
            "--lower-scale-multiplier",
            "0.75",
            "--upper-scale-multiplier",
            "1.5",
            "--material-standardized-shift",
            "0.5",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "2000",
            "--target-accept",
            "0.99",
            "--seed",
            "20260825",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_hierarchical_prior_sensitivity"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["base_scenario"], "baseline");
    let scenarios = result["scenarios"].as_array().expect("scenarios");
    assert_eq!(scenarios.len(), 5);
    assert_eq!(result["material_standardized_shift"], 0.5);
    let baseline = &scenarios[0]["posterior"];
    let mut requests = HashSet::new();
    let mut material = Vec::new();
    for (scenario, (name, global_sd, between_sd)) in scenarios.iter().zip([
        ("baseline", 5.0, 2.0),
        ("between_patient_sd_lower", 5.0, 1.5),
        ("between_patient_sd_upper", 5.0, 3.0),
        ("global_sd_lower", 3.75, 2.0),
        ("global_sd_upper", 7.5, 2.0),
    ]) {
        assert_eq!(scenario["scenario_id"], name);
        assert_eq!(scenario["global_prior_sd"], global_sd);
        assert_eq!(scenario["between_patient_sd_prior"], between_sd);
        assert_eq!(scenario["fit_state"], "complete");
        assert_eq!(scenario["diagnostics"]["divergences"], 0);
        // Existing request identities must differ: relabeling one baseline fit is insufficient.
        assert!(requests.insert(scenario["request_sha256"].as_str().unwrap()));
        let mut maximum = 0.0_f64;
        for (parameter, field) in [
            ("global_mean", "global_mean_shift_standardized"),
            (
                "between_patient_sd",
                "between_patient_sd_shift_standardized",
            ),
        ] {
            let scale = baseline[parameter]["sd"].as_f64().unwrap();
            assert!(scale > 0.0);
            let shift = (scenario["posterior"][parameter]["mean"].as_f64().unwrap()
                - baseline[parameter]["mean"].as_f64().unwrap())
                / scale;
            assert!(
                (scenario[field].as_f64().unwrap() - shift).abs() < 1e-12,
                "{name}, {parameter}"
            );
            maximum = maximum.max(shift.abs());
        }
        assert!(
            (scenario["maximum_absolute_standardized_shift"]
                .as_f64()
                .unwrap()
                - maximum)
                .abs()
                < 1e-12
        );
        assert_eq!(scenario["material_shift"], maximum >= 0.5);
        if maximum >= 0.5 {
            material.push(name);
        }
    }
    assert_eq!(
        result["material_shift_scenarios"],
        serde_json::json!(material)
    );
    assert_eq!(
        result["sensitivity_state"],
        if material.is_empty() {
            "stable_within_declared_prior_grid"
        } else {
            "sensitive_within_declared_prior_grid"
        }
    );
    assert_eq!(result["claim_status"], "experimental_prior_sensitivity");
}
