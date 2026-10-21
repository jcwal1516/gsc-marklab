#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

fn run(input: &serde_json::Value, linear_weight: &str, relaxation: &str) -> serde_json::Value {
    let directory = tempfile::tempdir().expect("tempdir");
    let input_path = directory.path().join("input.json");
    fs::write(&input_path, serde_json::to_vec(input).unwrap()).expect("input");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "unbalanced-fused-gromov-wasserstein",
            "--input",
            input_path.to_str().expect("input path"),
            "--linear-weight",
            linear_weight,
            "--marginal-relaxation",
            relaxation,
            "--epsilon",
            "0",
            "--feature-scale",
            "1",
            "--structure-scale",
            "1",
            "--tolerance",
            "1e-12",
            "--maximum-iterations",
            "2000",
            "--inner-tolerance",
            "1e-14",
            "--maximum-inner-iterations",
            "20000",
            "--timeout-seconds",
            "120",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();
    serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON")
}

#[test]
fn identical_spaces_are_matched_one_to_one_with_supplied_masses() {
    let masses = [0.1, 0.2, 0.3, 0.4];
    let coordinates = [[0.0, 0.0], [1.0, 0.0], [0.0, 2.0], [3.0, 1.0]];
    let structure = coordinates
        .iter()
        .flat_map(|a| {
            coordinates
                .iter()
                .map(move |b: &[f64; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt())
        })
        .collect::<Vec<_>>();
    let support = |prefix: &str| {
        masses
            .iter()
            .enumerate()
            .map(|(index, mass)| {
                serde_json::json!({"id": format!("{prefix}{index}"), "mass": mass, "features": [index as f64]})
            })
            .collect::<Vec<_>>()
    };
    let result = run(
        &serde_json::json!({
            "source": support("s"), "target": support("t"),
            "source_structure_row_major": structure, "target_structure_row_major": structure
        }),
        "1",
        "1",
    );
    assert_eq!(
        result["format"],
        "marklab.unbalanced_fused_gromov_wasserstein"
    );
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["solver"], "kl_majorization_minimization");
    for entry in result["best_plan"].as_array().unwrap() {
        let row = entry["source_id"].as_str().unwrap()[1..]
            .parse::<usize>()
            .unwrap();
        let column = entry["target_id"].as_str().unwrap()[1..]
            .parse::<usize>()
            .unwrap();
        let expected = if row == column { masses[row] } else { 0.0 };
        assert!(
            (entry["mass"].as_f64().unwrap() - expected).abs() < 1e-6,
            "{entry}"
        );
    }
    assert!(result["best_objective"]["total"].as_f64().unwrap().abs() < 1e-6);
}

#[test]
fn unequal_totals_relax_marginals_to_the_analytic_single_point_mass() {
    // One point each: P = Q = p solves 4 rho p log(p^2 / (u v)) = 0 when the feature cost is
    // zero, so p = sqrt(u v) = sqrt(0.6 * 1.5).
    let result = run(
        &serde_json::json!({
            "source": [{"id": "s", "mass": 0.6, "features": [0.0]}],
            "target": [{"id": "t", "mass": 1.5, "features": [0.0]}],
            "source_structure_row_major": [0.0], "target_structure_row_major": [0.0]
        }),
        "1",
        "0.5",
    );
    let expected = (0.6_f64 * 1.5).sqrt();
    assert!((result["transported_mass"].as_f64().unwrap() - expected).abs() < 1e-6);
    assert!(
        (result["source_marginal_minus_mass"][0].as_f64().unwrap() - (expected - 0.6)).abs() < 1e-6
    );
    assert!(
        (result["target_marginal_minus_mass"][0].as_f64().unwrap() - (expected - 1.5)).abs() < 1e-6
    );
}
