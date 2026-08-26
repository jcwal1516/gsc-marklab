#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn nested_grid_strauss_pseudolikelihood_is_bounded_and_finite() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("points.csv");
    let output = directory.path().join("fit.json");
    let mut csv = String::from("point_id,x_um,y_um\n");
    let mut index = 0;
    for iy in 0..4 {
        for ix in 0..4 {
            writeln!(
                csv,
                "p{index},{},{},",
                2.0 + ix as f64 * 5.0,
                2.0 + iy as f64 * 5.0
            )
            .unwrap();
            index += 1;
        }
    }
    // Remove the accidental trailing field separator while preserving exact three-column rows.
    let csv = csv.replace(",\n", "\n");
    fs::write(&input, csv).expect("points");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "fit-strauss-pseudolikelihood",
            "--input",
            input.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "20",
            "--ymax-um",
            "20",
            "--interaction-radius-um",
            "4",
            "--coarse-grid-x",
            "8",
            "--coarse-grid-y",
            "8",
            "--fine-grid-x",
            "16",
            "--fine-grid-y",
            "16",
            "--beta-min-per-um2",
            "0.0001",
            "--beta-max-per-um2",
            "1",
            "--gamma-min",
            "0.001",
            "--gamma-max",
            "1",
            "--maximum-iterations",
            "1000",
            "--maximum-neighbor-visits",
            "1000000",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("pseudolikelihood JSON");
    assert_eq!(result["format"], "marklab.strauss_pseudolikelihood_fit");
    assert_eq!(result["fit_state"], "complete");
    for name in ["coarse_fit", "fine_fit"] {
        let fit = &result[name];
        assert!(fit["beta_per_um2"].as_f64().unwrap() > 0.0);
        assert!((0.001..=1.0).contains(&fit["gamma"].as_f64().unwrap()));
        assert!(fit["model_se_log_beta"].as_f64().unwrap() > 0.0);
        assert!(fit["model_se_log_gamma"].as_f64().unwrap() > 0.0);
        assert!(fit["robust_se_log_beta"].as_f64().unwrap() > 0.0);
        assert!(fit["robust_se_log_gamma"].as_f64().unwrap() > 0.0);
        assert!(fit["objective"].as_f64().unwrap() < fit["initial_objective"].as_f64().unwrap());
        assert!((fit["weight_sum_um2"].as_f64().unwrap() - 400.0).abs() <= 1e-9);
    }
    assert!(
        result["refinement"]["absolute_beta_difference"]
            .as_f64()
            .unwrap()
            >= 0.0
    );
    assert!(
        result["refinement"]["absolute_gamma_difference"]
            .as_f64()
            .unwrap()
            >= 0.0
    );
    assert_eq!(result["claim_status"], "experimental_pseudolikelihood");
}
