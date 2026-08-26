#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn permutation_equivariant_flow_and_diffusion_generate_supported_diverse_point_sets() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("point_sets.json");
    let patterns = (0..16)
        .map(|patient| {
            let context = if (patient as usize).is_multiple_of(2) {
                -1.0
            } else {
                1.0
            };
            let points = (0..4)
                .rev()
                .map(|index| {
                    let center = if context < 0.0 { 0.3 } else { 0.7 };
                    [
                        center + 0.04 * (index as f64 - 1.5),
                        0.25 + 0.15 * index as f64 + 0.01 * (patient % 3) as f64,
                    ]
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "pattern_id":format!("p{patient:02}"),
                "patient_id":format!("p{patient:02}"),
                "split":if patient < 12 {"train"} else {"test"},
                "window":[0.0,1.0,0.0,1.0],
                "context":[context],
                "points":points
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "patterns":patterns,
        "flow":{"family":"conditional_logistic_normal_iid_equivariant","boundary_epsilon":0.0001,"variance_floor":0.01},
        "diffusion":{"schedule":"variance_preserving_linear_beta","steps":64,"beta_min":0.1,"beta_max":8.0},
        "generated_patterns_per_context":16,
        "seed":79,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "neural",
            "point-set-generators",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.point_set_flow_and_diffusion");
    assert!(
        result["flow"]["heldout_log_likelihood"].as_f64().unwrap()
            > result["flow"]["uniform_poisson_log_likelihood"]
                .as_f64()
                .unwrap()
    );
    assert!(
        result["flow"]["permutation_log_likelihood_error"]
            .as_f64()
            .unwrap()
            < 1e-12
    );
    assert!(result["diffusion"]["score_matching_loss"].as_f64().unwrap() < 1.0);
    assert_eq!(result["generated"]["all_points_inside_window"], true);
    assert_eq!(result["generated"]["pattern_count"], 32);
    assert!(
        result["generated"]["minimum_pairwise_pattern_distance"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_point_set_generators"
    );
}
