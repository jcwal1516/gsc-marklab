#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn anisotropic_gp3d_fits_and_predicts_a_bounded_axis_aligned_field() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("observations.csv");
    fs::write(
        &input,
        "observation_id,x_um,y_um,z_um,value\n\
o-00,0,0,0,0.00\n\
o-01,1,0,0,0.80\n\
o-02,2,0,0,1.20\n\
o-03,3,0,0,1.00\n\
o-04,0,2,0,0.20\n\
o-05,1,2,0,1.00\n\
o-06,2,2,0,1.40\n\
o-07,3,2,0,1.20\n\
o-08,0,0,4,0.10\n\
o-09,1,0,4,0.90\n\
o-10,2,2,4,1.50\n\
o-11,3,2,4,1.30\n",
    )
    .expect("observations");
    let prediction = directory.path().join("prediction.csv");
    fs::write(
        &prediction,
        "prediction_id,x_um,y_um,z_um\n\
p-1,0.5,1,2\n\
p-2,2.5,1,2\n",
    )
    .expect("prediction coordinates");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "anisotropic-gp-3d",
            "--input",
            input.to_str().expect("input path"),
            "--predict",
            prediction.to_str().expect("prediction path"),
            "--mean-prior-mean",
            "0",
            "--mean-prior-sd",
            "3",
            "--amplitude-prior-sd",
            "2",
            "--length-scale-x-prior-sd-um",
            "4",
            "--length-scale-y-prior-sd-um",
            "8",
            "--length-scale-z-prior-sd-um",
            "12",
            "--noise-prior-sd",
            "0.5",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "1500",
            "--draws",
            "1500",
            "--target-accept",
            "0.97",
            "--seed",
            "20260825",
            "--timeout-seconds",
            "240",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.bayesian_anisotropic_gp3d_fit");
    assert_eq!(result["version"], 1);
    assert_eq!(result["backend"]["name"], "pymc");
    assert_eq!(result["backend"]["version"], "6.3.0");
    assert_eq!(
        result["model"]["family"],
        "exact_axis_aligned_anisotropic_matern32_gp3d"
    );
    assert_eq!(result["model"]["coordinate_dimension"], 3);
    assert_eq!(result["claim_status"], "experimental");
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
    assert_eq!(result["input"]["observations"], 12);
    assert_eq!(result["input"]["predictions"], 2);
    assert_eq!(result["predictions"].as_array().unwrap().len(), 2);
    for axis in [
        "length_scale_x_um",
        "length_scale_y_um",
        "length_scale_z_um",
    ] {
        assert!(result["posterior"][axis]["mean"].as_f64().unwrap() > 0.0);
    }
    let metric = result["posterior_mean_anisotropy_metric"]
        .as_array()
        .expect("metric");
    assert_eq!(metric.len(), 3);
    assert_eq!(metric[0][1], 0.0);
    assert_eq!(metric[0][2], 0.0);
    assert_eq!(metric[1][0], 0.0);
    for prediction in result["predictions"].as_array().unwrap() {
        assert!(prediction["mean"].as_f64().unwrap().is_finite());
        assert!(prediction["sd"].as_f64().unwrap() > 0.0);
    }
}
