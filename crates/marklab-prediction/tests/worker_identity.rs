use marklab_prediction::{
    GroupedConformalPatient, GroupedConformalSpec, GroupedConformalWorkerRequest,
    GroupedConformalWorkerResult, LateFusionPatient, LateFusionSpec, LateFusionWorkerRequest,
    LateFusionWorkerResult,
};
use serde_json::json;

#[test]
fn grouped_conformal_rejects_a_result_from_another_python_version() {
    let patients = (0..30)
        .map(|i| GroupedConformalPatient {
            patient_id: format!("p{i:02}"),
            split: match i {
                0..12 => "train",
                12..22 => "calibration",
                _ => "test",
            }
            .into(),
            site: "a".into(),
            subgroup: "b".into(),
            label: i % 2,
            features: vec![0.0; 2],
        })
        .collect();
    let request = GroupedConformalWorkerRequest::new(
        GroupedConformalSpec {
            patients,
            feature_names: vec!["feature_1".into(), "feature_2".into()],
            alpha: 0.2,
            l2_penalty: 1.0,
            timeout_seconds: 30,
        },
        "lock".into(),
        "worker".into(),
    )
    .unwrap();
    let predictions: Vec<_> = request
        .patients
        .iter()
        .filter(|p| p.split == "test")
        .map(|p| {
            json!({
                "patient_id": p.patient_id, "site": p.site, "subgroup": p.subgroup,
                "label": p.label, "probability_one": 0.5, "prediction_set": [0, 1], "covered": true
            })
        })
        .collect();
    let coverage = |group: &str| json!({"group": group, "count": 8, "covered": 8, "coverage": 1.0});
    let mut result: GroupedConformalWorkerResult = serde_json::from_value(json!({
        "format": "marklab.scipy_grouped_conformal_worker_result", "version": 1,
        "backend": request.backend, "request_sha256": "request",
        "model": {"training_mean": [0.0, 0.0], "training_population_sd": [1.0, 1.0], "intercept": 0.0,
                  "coefficients": [0.0, 0.0], "l2_penalty": 1.0},
        "calibration_count": 10, "corrected_rank": 9, "nonconformity_threshold": 0.5,
        "predictions": predictions,
        "coverage": {"overall": coverage("all"), "by_site": [coverage("a")], "by_subgroup": [coverage("b")]}
    }))
    .expect("valid worker response");
    result
        .validate(&request, "request")
        .expect("matching runtime");
    result.backend.python_version = "other".into();
    assert!(result.validate(&request, "request").is_err());
}

#[test]
fn late_fusion_rejects_a_result_from_another_python_version() {
    let patients = (0..30)
        .map(|i| LateFusionPatient {
            patient_id: format!("p{i:02}"),
            split: ["meta_train", "calibration", "test"][usize::from(i) / 10].into(),
            base_prediction_source: "patient_level_out_of_fold".into(),
            label: i % 2,
            modality_probabilities: vec![Some(0.5); 2],
        })
        .collect();
    let request = LateFusionWorkerRequest::new(
        LateFusionSpec {
            patients,
            modalities: vec!["modality_a".into(), "modality_b".into()],
            l2_penalty: 1.0,
            timeout_seconds: 30,
        },
        "lock".into(),
        "worker".into(),
    )
    .unwrap();
    let predictions: Vec<_> = request
        .patients
        .iter()
        .filter(|p| p.split == "test")
        .map(|p| {
            json!({"patient_id": p.patient_id, "label": p.label, "availability": [true, true],
               "raw_probability": 0.5, "probability": 0.5})
        })
        .collect();
    let ablation = |name: &str| json!({"modality": name, "brier_score": 0.25, "brier_difference_ablated_minus_full": 0.0});
    let mut result: LateFusionWorkerResult = serde_json::from_value(json!({
        "format": "marklab.scipy_late_fusion_worker_result", "version": 1,
        "backend": request.backend, "request_sha256": "request",
        "model": {"intercept": 0.0, "coefficients": [0.0, 0.0, 0.0, 0.0],
                  "feature_names": ["modality_a_probability", "modality_a_available",
                                    "modality_b_probability", "modality_b_available"], "l2_penalty": 1.0},
        "calibrator": {"intercept": 0.0, "slope": 1.0}, "predictions": predictions,
        "metrics": {"brier_score": 0.25}, "missing_scenarios": [],
        "modality_ablations": [ablation("modality_a"), ablation("modality_b")]
    }))
    .expect("valid worker response");
    result
        .validate(&request, "request")
        .expect("matching runtime");
    result.backend.python_version = "other".into();
    assert!(result.validate(&request, "request").is_err());
}
