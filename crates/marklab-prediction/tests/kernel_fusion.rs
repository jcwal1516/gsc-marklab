use marklab_prediction::{
    learn_multimodal_kernels, KernelFusionKernel, KernelFusionLimits, KernelFusionModality,
    KernelFusionPatient, KernelFusionSpec,
};

fn fixture(identical_modalities: bool) -> KernelFusionSpec {
    KernelFusionSpec {
        format: "marklab.multimodal_kernel_fusion_request".into(),
        version: 1,
        target_name: "research_response".into(),
        target_unit: "a.u.".into(),
        modalities: vec![
            KernelFusionModality {
                name: "patch".into(),
                feature_names: vec!["patch_feature".into()],
                kernel: KernelFusionKernel::Linear,
            },
            KernelFusionModality {
                name: "cell".into(),
                feature_names: vec!["cell_feature".into()],
                kernel: KernelFusionKernel::Linear,
            },
        ],
        patients: (0..24)
            .map(|i| {
                let signal = (i / 3) as f64 - 3.5;
                let patch = if identical_modalities {
                    10.0 + 2.0 * signal
                } else {
                    ((i / 3) % 2) as f64
                };
                KernelFusionPatient {
                    patient_id: format!("p{i:02}"),
                    outer_fold: i % 3,
                    inner_fold: (i / 3) % 2,
                    target: 7.0 + 3.0 * signal,
                    features: vec![vec![patch], vec![signal]],
                }
            })
            .collect(),
        outer_folds: 3,
        inner_folds: 2,
        weight_denominator: 4,
        ridge_penalties: vec![0.1, 1.0],
        limits: KernelFusionLimits {
            maximum_patients: 100,
            maximum_features: 20,
            maximum_weight_candidates: 100,
            maximum_work_units: 50_000_000,
            maximum_memory_mib: 64,
        },
    }
}

#[test]
fn centered_kernel_ridge_matches_the_linear_closed_form() {
    // Both affine-related feature blocks standardize to the same unit-trace linear kernel.
    // Every outer-training set has 16 equally distributed rows; dual ridge shrinks the
    // exact centered linear response by n/(n+lambda). This is independent of modal weights.
    for multiplier in [1e-100, 1.0, 1e90] {
        let mut spec = fixture(true);
        for patient in &mut spec.patients {
            for modality in &mut patient.features {
                for value in modality {
                    *value *= multiplier;
                }
            }
        }
        let result = learn_multimodal_kernels(spec).unwrap();
        for model in &result.models {
            for fold in &model.folds {
                assert_eq!(fold.ridge_penalty, 0.1);
                assert_eq!(fold.training_patients, 16);
                assert!(fold.inactive_modalities.is_empty());
            }
            for prediction in &model.predictions {
                let expected = 7.0 + 16.0 / 16.1 * (prediction.observed - 7.0);
                assert!((prediction.predicted - expected).abs() < 1e-10);
            }
        }
    }
}

#[test]
fn weights_learn_signal_without_using_outer_test_labels() {
    let spec = fixture(false);
    let result = learn_multimodal_kernels(spec.clone()).unwrap();
    let learned = result
        .models
        .iter()
        .find(|model| model.id == "learned")
        .unwrap();
    let patch = result
        .models
        .iter()
        .find(|model| model.id == "only_patch")
        .unwrap();
    assert!(learned.rmse < patch.rmse * 0.1);
    assert!(learned.folds.iter().all(|fold| fold.weights[1] >= 0.75));
    let mut perturbed = spec.clone();
    for patient in &mut perturbed.patients {
        if patient.outer_fold == 0 {
            patient.target += 1000.0;
        }
    }
    let rerun = learn_multimodal_kernels(perturbed).unwrap();
    let heldout = rerun
        .models
        .iter()
        .find(|model| model.id == "learned")
        .unwrap();
    assert_eq!(learned.folds[0], heldout.folds[0]);
    for (before, after) in learned
        .predictions
        .iter()
        .zip(&heldout.predictions)
        .filter(|(row, _)| row.outer_fold == 0)
    {
        assert_eq!(before.predicted, after.predicted);
    }
    let mut reordered = spec;
    reordered.patients.reverse();
    assert_eq!(result, learn_multimodal_kernels(reordered).unwrap());
}

#[test]
fn nonlinear_kernel_and_constant_training_support_are_explicit() {
    let mut spec = fixture(false);
    spec.modalities[0].kernel = KernelFusionKernel::Rbf { bandwidth: 0.7 };
    // A constant source contains no centered training information. It remains visible
    // as an inactive modality and predicts only the training intercept when used alone.
    for patient in &mut spec.patients {
        patient.features[1][0] = 1.0;
    }
    let result = learn_multimodal_kernels(spec).unwrap();
    let patch = result
        .models
        .iter()
        .find(|model| model.id == "only_patch")
        .unwrap();
    for prediction in &patch.predictions {
        let patient_index = prediction.patient_id[1..].parse::<usize>().unwrap();
        let sign = if (patient_index / 3) % 2 == 0 {
            -1.0
        } else {
            1.0
        };
        let expected = 7.0 + sign * (16.0 / 16.1) * 1.5;
        assert!((prediction.predicted - expected).abs() < 1e-10);
    }
    let constant = result
        .models
        .iter()
        .find(|model| model.id == "only_cell")
        .unwrap();
    assert!(constant
        .folds
        .iter()
        .all(|fold| fold.inactive_modalities.contains(&"cell".to_owned())));
    assert!(constant
        .predictions
        .iter()
        .all(|row| (row.predicted - 7.0).abs() < 1e-12));
    assert!(result.models.iter().all(|model| model.rmse.is_finite()));
}

#[test]
fn invalid_folds_features_and_work_are_rejected() {
    let good = fixture(false);
    let mut bad = good.clone();
    bad.patients[0].features.pop();
    assert!(learn_multimodal_kernels(bad).is_err());
    let mut bad = good.clone();
    bad.patients[0].patient_id = bad.patients[1].patient_id.clone();
    assert!(learn_multimodal_kernels(bad).is_err());
    let mut bad = good.clone();
    bad.patients.iter_mut().for_each(|row| row.inner_fold = 0);
    assert!(learn_multimodal_kernels(bad).is_err());
    let mut bad = good.clone();
    bad.limits.maximum_work_units = 1;
    assert!(learn_multimodal_kernels(bad).is_err());
    let mut bad = good.clone();
    bad.patients[0].target = f64::NAN;
    assert!(learn_multimodal_kernels(bad).is_err());
    let mut bad = good;
    bad.ridge_penalties = vec![0.0];
    assert!(learn_multimodal_kernels(bad).is_err());
}
