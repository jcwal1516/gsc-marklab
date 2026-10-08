#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn thomas_process_is_seeded_bounded_and_conserves_offspring() {
    let directory = tempfile::tempdir().expect("tempdir");
    let first = directory.path().join("thomas-first.json");
    let second = directory.path().join("thomas-second.json");
    run(&first);
    run(&second);
    let first_bytes = fs::read(&first).expect("first Thomas result");
    assert_eq!(
        first_bytes,
        fs::read(&second).expect("second Thomas result")
    );
    let result: serde_json::Value =
        serde_json::from_slice(&first_bytes).expect("Thomas result JSON");
    assert_eq!(result["format"], "marklab.thomas_process_simulation");
    assert_eq!(result["coordinate_unit"], "micrometer");
    assert_eq!(result["seed"], 17101);
    let truncation = &result["truncation"];
    assert_eq!(truncation["sigma_multiple"], 6.0);
    assert!((truncation["radius_um"].as_f64().unwrap() - 30.0).abs() <= 1e-12);
    let expected_area = 10_000.0 + 2.0 * 30.0 * 200.0 + std::f64::consts::PI * 30.0_f64.powi(2);
    assert!((truncation["expanded_area_um2"].as_f64().unwrap() - expected_area).abs() <= 1e-9);
    assert!(
        truncation["radial_tail_probability_bound"]
            .as_f64()
            .unwrap()
            < 2e-8
    );

    let parents = result["parents"].as_array().expect("parents");
    let offspring = result["offspring"].as_array().expect("offspring");
    assert!(!parents.is_empty());
    assert!(!offspring.is_empty());
    let radius = truncation["radius_um"].as_f64().unwrap();
    let parent_generated = parents
        .iter()
        .map(|parent| {
            let x = parent["x_um"].as_f64().unwrap();
            let y = parent["y_um"].as_f64().unwrap();
            let dx = if x < 0.0 {
                -x
            } else if x > 100.0 {
                x - 100.0
            } else {
                0.0
            };
            let dy = if y < 0.0 {
                -y
            } else if y > 100.0 {
                y - 100.0
            } else {
                0.0
            };
            assert!(dx.hypot(dy) <= radius);
            parent["generated_offspring"].as_u64().unwrap()
        })
        .sum::<u64>();
    for (index, child) in offspring.iter().enumerate() {
        assert_eq!(child["offspring_id"], format!("offspring:{index}"));
        assert!(child["parent_index"].as_u64().unwrap() < parents.len() as u64);
        assert!((0.0..100.0).contains(&child["x_um"].as_f64().unwrap()));
        assert!((0.0..100.0).contains(&child["y_um"].as_f64().unwrap()));
    }
    let counts = &result["counts"];
    assert_eq!(
        counts["generated_parents"].as_u64().unwrap(),
        parents.len() as u64
    );
    assert_eq!(
        counts["generated_offspring"].as_u64().unwrap(),
        parent_generated
    );
    assert_eq!(
        counts["retained_offspring"].as_u64().unwrap(),
        offspring.len() as u64
    );
    assert_eq!(
        counts["generated_offspring"].as_u64().unwrap(),
        counts["retained_offspring"].as_u64().unwrap()
            + counts["discarded_offspring"].as_u64().unwrap()
    );
}

fn run(output: &std::path::Path) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "simulate-thomas-process",
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "100",
            "--ymax-um",
            "100",
            "--kappa-parent-per-um2",
            "0.005",
            "--mu-offspring",
            "4",
            "--sigma-um",
            "5",
            "--seed",
            "17101",
            "--maximum-parents",
            "100000",
            "--maximum-offspring",
            "100000",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}
