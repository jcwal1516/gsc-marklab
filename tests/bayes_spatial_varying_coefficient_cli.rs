#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn exact_spatially_varying_coefficient_recovers_smooth_signal() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.csv");
    fs::write(
        &input,
        "coordinate_id,x_um,y,global_x,spatial_z\n\
p00,0,0.62,-1,0.5\n\
p01,1,1.5567499630944013,1,0.5909090909090909\n\
p02,2,0.9778792717770607,-1,0.6818181818181819\n\
p03,3,1.8821858797087616,1,0.7727272727272727\n\
p04,4,1.1894442079008758,-1,0.8636363636363636\n\
p05,5,1.9252234140045912,1,0.9545454545454546\n\
p06,6,1.0570929708088244,-1,1.0454545454545454\n\
p07,7,1.7987217360155938,1,1.1363636363636362\n\
p08,8,0.9428384691256566,-1,1.2272727272727273\n\
p09,9,1.868463710927986,1,1.3181818181818183\n\
p10,10,1.2705472908028612,-1,1.4090909090909092\n\
p11,11,2.3999999999999995,1,1.5\n",
    )
    .expect("input");
    let output = directory.path().join("fit.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "spatial-varying-coefficient",
            "--input",
            input.to_str().unwrap(),
            "--global-predictor-name",
            "global_x",
            "--spatial-predictor-name",
            "spatial_z",
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "2",
            "--coefficient-prior-sd",
            "2",
            "--amplitude-prior-sd",
            "1",
            "--length-scale-prior-sd-um",
            "5",
            "--known-noise-sd",
            "0.08",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "1000",
            "--target-accept",
            "0.97",
            "--seed",
            "9101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("SVC JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_spatial_varying_coefficient_fit"
    );
    assert_eq!(result["fit_state"], "complete");
    assert!(
        result["posterior"]["global_coefficient"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        result["posterior"]["spatial_mean_coefficient"]["mean"]
            .as_f64()
            .unwrap()
            > 0.4
    );
    let field = result["coefficient_field"].as_array().unwrap();
    assert_eq!(field.len(), 12);
    let deviations = field
        .iter()
        .map(|row| row["deviation"]["mean"].as_f64().unwrap())
        .collect::<Vec<_>>();
    assert!(deviations.iter().sum::<f64>().abs() <= 1e-10);
    let range = deviations.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - deviations.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(range > 0.1);
    assert_eq!(
        result["constraints"],
        serde_json::json!(["sum_to_zero:delta"])
    );
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
}
