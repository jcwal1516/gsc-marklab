use marklab_bayes::{fit_predictive_stacking, PredictiveStackingPatient, PredictiveStackingSpec};
use serde_json::Value;
fn fixture(name: &str, suffix: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/tests/fixtures/predictive_stacking/{name}.{suffix}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
fn spec(name: &str) -> PredictiveStackingSpec {
    let v = fixture(name, "spec");
    PredictiveStackingSpec {
        patients: v["patients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PredictiveStackingPatient {
                patient_id: p["patient_id"].as_str().unwrap().into(),
                held_out_unit: p["held_out_unit"].as_str().unwrap().into(),
                log_predictive_densities: p["log_predictive_densities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect(),
            })
            .collect(),
        model_names: v["model_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        timeout_seconds: 120,
    }
}
fn compare(x: &Value, y: &Value, path: &str) {
    match x {
        Value::Object(fields) => {
            assert_eq!(fields.len(), y.as_object().unwrap().len(), "{path}");
            for (k, v) in fields {
                compare(v, &y[k], &format!("{path}.{k}"));
            }
        }
        Value::Array(rows) => {
            assert_eq!(rows.len(), y.as_array().unwrap().len(), "{path}");
            for (i, v) in rows.iter().enumerate() {
                compare(v, &y[i], &format!("{path}[{i}]"));
            }
        }
        Value::Number(n) if n.is_f64() => {
            let x = n.as_f64().unwrap();
            let y = y.as_f64().unwrap();
            assert!(
                y.is_finite() && (x - y).abs() <= 1e-6 * (1. + x.abs()),
                "{path}: {x} != {y}"
            );
        }
        _ => assert_eq!(x, y, "{path}"),
    }
}
#[test]
fn complete_stacking_and_every_jackknife_extremum_match_frozen_slsqp() {
    for name in [
        "symmetric",
        "boundary",
        "representative",
        "demanding",
        "maximum",
        "identical",
        "duplicate_models",
        "offset",
        "near_duplicate",
    ] {
        let oracle = fixture(name, "oracle");
        let fit = serde_json::to_value(
            fit_predictive_stacking(spec(name)).unwrap_or_else(|e| panic!("{name}: {e}")),
        )
        .unwrap();
        for key in [
            "weights",
            "objective_sum_log_predictive_density",
            "grouped_mixture_log_predictive_density",
            "leave_one_patient_out_sensitivity",
        ] {
            compare(&oracle[key], &fit[key], &format!("{name}.{key}"));
        }
        assert_eq!(fit["version"], 2);
        assert_eq!(fit["backend"]["name"], "marklab-rust");
    }
}
#[test]
fn symmetric_jackknife_has_the_analytic_seven_patient_optimum() {
    let fit = fit_predictive_stacking(spec("symmetric")).unwrap();
    for row in fit.weights {
        assert!((row.weight - 0.5).abs() < 1e-8);
    }
    for row in fit.leave_one_patient_out_sensitivity {
        assert!((row.leave_one_patient_out_minimum - 8. / 21.).abs() < 1e-6);
        assert!((row.leave_one_patient_out_maximum - 13. / 21.).abs() < 1e-6);
    }
}

#[test]
fn stacking_preserves_canonical_patients_model_order_and_exact_boundary_weights() {
    let input = spec("boundary");
    let baseline = fit_predictive_stacking(input.clone()).unwrap();
    assert_eq!(
        baseline
            .weights
            .iter()
            .map(|w| w.weight)
            .collect::<Vec<_>>(),
        vec![1., 0., 0.]
    );
    let mut reversed = input;
    reversed.patients.reverse();
    let reordered = fit_predictive_stacking(reversed).unwrap();
    assert_eq!(
        serde_json::to_vec(&baseline).unwrap(),
        serde_json::to_vec(&reordered).unwrap()
    );
}

#[test]
fn stacking_admission_rejects_wrong_units_duplicate_ids_and_nonfinite_densities() {
    for case in 0..7 {
        let mut s = spec("symmetric");
        match case {
            0 => s.patients[0].held_out_unit = "slide".into(),
            1 => s.patients[1].patient_id = s.patients[0].patient_id.clone(),
            2 => s.patients[0].log_predictive_densities[0] = f64::INFINITY,
            3 => {
                s.patients.pop();
            }
            4 => s.model_names[1] = s.model_names[0].clone(),
            5 => s.patients[0]
                .log_predictive_densities
                .pop()
                .map(|_| ())
                .unwrap(),
            _ => s.timeout_seconds = 0,
        };
        assert!(fit_predictive_stacking(s).is_err());
    }
}

#[test]
fn legacy_stacking_results_still_validate_after_native_admission_extraction() {
    let name = "symmetric";
    let raw = std::fs::read(format!(
        "{}/tests/fixtures/predictive_stacking/{name}.request.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let request: Value = serde_json::from_slice(&raw).unwrap();
    let request = marklab_bayes::PredictiveStackingWorkerRequest::new(
        spec(name),
        request["backend"]["environment_lock_sha256"]
            .as_str()
            .unwrap()
            .into(),
        request["backend"]["worker_sha256"].as_str().unwrap().into(),
    )
    .unwrap();
    let result: marklab_bayes::PredictiveStackingWorkerResult =
        serde_json::from_value(fixture(name, "oracle")).unwrap();
    result
        .validate(&request, &marklab_bayes::sha256_hex(&raw))
        .unwrap();
}
