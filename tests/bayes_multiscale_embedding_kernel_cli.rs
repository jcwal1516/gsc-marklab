#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn multiscale_kernel_matches_weighted_linear_hand_oracle_and_sensitivity() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("summaries.csv");
    let weights = directory.path().join("weights.csv");
    fs::write(
        &input,
        "sample_id,scale_um,embedding_0,embedding_1\na,10,1,0\na,20,0,2\nb,10,3,0\nb,20,0,1\n",
    )
    .unwrap();
    for (run, scale_weights, expected_total) in [
        (0, "10,0.25\n20,0.75\n", 2.25),
        (1, "10,1\n20,1e-16\n", 3.0),
    ] {
        let output = directory.path().join(format!("output-{run}.json"));
        fs::write(&weights, format!("scale_um,weight\n{scale_weights}")).unwrap();
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "multiscale-embedding-kernel",
                "--input",
                input.to_str().unwrap(),
                "--weights",
                weights.to_str().unwrap(),
                "--sample-a",
                "a",
                "--sample-b",
                "b",
                "--base-kernel",
                "linear",
                "--maximum-component-scale-visits",
                "4",
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();

        let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(result["format"], "marklab.multiscale_embedding_kernel");
        assert_eq!(result["version"], 1);
        assert_eq!(result["base_kernel"], "linear");
        assert_eq!(result["total"], expected_total);
        assert_eq!(result["scales"][0]["raw_kernel"], 3.0);
        assert_eq!(result["scales"][1]["raw_kernel"], 2.0);
        if run == 0 {
            assert_eq!(result["scales"][0]["contribution"], 0.75);
            assert_eq!(result["scales"][1]["contribution"], 1.5);
        }
        assert_eq!(
            result["drop_one_scale_sensitivity"][0]["renormalized_total"],
            2.0
        );
        assert_eq!(
            result["drop_one_scale_sensitivity"][1]["renormalized_total"],
            3.0
        );
    }
}

#[test]
fn multiscale_cosine_is_scale_invariant_and_rejects_zero_vectors() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("summaries.csv");
    let weights = directory.path().join("weights.csv");
    fs::write(&weights, "scale_um,weight\n10,0.5\n20,0.5\n").unwrap();
    for (run, left, right, expected) in [
        (0, [1e100, 0.0], [1e100, 0.0], Some(1.0)),
        (1, [1e308, 1e308], [-1e308, -1e308], Some(-1.0)),
        (2, [1e-200, 0.0], [1e-200, 0.0], Some(1.0)),
        (3, [1e100, 0.0], [0.0, 1e100], Some(0.0)),
        (4, [0.0, 0.0], [1.0, 0.0], None),
    ] {
        let output = directory.path().join(format!("output-{run}.json"));
        fs::write(
            &input,
            format!(
                "sample_id,scale_um,embedding_0,embedding_1\na,10,{},{}\na,20,{},{}\nb,10,{},{}\nb,20,{},{}\n",
                left[0], left[1], left[0], left[1], right[0], right[1], right[0], right[1],
            ),
        )
        .unwrap();
        let mut command = Command::cargo_bin("marklab").unwrap();
        command.args([
            "bayes",
            "multiscale-embedding-kernel",
            "--input",
            input.to_str().unwrap(),
            "--weights",
            weights.to_str().unwrap(),
            "--sample-a",
            "a",
            "--sample-b",
            "b",
            "--base-kernel",
            "cosine",
            "--maximum-component-scale-visits",
            "4",
            "--out",
            output.to_str().unwrap(),
        ]);
        if let Some(expected) = expected {
            command.assert().success();
            let result: serde_json::Value =
                serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
            assert!((result["total"].as_f64().unwrap() - expected).abs() < 1e-12);
            for scale in result["scales"].as_array().unwrap() {
                assert!((scale["raw_kernel"].as_f64().unwrap() - expected).abs() < 1e-12);
            }
            if run == 1 {
                let project = directory.path().join("project");
                for replay in 0..2 {
                    let output = directory.path().join(format!("project-{replay}.json"));
                    Command::cargo_bin("marklab")
                        .unwrap()
                        .args([
                            "project",
                            "multiscale-embedding-kernel",
                            "--project",
                            project.to_str().unwrap(),
                            "--input",
                            input.to_str().unwrap(),
                            "--weights",
                            weights.to_str().unwrap(),
                            "--sample-a",
                            "a",
                            "--sample-b",
                            "b",
                            "--base-kernel",
                            "cosine",
                            "--maximum-summaries",
                            "4",
                            "--maximum-dimension",
                            "2",
                            "--maximum-component-scale-visits",
                            "4",
                            "--memory-budget-mib",
                            "8",
                            "--out",
                            output.to_str().unwrap(),
                        ])
                        .assert()
                        .success()
                        .stderr(predicates::str::contains(if replay == 0 {
                            "cache_status=miss"
                        } else {
                            "cache_status=hit"
                        }));
                    let project_result: serde_json::Value =
                        serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
                    assert_eq!(project_result, result);
                }
            }
        } else {
            let result = command.assert().failure().get_output().clone();
            assert!(String::from_utf8(result.stderr)
                .unwrap()
                .contains("zero norm"));
            assert!(!output.exists());
        }
    }
}
