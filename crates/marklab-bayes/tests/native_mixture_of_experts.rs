use marklab_bayes::{fit_mixture_of_experts, MixtureOfExpertsPatient, MixtureOfExpertsSpec};
use serde_json::Value;

fn spec(name: &str) -> MixtureOfExpertsSpec {
    let value: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/tests/fixtures/mixture_of_experts/{name}.spec.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    MixtureOfExpertsSpec {
        patients: value["patients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| MixtureOfExpertsPatient {
                patient_id: row["patient_id"].as_str().unwrap().into(),
                split: row["split"].as_str().unwrap().into(),
                expert_prediction_source: row["expert_prediction_source"].as_str().unwrap().into(),
                label: row["label"].as_u64().unwrap() as u8,
                context: row["context"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap())
                    .collect(),
                expert_probabilities: row["expert_probabilities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(Value::as_f64)
                    .collect(),
            })
            .collect(),
        context_names: value["context_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        expert_names: value["expert_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        l2_penalty: value["l2_penalty"].as_f64().unwrap(),
        entropy_regularization: value["entropy_regularization"].as_f64().unwrap(),
        ood_validation_quantile: value["ood_validation_quantile"].as_f64().unwrap(),
        timeout_seconds: value["timeout_seconds"].as_u64().unwrap(),
    }
}

fn compare(left: &Value, right: &Value, path: &str, workload: &str) {
    match left {
        Value::Object(fields) => {
            assert_eq!(fields.len(), right.as_object().unwrap().len(), "{path}");
            for (key, value) in fields {
                compare(value, &right[key], &format!("{path}.{key}"), workload);
            }
        }
        Value::Array(rows) => {
            assert_eq!(rows.len(), right.as_array().unwrap().len(), "{path}");
            for (index, row) in rows.iter().enumerate() {
                compare(row, &right[index], &format!("{path}[{index}]"), workload);
            }
        }
        Value::Number(number) if number.is_f64() => {
            let expected = number.as_f64().unwrap();
            let actual = right.as_f64().unwrap();
            let tolerance = if workload == "demanding"
                && (path.contains("coefficients") || path.contains("gating_weights"))
            {
                1e-4
            } else if workload == "demanding"
                && (path.contains("intercept")
                    || path.contains("slope")
                    || path.contains("raw_probability")
                    || path.ends_with(".probability")
                    || path.contains("metrics"))
            {
                1e-5
            } else if path.contains("coefficients")
                || path.contains("intercept")
                || path.contains("slope")
            {
                2e-5
            } else {
                2e-6
            } * (1.0 + expected.abs());
            assert!(
                actual.is_finite() && (expected - actual).abs() <= tolerance,
                "{path}: {expected} != {actual} (tolerance {tolerance})"
            );
        }
        _ => assert_eq!(left, right, "{path}"),
    }
}

#[test]
fn complete_native_fit_matches_frozen_python_oracles() {
    for name in ["small", "representative", "maximum_features", "demanding"] {
        let reference: Value = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/tests/fixtures/mixture_of_experts/{name}.oracle.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let fit = serde_json::to_value(fit_mixture_of_experts(spec(name)).unwrap()).unwrap();
        for key in [
            "model",
            "calibrator",
            "ood_threshold",
            "predictions",
            "metrics",
        ] {
            compare(&reference[key], &fit[key], &format!("{name}.{key}"), name);
        }
        assert_eq!(fit["format"], "marklab.mixture_of_experts_fusion");
        assert_eq!(fit["version"], 2);
        assert_eq!(fit["backend"]["name"], "marklab-rust");
        assert_eq!(
            fit["claim_status"],
            "experimental_context_gated_predictive_mixture"
        );
    }
}

#[test]
fn fit_and_calibration_are_disjoint_and_patient_order_is_canonical() {
    let input = spec("small");
    let original = serde_json::to_value(fit_mixture_of_experts(input.clone()).unwrap()).unwrap();
    let mut changed = input.clone();
    for patient in &mut changed.patients {
        if patient.split == "test" {
            patient.label = 1 - patient.label;
            for value in patient.expert_probabilities.iter_mut().flatten() {
                *value = 1.0 - *value;
            }
            patient.context[0] += 100.0;
        }
    }
    let test_changed = serde_json::to_value(fit_mixture_of_experts(changed).unwrap()).unwrap();
    assert_eq!(original["model"], test_changed["model"]);
    assert_eq!(original["calibrator"], test_changed["calibrator"]);
    assert_eq!(original["ood_threshold"], test_changed["ood_threshold"]);

    let mut changed = input.clone();
    for patient in &mut changed.patients {
        if patient.split == "calibration" {
            patient.label = 1 - patient.label;
        }
    }
    let calibration_changed =
        serde_json::to_value(fit_mixture_of_experts(changed).unwrap()).unwrap();
    assert_eq!(original["model"], calibration_changed["model"]);
    assert_ne!(original["calibrator"], calibration_changed["calibrator"]);
    assert_eq!(
        original["ood_threshold"],
        calibration_changed["ood_threshold"]
    );

    let mut reversed = input;
    reversed.patients.reverse();
    assert_eq!(
        original,
        serde_json::to_value(fit_mixture_of_experts(reversed).unwrap()).unwrap()
    );
}

#[test]
fn unavailable_experts_have_exact_zero_weight() {
    let fit = fit_mixture_of_experts(spec("small")).unwrap();
    for prediction in &fit.predictions {
        for (available, weight) in prediction
            .availability
            .iter()
            .zip(&prediction.gating_weights)
        {
            if !available {
                assert_eq!(*weight, 0.0);
            }
        }
        assert!((prediction.gating_weights.iter().sum::<f64>() - 1.0).abs() <= 1e-12);
    }
    assert!(fit.metrics.brier_score < 0.25);
}

#[test]
fn invalid_shortcuts_leakage_degeneracy_and_resources_are_rejected() {
    for case in 0..8 {
        let mut input = spec("small");
        match case {
            0 => input.context_names[0] = "context_site_id".into(),
            1 => input.patients[0].expert_prediction_source = "in_sample".into(),
            2 => input.patients[0].expert_probabilities.fill(None),
            3 => input.patients[0].context[0] = f64::NAN,
            4 => input.l2_penalty = 0.0,
            5 => input.entropy_regularization = 0.0,
            6 => input.timeout_seconds = 0,
            _ => {
                for patient in input
                    .patients
                    .iter_mut()
                    .filter(|patient| patient.split == "gate_train")
                {
                    patient.context[0] = 1.0;
                }
            }
        }
        assert!(fit_mixture_of_experts(input).is_err(), "case {case}");
    }
}

#[test]
fn legacy_worker_result_remains_readable() {
    let raw = include_str!("fixtures/mixture_of_experts/small.oracle.json");
    let value: Value = serde_json::from_str(raw).unwrap();
    let request = marklab_bayes::MixtureOfExpertsWorkerRequest::new(
        spec("small"),
        value["backend"]["environment_lock_sha256"]
            .as_str()
            .unwrap()
            .into(),
        value["backend"]["worker_sha256"].as_str().unwrap().into(),
    )
    .unwrap();
    let result: marklab_bayes::MixtureOfExpertsWorkerResult = serde_json::from_str(raw).unwrap();
    result
        .validate(
            &request,
            &marklab_bayes::sha256_hex(include_bytes!(
                "fixtures/mixture_of_experts/small.request.json"
            )),
        )
        .unwrap();
}
