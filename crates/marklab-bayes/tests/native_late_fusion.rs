use marklab_bayes::{fit_late_fusion, LateFusionSpec};
use serde_json::Value;

fn spec(name: &str) -> LateFusionSpec {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/tests/fixtures/late_fusion/{name}.spec.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
fn compare(a: &Value, b: &Value, path: &str) {
    match a {
        Value::Object(fields) => {
            assert_eq!(fields.len(), b.as_object().unwrap().len(), "{path}");
            for (key, value) in fields {
                compare(value, &b[key], &format!("{path}.{key}"));
            }
        }
        Value::Array(rows) => {
            assert_eq!(rows.len(), b.as_array().unwrap().len(), "{path}");
            for (i, row) in rows.iter().enumerate() {
                compare(row, &b[i], &format!("{path}[{i}]"));
            }
        }
        Value::Number(n) if n.is_f64() => {
            let x = n.as_f64().unwrap();
            let y = b.as_f64().unwrap();
            let tolerance = if path.contains("intercept")
                || path.contains("coefficients")
                || path.contains("slope")
            {
                1e-6 * (1. + x.abs())
            } else {
                2e-7
            };
            assert!(
                y.is_finite() && (x - y).abs() <= tolerance,
                "{path}: {x} != {y}"
            );
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn complete_fusion_matches_frozen_oracles_including_missingness_and_ablations() {
    for name in ["small", "representative", "demanding_null"] {
        let reference: Value = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/tests/fixtures/late_fusion/{name}.oracle.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let fit = serde_json::to_value(fit_late_fusion(spec(name)).unwrap()).unwrap();
        for key in [
            "model",
            "calibrator",
            "predictions",
            "metrics",
            "missing_scenarios",
            "modality_ablations",
        ] {
            compare(&reference[key], &fit[key], &format!("{name}.{key}"));
        }
        assert_eq!(fit["backend"]["name"], "marklab-rust");
        assert_eq!(
            fit["claim_status"],
            "experimental_patient_level_late_fusion"
        );
    }
}
#[test]
fn fusion_fit_and_calibration_respect_disjoint_patient_boundaries() {
    let input = spec("small");
    let original = serde_json::to_value(fit_late_fusion(input.clone()).unwrap()).unwrap();
    let mut changed = input.clone();
    for p in &mut changed.patients {
        if p.split == "test" {
            p.label = 1 - p.label;
            for v in p.modality_probabilities.iter_mut().flatten() {
                *v = 1. - *v;
            }
        }
    }
    let fit = serde_json::to_value(fit_late_fusion(changed).unwrap()).unwrap();
    assert_eq!(original["model"], fit["model"]);
    assert_eq!(original["calibrator"], fit["calibrator"]);
    assert_ne!(original["request_sha256"], fit["request_sha256"]);
    let mut changed = input.clone();
    for p in &mut changed.patients {
        if p.split == "calibration" {
            p.label = 1 - p.label;
        }
    }
    let fit = serde_json::to_value(fit_late_fusion(changed).unwrap()).unwrap();
    assert_eq!(original["model"], fit["model"]);
    assert_ne!(original["calibrator"], fit["calibrator"]);
    let mut reversed = input;
    reversed.patients.reverse();
    assert_eq!(
        original,
        serde_json::to_value(fit_late_fusion(reversed).unwrap()).unwrap()
    );
}
#[test]
fn balanced_null_retains_half_probabilities_and_zero_ablations() {
    let fit = fit_late_fusion(spec("demanding_null")).unwrap();
    assert_eq!(fit.metrics.brier_score, 0.25);
    assert!(fit
        .predictions
        .iter()
        .all(|p| p.probability == 0.5 && p.raw_probability == 0.5));
    assert!(fit
        .modality_ablations
        .iter()
        .all(|a| a.brier_difference_ablated_minus_full == 0.));
}
#[test]
fn fusion_rejects_leaky_missing_and_invalid_inputs() {
    for case in 0..6 {
        let mut input = spec("small");
        match case {
            0 => input.patients[0].base_prediction_source = "in_sample".into(),
            1 => input.patients[0].modality_probabilities.fill(None),
            2 => input.patients[0].modality_probabilities[0] = Some(f64::NAN),
            3 => input.l2_penalty = 0.,
            4 => input.timeout_seconds = 0,
            _ => input.modalities[0] = input.modalities[1].clone(),
        };
        assert!(fit_late_fusion(input).is_err());
    }
}

#[test]
fn globally_missing_modality_has_zero_test_ablation_effect() {
    let fit = fit_late_fusion(spec("missing_column")).unwrap();
    assert!(fit
        .predictions
        .iter()
        .all(|p| !p.availability[3] && p.probability.is_finite()));
    assert_eq!(
        fit.modality_ablations[3].brier_difference_ablated_minus_full,
        0.
    );
    assert_eq!(
        fit.missing_scenarios.iter().map(|r| r.count).sum::<u32>(),
        100
    );
}

#[test]
fn demanding_admitted_fusion_preserves_complete_patient_and_ablation_work() {
    let fit = fit_late_fusion(spec("demanding")).unwrap();
    assert_eq!(fit.predictions.len(), 1000);
    assert_eq!(fit.modality_ablations.len(), 16);
    assert_eq!(
        fit.missing_scenarios.iter().map(|r| r.count).sum::<u32>(),
        1000
    );
    assert!(fit
        .predictions
        .iter()
        .all(|p| (0.0..=1.).contains(&p.probability)));
}

#[test]
fn constant_calibration_scores_preserve_smoothed_probability_and_initial_nullspace() {
    let mut input = spec("small");
    let mut count = 0;
    for p in &mut input.patients {
        if p.split == "calibration" {
            p.modality_probabilities.fill(Some(0.5));
            p.label = u8::from(count < 2);
            count += 1;
        }
    }
    let fit = fit_late_fusion(input).unwrap();
    let z = fit.model.intercept
        + fit
            .model
            .coefficients
            .chunks_exact(2)
            .map(|b| 0.5 * b[0] + b[1])
            .sum::<f64>();
    let raw = 1. / (1. + (-z).exp());
    let clipped = raw.clamp(1e-12, 1. - 1e-12);
    let logit = (clipped / (1. - clipped)).ln();
    let value = fit.calibrator.intercept + fit.calibrator.slope * logit;
    assert!((1. / (1. + (-value).exp()) - 0.23).abs() < 1e-8);
    assert!((fit.calibrator.slope - 1. - logit * fit.calibrator.intercept).abs() < 1e-8);
}

#[test]
fn legacy_fusion_result_remains_readable() {
    let raw = include_str!("fixtures/late_fusion/small.oracle.json");
    let value: Value = serde_json::from_str(raw).unwrap();
    let request = marklab_bayes::LateFusionWorkerRequest::new(
        spec("small"),
        value["backend"]["environment_lock_sha256"]
            .as_str()
            .unwrap()
            .into(),
        value["backend"]["worker_sha256"].as_str().unwrap().into(),
    )
    .unwrap();
    let result: marklab_bayes::LateFusionWorkerResult = serde_json::from_str(raw).unwrap();
    result
        .validate(
            &request,
            &marklab_bayes::sha256_hex(include_bytes!("fixtures/late_fusion/small.request.json")),
        )
        .unwrap();
}
