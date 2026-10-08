#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn gamma_one_birth_death_chain_matches_poisson_count_scale_and_repeats() {
    let directory = tempfile::tempdir().expect("tempdir");
    let first = directory.path().join("gibbs-first.json");
    let second = directory.path().join("gibbs-second.json");
    run(&first);
    run(&second);
    let first_bytes = fs::read(&first).expect("first Gibbs result");
    assert_eq!(first_bytes, fs::read(&second).expect("second Gibbs result"));
    let result: serde_json::Value =
        serde_json::from_slice(&first_bytes).expect("Gibbs result JSON");
    assert_eq!(result["format"], "marklab.strauss_birth_death_simulation");
    assert_eq!(result["gamma"], 1.0);
    assert_eq!(
        result["diagnostics"]["poisson_special_case_expected_count"],
        100.0
    );
    let mean = result["diagnostics"]["post_burn_mean_count"]
        .as_f64()
        .unwrap();
    assert!((mean - 100.0).abs() < 20.0);
    assert!(result["diagnostics"]["accepted_births"].as_u64().unwrap() > 0);
    assert!(result["diagnostics"]["accepted_deaths"].as_u64().unwrap() > 0);
    assert!(
        result["diagnostics"]["maximum_observed_count"]
            .as_u64()
            .unwrap()
            <= 10_000
    );
    let trace = result["count_trace"].as_array().expect("count trace");
    assert_eq!(trace.len(), 20_001);
    let points = result["final_pattern"].as_array().expect("final pattern");
    assert_eq!(
        points.len() as u64,
        result["diagnostics"]["final_count"].as_u64().unwrap()
    );
    for (index, point) in points.iter().enumerate() {
        assert_eq!(point["point_id"], format!("point:{index}"));
        assert!((0.0..100.0).contains(&point["x_um"].as_f64().unwrap()));
        assert!((0.0..100.0).contains(&point["y_um"].as_f64().unwrap()));
    }
    assert_eq!(
        result["claim_status"],
        "experimental_finite_chain_simulation"
    );
}

fn run(output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "simulate-strauss-birth-death",
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "100",
            "--ymax-um",
            "100",
            "--beta-per-um2",
            "0.01",
            "--gamma",
            "1",
            "--interaction-radius-um",
            "5",
            "--iterations",
            "20000",
            "--burn-in",
            "5000",
            "--seed",
            "19101",
            "--maximum-points",
            "10000",
            "--maximum-neighbor-visits",
            "100000000",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
