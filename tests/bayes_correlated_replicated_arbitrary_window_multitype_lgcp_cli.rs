#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn correlated_multitype_lgcp_exposes_identified_cross_type_covariance_and_spatial_ppc() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("fit.json");
    fs::write(&input, input_csv()).unwrap();

    command(&input, &output).assert().success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.correlated_replicated_arbitrary_window_multitype_lgcp"
    );
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["type_count"], 3);
    assert_eq!(
        result["cross_type_correlations"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        result["correlation_parameterization"],
        "lower_triangular_lkj_cholesky_with_positive_marginal_field_scales"
    );
    assert!(
        result["coregionalization_oracle"]["maximum_absolute_error"]
            .as_f64()
            .unwrap()
            < 1e-12
    );
    assert_eq!(
        result["pattern_type_posterior_predictive"]
            .as_array()
            .unwrap()
            .len(),
        48
    );
    let correlations = result["cross_type_correlations"].as_array().unwrap();
    for row in correlations {
        let summary = &row["correlation"];
        assert!((-1.0..=1.0).contains(&summary["mean"].as_f64().unwrap()));
        assert!((-1.0..=1.0).contains(&summary["interval_lower"].as_f64().unwrap()));
        assert!((-1.0..=1.0).contains(&summary["interval_upper"].as_f64().unwrap()));
    }
    let mean_for = |left: &str, right: &str| {
        correlations
            .iter()
            .find(|row| row["type_a"] == left && row["type_b"] == right)
            .unwrap()["correlation"]["mean"]
            .as_f64()
            .unwrap()
    };
    assert!(mean_for("A", "B") > 0.1, "{correlations:?}");
    assert!(mean_for("A", "C") < -0.1, "{correlations:?}");
    let matrix = result["posterior_mean_correlation_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(matrix.len(), 9);
    for index in 0..3 {
        assert!((matrix[index * 3 + index] - 1.0).abs() < 1e-12);
        for other in 0..3 {
            assert!((matrix[index * 3 + other] - matrix[other * 3 + index]).abs() < 1e-12);
        }
    }
    let determinant = matrix[0] * (matrix[4] * matrix[8] - matrix[5] * matrix[7])
        - matrix[1] * (matrix[3] * matrix[8] - matrix[5] * matrix[6])
        + matrix[2] * (matrix[3] * matrix[7] - matrix[4] * matrix[6]);
    assert!(determinant > 0.0, "{matrix:?}");
    assert_eq!(result["backend"]["name"], "numpyro");
    assert!(matches!(
        result["fit_state"].as_str().unwrap(),
        "complete" | "nonconverged"
    ));
}

fn command(input: &std::path::Path, output: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "bayes",
        "fit-correlated-replicated-arbitrary-window-multitype-lgcp",
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "reference",
        "--comparison-group",
        "comparison",
        "--reference-type",
        "A",
        "--intercept-prior-mean",
        "2",
        "--intercept-prior-sd",
        "1",
        "--group-effect-prior-sd",
        "0.3",
        "--covariate-effect-prior-sd",
        "0.2",
        "--patient-sd-prior-scale",
        "0.15",
        "--pattern-sd-prior-scale",
        "0.15",
        "--field-amplitude-prior-scale",
        "1.5",
        "--field-length-scale-prior-scale-um",
        "1.5",
        "--lkj-concentration",
        "2",
        "--jitter",
        "0.000001",
        "--chains",
        "2",
        "--tune",
        "100",
        "--draws",
        "100",
        "--target-accept",
        "0.9",
        "--seed",
        "20260831",
        "--maximum-patients",
        "8",
        "--maximum-patterns",
        "16",
        "--maximum-types",
        "3",
        "--maximum-nodes-per-pattern",
        "9",
        "--maximum-total-nodes",
        "144",
        "--maximum-total-node-type-rows",
        "432",
        "--maximum-total-events",
        "5000",
        "--maximum-draw-node-type-work",
        "86400",
        "--maximum-kernel-cube-work",
        "11664",
        "--maximum-coregionalization-work",
        "500000",
        "--maximum-tree-depth",
        "10",
        "--timeout-seconds",
        "600",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}

fn input_csv() -> String {
    let mut csv = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..9 {
                    let ix = node_index % 3;
                    let iy = node_index / 3;
                    let high = if ix == iy { 20 } else { 2 };
                    let low = if ix == iy { 2 } else { 12 };
                    for (type_index, (type_id, count)) in [("A", high), ("B", high - 1), ("C", low)]
                        .into_iter()
                        .enumerate()
                    {
                        writeln!(
                            csv,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,9,{},{},{:064x},{:064x}",
                            ix as f64 + 0.5,
                            iy as f64 + 0.5,
                            0,
                            count + group_index + type_index,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    csv
}
