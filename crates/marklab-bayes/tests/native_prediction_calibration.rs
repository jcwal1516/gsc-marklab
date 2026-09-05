use marklab_bayes::{fit_prediction_calibration, PredictionCalibrationSpec};
use serde_json::Value;

fn spec(name: &str) -> PredictionCalibrationSpec {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/tests/fixtures/prediction_calibration/{name}.spec.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}

fn compare(a: &Value, b: &Value, path: &str) {
    match a {
        Value::Object(rows) => {
            for (key, value) in rows {
                compare(value, &b[key], &format!("{path}.{key}"));
            }
        }
        Value::Array(rows) => {
            assert_eq!(rows.len(), b.as_array().unwrap().len(), "{path}");
            for (i, value) in rows.iter().enumerate() {
                compare(value, &b[i], &format!("{path}[{i}]"));
            }
        }
        Value::Number(n) if n.is_f64() => {
            let x = n.as_f64().unwrap();
            let y = b.as_f64().unwrap();
            if path.ends_with(".score") || path.ends_with(".lower") || path.ends_with(".upper") {
                assert_eq!(x.to_bits(), y.to_bits(), "{path}");
                return;
            }
            let tol = if path.contains("wilson") {
                1e-12
            } else if path.contains("slope")
                || path.contains("intercept")
                || path.contains("calibration_in_the_large")
            {
                1e-6 * (1. + x.abs())
            } else {
                2e-7
            };
            assert!(
                y.is_finite() && (x - y).abs() <= tol,
                "{path}: {x} != {y}, tolerance {tol}"
            );
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

#[test]
fn native_calibration_matches_frozen_python_fits_and_all_diagnostics() {
    for name in [
        "small",
        "representative",
        "demanding",
        "constant",
        "separated",
        "saturated",
        "representative_binary_scores",
        "demanding_binary_scores",
    ] {
        let oracle: Value = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/tests/fixtures/prediction_calibration/{name}.oracle.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let fit = serde_json::to_value(fit_prediction_calibration(spec(name)).unwrap()).unwrap();
        for key in ["calibrator", "predictions", "metrics"] {
            compare(&oracle[key], &fit[key], &format!("{name}.{key}"));
        }
        assert_eq!(fit["backend"]["name"], "marklab-rust");
    }
}

#[test]
fn held_out_rows_do_not_fit_the_calibrator_and_order_is_canonical() {
    let input = spec("small");
    let original =
        serde_json::to_value(fit_prediction_calibration(input.clone()).unwrap()).unwrap();
    let mut changed = input.clone();
    for row in &mut changed.rows {
        if row.split == "test" {
            row.label = 1 - row.label;
            row.score *= 2.;
        }
    }
    let result = serde_json::to_value(fit_prediction_calibration(changed).unwrap()).unwrap();
    assert_eq!(original["calibrator"], result["calibrator"]);
    assert_ne!(original["request_sha256"], result["request_sha256"]);
    let mut reversed = input;
    reversed.rows.reverse();
    assert_eq!(
        original,
        serde_json::to_value(fit_prediction_calibration(reversed).unwrap()).unwrap()
    );
}

#[test]
fn native_calibration_preserves_admission() {
    for invalid in 0..5 {
        let mut input = spec("small");
        match invalid {
            0 => input.rows[0].score = f64::NAN,
            1 => input.rows[0].patient_id = input.rows[1].patient_id.clone(),
            2 => input.bins = 21,
            3 => input.timeout_seconds = 0,
            _ => input.rows.retain(|r| r.label == 1),
        }
        assert!(fit_prediction_calibration(input).is_err());
    }
}

#[test]
fn constant_raw_scores_preserve_the_smoothed_intercept_only_probability() {
    let mut input = spec("small");
    for (i, row) in input.rows.iter_mut().enumerate() {
        row.score = 2.;
        if row.split == "training_oof" {
            row.label = u8::from(i < 6);
        }
    }
    let fit = fit_prediction_calibration(input).unwrap();
    let expected = (6. * 7. / 8. + 2. / 4.) / 8.;
    for row in fit.predictions {
        assert!((row.probability - expected).abs() < 1e-10);
    }
    let initial = (7_f64 / 3.).ln();
    assert!((fit.calibrator.slope - 2. * (fit.calibrator.intercept - initial)).abs() < 1e-10);
}

#[test]
fn legacy_python_calibration_result_remains_readable() {
    let raw = include_str!("fixtures/prediction_calibration/small.oracle.json");
    let value: Value = serde_json::from_str(raw).unwrap();
    let request = marklab_bayes::PredictionCalibrationWorkerRequest::new(
        spec("small"),
        value["backend"]["environment_lock_sha256"]
            .as_str()
            .unwrap()
            .into(),
        value["backend"]["worker_sha256"].as_str().unwrap().into(),
    )
    .unwrap();
    let result: marklab_bayes::PredictionCalibrationWorkerResult =
        serde_json::from_str(raw).unwrap();
    let request_digest = marklab_bayes::sha256_hex(
        include_str!("fixtures/prediction_calibration/small.request.json")
            .trim_end()
            .as_bytes(),
    );
    result.validate(&request, &request_digest).unwrap();
}
