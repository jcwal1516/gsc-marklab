#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn multiresolution_factors_separate_declared_scale_contributions_and_recover_masks() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("multiresolution.json");
    let coarse = (0..16)
        .map(|index| (index as f64 - 7.5) / 4.0)
        .collect::<Vec<_>>();
    let fine = (0..16)
        .map(|index| {
            if (index as usize).is_multiple_of(2) {
                -1.0
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    let rows = (0..16)
        .map(|index| {
            let values = [coarse[index] + 0.35 * fine[index], 1.5 * coarse[index] - 0.25 * fine[index]];
            let observed = (0..2)
                .map(|feature| !matches!((index, feature), (3, 0) | (6, 1) | (11, 0) | (14, 1)))
                .collect::<Vec<_>>();
            serde_json::json!({"entity_id":format!("r{index:02}"),"values":values,"observed":observed})
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "matrix_id":"multiscale_region_features",
        "entity_level":"region",
        "likelihood":"gaussian",
        "feature_names":["morphology","ihc"],
        "rows":rows,
        "scales":[
            {"scale_id":"coarse","physical_scale_um":200.0,"basis_columns":[coarse],"factors":1,"coefficient_precision":0.1},
            {"scale_id":"fine","physical_scale_um":25.0,"basis_columns":[fine],"factors":1,"coefficient_precision":0.1}
        ],
        "loading_precision":0.1,
        "noise_standard_deviation":0.08,
        "maximum_iterations":500,
        "seed":41,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "multiresolution-factor",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.multiresolution_spatial_factor_model"
    );
    assert_eq!(result["scale_contributions"].as_array().unwrap().len(), 2);
    assert!(result["scale_contributions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|scale| { scale["variance_contribution"].as_f64().unwrap() > 0.0 }));
    let fraction_sum = result["scale_contributions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scale| scale["variance_fraction"].as_f64().unwrap())
        .sum::<f64>();
    assert!((fraction_sum - 1.0).abs() < 1e-10);
    assert_eq!(result["masked_predictions"].as_array().unwrap().len(), 4);
    assert!(result["masked_rmse"].as_f64().unwrap() < 0.25);
    assert_eq!(result["fit_state"], "approximate_only");
}
