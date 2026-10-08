#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn five_positive_pairs_match_exact_hidden_bias_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let sets = (0..5)
        .map(|index| {
            serde_json::json!({
                "set_id": format!("s{index}"),
                "observations": [
                    {"unit_id": format!("t{index}"), "treatment": true, "outcome": 1.0},
                    {"unit_id": format!("c{index}"), "treatment": false, "outcome": 0.0}
                ]
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "matched_sets": sets,
            "gamma_grid": [1.0, 2.0, 4.0],
            "alpha": 0.05,
            "maximum_set_gamma_evaluations": 100
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("output.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "causal",
            "rosenbaum-sign-sensitivity",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.rosenbaum_matched_pair_sign_sensitivity"
    );
    assert_eq!(result["positive_differences"], 5);
    assert_eq!(result["ties"], 0);
    assert_close(
        result["curve"][0]["upper_p_value"].as_f64().unwrap(),
        0.5_f64.powi(5),
    );
    assert_close(
        result["curve"][1]["lower_p_value"].as_f64().unwrap(),
        (1.0_f64 / 3.0).powi(5),
    );
    assert_close(
        result["curve"][1]["upper_p_value"].as_f64().unwrap(),
        (2.0_f64 / 3.0).powi(5),
    );
    assert_close(
        result["curve"][2]["upper_p_value"].as_f64().unwrap(),
        0.8_f64.powi(5),
    );
    assert_eq!(result["critical_gamma"], 1.0);
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}
