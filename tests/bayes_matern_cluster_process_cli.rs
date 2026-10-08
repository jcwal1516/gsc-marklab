#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn matern_cluster_process_is_seeded_exact_window_and_radius_bounded() {
    let directory = tempfile::tempdir().expect("tempdir");
    let first = directory.path().join("matern-first.json");
    let second = directory.path().join("matern-second.json");
    run(&first);
    run(&second);
    let first_bytes = fs::read(&first).expect("first Matérn result");
    assert_eq!(
        first_bytes,
        fs::read(&second).expect("second Matérn result")
    );
    let result: serde_json::Value =
        serde_json::from_slice(&first_bytes).expect("Matérn result JSON");
    assert_eq!(
        result["format"],
        "marklab.matern_cluster_process_simulation"
    );
    assert_eq!(result["coordinate_unit"], "micrometer");
    assert_eq!(result["seed"], 18101);
    let boundary = &result["boundary"];
    assert_eq!(boundary["radius_um"], 10.0);
    assert_eq!(boundary["exact_for_bounded_offspring"], true);
    let expected_area = 10_000.0 + 2.0 * 10.0 * 200.0 + std::f64::consts::PI * 100.0;
    assert!((boundary["expanded_area_um2"].as_f64().unwrap() - expected_area).abs() <= 1e-9);

    let parents = result["parents"].as_array().expect("parents");
    let offspring = result["offspring"].as_array().expect("offspring");
    assert!(!parents.is_empty());
    assert!(!offspring.is_empty());
    let radius = boundary["radius_um"].as_f64().unwrap();
    let generated = parents
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
        let parent = &parents[child["parent_index"].as_u64().unwrap() as usize];
        let x = child["x_um"].as_f64().unwrap();
        let y = child["y_um"].as_f64().unwrap();
        assert!((0.0..100.0).contains(&x));
        assert!((0.0..100.0).contains(&y));
        assert!(
            (x - parent["x_um"].as_f64().unwrap()).hypot(y - parent["y_um"].as_f64().unwrap())
                <= radius * (1.0 + 1e-12)
        );
    }
    let counts = &result["counts"];
    assert_eq!(
        counts["generated_parents"].as_u64().unwrap(),
        parents.len() as u64
    );
    assert_eq!(counts["generated_offspring"].as_u64().unwrap(), generated);
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
            "simulate-matern-cluster-process",
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
            "--radius-um",
            "10",
            "--seed",
            "18101",
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
