use marklab_bayes::{fit_grouped_conformal, GroupedConformalPatient, GroupedConformalSpec};

fn balanced() -> GroupedConformalSpec {
    let patients = (0..30)
        .map(|i| GroupedConformalPatient {
            patient_id: format!("p{i:02}"),
            split: if i < 12 {
                "train"
            } else if i < 22 {
                "calibration"
            } else {
                "test"
            }
            .into(),
            site: format!("s{}", i % 2),
            subgroup: "g".into(),
            label: (i % 2) as u8,
            features: vec![(i / 2) as f64, ((i / 2) % 3) as f64],
        })
        .collect();
    GroupedConformalSpec {
        patients,
        feature_names: vec!["feature_1".into(), "feature_2".into()],
        alpha: 0.2,
        l2_penalty: 0.1,
        timeout_seconds: 30,
    }
}

#[test]
fn balanced_labels_have_analytic_half_probabilities_and_inclusive_sets() {
    let fit = fit_grouped_conformal(balanced()).expect("native fit");
    assert_eq!(fit.corrected_rank, 9);
    assert_eq!(fit.nonconformity_threshold, 0.5);
    assert_eq!(fit.model.intercept, 0.0);
    assert_eq!(fit.model.coefficients, [0.0, 0.0]);
    assert_eq!(fit.coverage.overall.count, 8);
    assert_eq!(fit.coverage.overall.covered, 8);
    for prediction in &fit.predictions {
        assert_eq!(prediction.probability_one, 0.5);
        assert_eq!(prediction.prediction_set, [0, 1]);
    }
}

#[test]
fn test_labels_do_not_leak_into_the_fit_or_calibration() {
    let spec = balanced();
    let before = fit_grouped_conformal(spec.clone()).expect("fit");
    let mut changed = spec;
    for row in &mut changed.patients {
        if row.split == "test" {
            row.label ^= 1;
        }
    }
    let after = fit_grouped_conformal(changed).expect("fit");
    assert_eq!(before.model.coefficients, after.model.coefficients);
    assert_eq!(before.model.intercept, after.model.intercept);
    assert_eq!(
        before.nonconformity_threshold,
        after.nonconformity_threshold
    );
    assert_ne!(before.request_sha256, after.request_sha256);
}

#[test]
fn held_out_features_and_calibration_labels_cannot_change_training_preparation_or_fit() {
    let mut spec: GroupedConformalSpec =
        serde_json::from_str(include_str!("fixtures/grouped_conformal/small.spec.json")).unwrap();
    let before = fit_grouped_conformal(spec.clone()).unwrap();
    for patient in &mut spec.patients {
        if patient.split != "train" {
            for feature in &mut patient.features {
                *feature += 100.0;
            }
        }
        if patient.split == "calibration" {
            patient.label ^= 1;
        }
    }
    let after = fit_grouped_conformal(spec).unwrap();
    assert_eq!(
        serde_json::to_vec(&before.model).unwrap(),
        serde_json::to_vec(&after.model).unwrap()
    );
    assert_ne!(before.request_sha256, after.request_sha256);
}

#[test]
fn near_collinear_balanced_design_retains_the_analytic_solution_at_minimum_alpha() {
    let mut spec = balanced();
    spec.alpha = 1.0 / 11.0;
    for patient in &mut spec.patients {
        patient.features[1] = patient.features[0] + 1e-10 * patient.features[1];
    }
    let fit = fit_grouped_conformal(spec).unwrap();
    assert_eq!(fit.corrected_rank, 10);
    assert_eq!(fit.model.intercept, 0.0);
    assert_eq!(fit.model.coefficients, [0.0, 0.0]);
    assert_eq!(fit.nonconformity_threshold, 0.5);
    assert!(fit.predictions.iter().all(|p| p.prediction_set == [0, 1]));
}

#[test]
fn native_admission_rejects_degenerate_invalid_and_nonfinite_inputs() {
    let mut spec = balanced();
    spec.patients[0].features[0] = f64::INFINITY;
    assert!(fit_grouped_conformal(spec).is_err());
    let mut spec = balanced();
    for row in &mut spec.patients {
        if row.split == "train" {
            row.features[0] = 1.0;
        }
    }
    assert!(fit_grouped_conformal(spec).is_err());
    let mut spec = balanced();
    spec.patients[0].patient_id = spec.patients[1].patient_id.clone();
    assert!(fit_grouped_conformal(spec).is_err());
    let mut spec = balanced();
    spec.alpha = 0.01;
    assert!(fit_grouped_conformal(spec).is_err());
}

#[test]
fn canonical_patient_order_is_reproducible() {
    let mut spec = balanced();
    let before = fit_grouped_conformal(spec.clone()).expect("fit");
    spec.patients.reverse();
    let after = fit_grouped_conformal(spec).expect("fit");
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(&after).unwrap()
    );
}

#[test]
fn legacy_python_response_reader_and_validation_remain_available() {
    use marklab_bayes::{sha256_hex, GroupedConformalWorkerRequest, GroupedConformalWorkerResult};
    let spec: GroupedConformalSpec =
        serde_json::from_str(include_str!("fixtures/grouped_conformal/small.spec.json")).unwrap();
    let raw = include_bytes!("fixtures/grouped_conformal/small.request.json");
    let request: serde_json::Value = serde_json::from_slice(raw).unwrap();
    let typed = GroupedConformalWorkerRequest::new(
        spec,
        request["backend"]["environment_lock_sha256"]
            .as_str()
            .unwrap()
            .into(),
        request["backend"]["worker_sha256"].as_str().unwrap().into(),
    )
    .unwrap();
    let legacy: GroupedConformalWorkerResult =
        serde_json::from_str(include_str!("fixtures/grouped_conformal/small.oracle.json")).unwrap();
    legacy.validate(&typed, &sha256_hex(raw)).unwrap();
}

#[test]
fn native_fit_agrees_with_frozen_scipy_oracle() {
    let spec: GroupedConformalSpec =
        serde_json::from_str(include_str!("fixtures/grouped_conformal/small.spec.json")).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/grouped_conformal/small.oracle.json")).unwrap();
    let fit = fit_grouped_conformal(spec).expect("native reference fit");
    for (a, b) in fit
        .model
        .coefficients
        .iter()
        .zip(expected["model"]["coefficients"].as_array().unwrap())
    {
        assert!((a - b.as_f64().unwrap()).abs() <= 1e-6 * (1.0 + a.abs()));
    }
    assert!(
        (fit.nonconformity_threshold - expected["nonconformity_threshold"].as_f64().unwrap()).abs()
            <= 2e-7
    );
    assert_eq!(
        serde_json::to_value(&fit.coverage).unwrap(),
        expected["coverage"]
    );
    for (a, b) in fit
        .predictions
        .iter()
        .zip(expected["predictions"].as_array().unwrap())
    {
        assert!((a.probability_one - b["probability_one"].as_f64().unwrap()).abs() <= 2e-7);
        assert_eq!(
            serde_json::to_value(&a.prediction_set).unwrap(),
            b["prediction_set"]
        );
        assert_eq!(a.patient_id, b["patient_id"].as_str().unwrap());
    }
}

#[test]
fn native_fit_recovers_the_admitted_300_patient_reference() {
    let spec: GroupedConformalSpec = serde_json::from_str(include_str!(
        "fixtures/grouped_conformal/representative.spec.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grouped_conformal/representative.oracle.json"
    ))
    .unwrap();
    let fit = fit_grouped_conformal(spec)
        .expect("native fit must converge where the unchanged reference converges");
    for (a, b) in fit
        .model
        .coefficients
        .iter()
        .zip(expected["model"]["coefficients"].as_array().unwrap())
    {
        assert!((a - b.as_f64().unwrap()).abs() <= 1e-6 * (1.0 + a.abs()));
    }
    let actual = serde_json::to_value(&fit.coverage).unwrap();
    for family in ["by_site", "by_subgroup"] {
        let a = actual[family].as_array().unwrap();
        let b = expected["coverage"][family].as_array().unwrap();
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            for field in ["group", "count", "covered"] {
                assert_eq!(a[field], b[field]);
            }
            assert!(
                (a["coverage"].as_f64().unwrap() - b["coverage"].as_f64().unwrap()).abs() <= 1e-12
            );
        }
    }
    for field in ["group", "count", "covered"] {
        assert_eq!(
            actual["overall"][field],
            expected["coverage"]["overall"][field]
        );
    }
    assert!(
        (actual["overall"]["coverage"].as_f64().unwrap()
            - expected["coverage"]["overall"]["coverage"]
                .as_f64()
                .unwrap())
        .abs()
            <= 1e-12
    );
    for (a, b) in fit
        .predictions
        .iter()
        .zip(expected["predictions"].as_array().unwrap())
    {
        assert!((a.probability_one - b["probability_one"].as_f64().unwrap()).abs() <= 2e-7);
        assert_eq!(
            serde_json::to_value(&a.prediction_set).unwrap(),
            b["prediction_set"]
        );
    }
}
