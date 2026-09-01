#![cfg(feature = "cli")]

use std::{fmt::Write, fs};

use assert_cmd::Command;

#[test]
fn fitted_hierarchy_separates_nested_crossed_and_transport_components() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hierarchy.csv");
    let output = directory.path().join("result.json");
    let mut csv = String::from("patient_id,slide_id,roi_id,batch_id,cohort_id,exposure,outcome\n");
    for cohort in 0..3 {
        for patient_within_cohort in 0..4 {
            let patient = cohort * 4 + patient_within_cohort;
            for slide in 0..2 {
                for roi in 0..2 {
                    for replicate in 0..4 {
                        let exposure = [-1.0, -0.3, 0.3, 1.0][replicate];
                        let batch = (patient + slide + roi + replicate) % 4;
                        let cohort_intercept = [-0.7, 0.0, 0.7][cohort];
                        let cohort_slope = [-0.25, 0.0, 0.25][cohort];
                        let patient_intercept = 0.25 * (patient_within_cohort as f64 - 1.5);
                        let patient_slope = 0.12 * (patient_within_cohort as f64 - 1.5);
                        let slide_intercept = if slide == 0 { -0.3 } else { 0.3 };
                        let roi_intercept = if (patient + slide + roi) % 2 == 0 {
                            -0.22
                        } else {
                            0.22
                        };
                        let batch_intercept = [-0.2, -0.07, 0.07, 0.2][batch];
                        let residual_code = patient * 7 + slide * 5 + roi * 3 + replicate * 11;
                        let residual = 0.35 * (residual_code as f64 * 1.7).sin();
                        let outcome = cohort_intercept
                            + patient_intercept
                            + slide_intercept
                            + roi_intercept
                            + batch_intercept
                            + residual
                            + exposure * (1.2 + cohort_slope + patient_slope);
                        writeln!(
                            csv,
                            "p{patient},p{patient}-s{slide},p{patient}-s{slide}-r{roi},b{batch},c{cohort},{exposure},{outcome}"
                        )
                        .expect("CSV row");
                    }
                }
            }
        }
    }
    fs::write(&input, csv).expect("fixture");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "gaussian-crossed-nested-hierarchy",
            "--input",
            input.to_str().expect("input path"),
            "--intercept-prior-sd",
            "3",
            "--slope-prior-sd",
            "2",
            "--component-prior-sd",
            "0.5",
            "--chains",
            "4",
            "--tune",
            "700",
            "--draws",
            "500",
            "--target-accept",
            "0.98",
            "--seed",
            "20260901",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result bytes")).expect("result JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_gaussian_crossed_nested_hierarchy"
    );
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["input"]["patients"], 12);
    assert_eq!(result["input"]["slides"], 24);
    assert_eq!(result["input"]["rois"], 48);
    assert_eq!(result["input"]["batches"], 4);
    assert_eq!(result["input"]["cohorts"], 3);
    assert_eq!(result["input"]["observations"], 192);
    assert_eq!(
        result["model"]["nesting"],
        "roi_within_slide_within_patient"
    );
    assert_eq!(
        result["model"]["crossed_effect"],
        "batch_crossed_with_patient"
    );
    assert_eq!(
        result["model"]["random_slopes"],
        serde_json::json!(["patient_exposure", "cohort_exposure"])
    );
    assert!(
        result["posterior"]["exposure_slope"]["mean"]
            .as_f64()
            .expect("slope")
            > 0.6
    );
    for component in [
        "cohort_intercept_sd",
        "cohort_exposure_slope_sd",
        "patient_intercept_sd",
        "patient_exposure_slope_sd",
        "slide_intercept_sd",
        "roi_intercept_sd",
        "batch_intercept_sd",
        "residual_sd",
    ] {
        assert!(
            result["posterior"][component]["mean"]
                .as_f64()
                .expect("component mean")
                > 0.0
        );
    }
    assert_eq!(result["fit_state"], "complete", "{}", result["diagnostics"]);
}
