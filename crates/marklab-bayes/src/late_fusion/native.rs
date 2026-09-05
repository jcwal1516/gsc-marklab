//! Disjoint meta-fitting/calibration and fixed-model missingness/ablation evaluation.
use super::{
    LateFusionCalibrator, LateFusionMetric, LateFusionModel, LateFusionPrediction, LateFusionSpec,
    MissingScenario, ModalityAblation,
};
use crate::{
    probability_transform::{clipped_logit, sigmoid},
    BayesError,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// Source-identified native execution, independent of Python reference provenance.
#[derive(Debug, Serialize)]
pub struct NativeFusionBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}
/// Native late-fusion result; version 2 changes provenance, retaining the scientific contract.
#[derive(Debug, Serialize)]
pub struct LateFusionFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativeFusionBackend,
    pub request_sha256: String,
    pub base_prediction_source: &'static str,
    pub meta_fit_split: &'static str,
    pub calibration_split: &'static str,
    pub evaluation_split: &'static str,
    pub modalities: Vec<String>,
    pub model: LateFusionModel,
    pub calibrator: LateFusionCalibrator,
    pub predictions: Vec<LateFusionPrediction>,
    pub metrics: LateFusionMetric,
    pub missing_scenarios: Vec<MissingScenario>,
    pub modality_ablations: Vec<ModalityAblation>,
    pub claim_status: &'static str,
}

/// Fit probability/availability fusion on meta_train, calibrate separately, then evaluate test.
///
/// Preserves exact patient OOF declarations, 30..=100000 rows, 2..=16 modalities, at least ten
/// and both labels in each split, positive L2 and 1..=3600 seconds. Missing modalities contribute
/// (0.5,0); no standardization occurs. Both fits retain a 1000-iteration ceiling and 1e-8 gradient
/// criterion. Ablations change only test features, never fitted parameters. Uses O(n*m+m²) memory.
/// Errors include invalid/fully missing rows, nonfinite/nonconverged fits, 16 MiB output exhaustion
/// and cooperative timeout; applications needing hard cancellation use the native child boundary.
pub fn fit_late_fusion(spec: LateFusionSpec) -> Result<LateFusionFit, BayesError> {
    let start = Instant::now();
    let spec = spec.validated()?;
    let deadline = start + Duration::from_secs(spec.timeout_seconds);
    // The required executable identity is immutable within this process.
    static IMPLEMENTATION_SHA256: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        let mut identity = Sha256::new();
        identity.update(
            concat!(
                include_str!("native.rs"),
                include_str!("../late_fusion.rs"),
                include_str!("../logistic_fit.rs"),
                include_str!("../linalg.rs"),
                include_str!("../probability_transform.rs")
            )
            .as_bytes(),
        );
        identity.update(marklab_numerics::BFGS_IMPLEMENTATION_SOURCE.as_bytes());
        format!("{:x}", identity.finalize())
    });
    let implementation_sha256 = IMPLEMENTATION_SHA256.clone();
    let request_sha256 = input_identity(&implementation_sha256, &spec);
    let model = fit_model(&spec, deadline)?;
    let calibrator = fit_calibrator(&spec, &model, deadline)?;
    let mut predictions = Vec::new();
    let mut scenarios = BTreeMap::<String, (u32, f64)>::new();
    let mut ablated = vec![0.; spec.modalities.len()];
    let mut squared = 0.;
    for patient in spec.patients.iter().filter(|p| p.split == "test") {
        check_deadline(deadline)?;
        let raw = raw_probability(&model, &patient.modality_probabilities, None)?;
        let probability = calibrated(&calibrator, raw)?;
        let error = (probability - f64::from(patient.label)).powi(2);
        squared += error;
        let missing = spec
            .modalities
            .iter()
            .zip(&patient.modality_probabilities)
            .filter_map(|(name, p)| p.is_none().then_some(name.as_str()))
            .collect::<Vec<_>>();
        let scenario = if missing.is_empty() {
            "complete".into()
        } else {
            format!("missing_{}", missing.join("_and_"))
        };
        let group = scenarios.entry(scenario).or_default();
        group.0 += 1;
        group.1 += error;
        for (index, total) in ablated.iter_mut().enumerate() {
            let raw = raw_probability(&model, &patient.modality_probabilities, Some(index))?;
            *total += (calibrated(&calibrator, raw)? - f64::from(patient.label)).powi(2);
        }
        predictions.push(LateFusionPrediction {
            patient_id: patient.patient_id.clone(),
            label: patient.label,
            availability: patient
                .modality_probabilities
                .iter()
                .map(Option::is_some)
                .collect(),
            raw_probability: raw,
            probability,
        });
    }
    let count = predictions.len() as f64;
    let brier_score = squared / count;
    let modality_ablations = spec
        .modalities
        .iter()
        .zip(ablated)
        .map(|(modality, total)| ModalityAblation {
            modality: modality.clone(),
            brier_score: total / count,
            brier_difference_ablated_minus_full: total / count - brier_score,
        })
        .collect();
    let missing_scenarios = scenarios
        .into_iter()
        .map(|(scenario, (count, squared))| MissingScenario {
            scenario,
            count,
            brier_score: squared / f64::from(count),
        })
        .collect();
    let fit = LateFusionFit {
        format: "marklab.late_fusion",
        version: 2,
        backend: NativeFusionBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        base_prediction_source: "patient_level_out_of_fold",
        meta_fit_split: "meta_train",
        calibration_split: "calibration",
        evaluation_split: "test",
        modalities: spec.modalities,
        model,
        calibrator,
        predictions,
        metrics: LateFusionMetric { brier_score },
        missing_scenarios,
        modality_ablations,
        claim_status: "experimental_patient_level_late_fusion",
    };
    if serde_json::to_vec(&fit)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec(
            "late-fusion result exceeds 16 MiB".into(),
        ));
    }
    check_deadline(deadline)?;
    Ok(fit)
}

fn fit_model(spec: &LateFusionSpec, deadline: Instant) -> Result<LateFusionModel, BayesError> {
    let rows = spec.patients.iter().filter(|p| p.split == "meta_train");
    let d = 2 * spec.modalities.len();
    let mut matrix = Vec::with_capacity(rows.clone().count() * d);
    let mut labels = Vec::new();
    for p in rows {
        check_deadline(deadline)?;
        matrix.extend(
            p.modality_probabilities
                .iter()
                .flat_map(|v| [v.unwrap_or(0.5), f64::from(v.is_some())]),
        );
        labels.push(f64::from(p.label));
    }
    let parameters = crate::logistic_fit::fit(&matrix, &labels, d, spec.l2_penalty, deadline)
        .map_err(|e| BayesError::InvalidSpec(format!("late-fusion meta fit: {e}")))?;
    Ok(LateFusionModel {
        intercept: parameters[0],
        coefficients: parameters[1..].to_vec(),
        feature_names: spec
            .modalities
            .iter()
            .flat_map(|name| [format!("{name}_probability"), format!("{name}_available")])
            .collect(),
        l2_penalty: spec.l2_penalty,
    })
}
fn fit_calibrator(
    spec: &LateFusionSpec,
    model: &LateFusionModel,
    deadline: Instant,
) -> Result<LateFusionCalibrator, BayesError> {
    let rows = spec.patients.iter().filter(|p| p.split == "calibration");
    let positives = rows.clone().filter(|p| p.label == 1).count() as f64;
    let negatives = rows.clone().count() as f64 - positives;
    let mut logits = Vec::new();
    let mut targets = Vec::new();
    for p in rows {
        check_deadline(deadline)?;
        let raw = raw_probability(model, &p.modality_probabilities, None)?;
        // Calibration fitting clips numerator/denominator independently in the reference.
        logits.push((raw.clamp(1e-12, 1. - 1e-12) / (1. - raw).clamp(1e-12, 1.)).ln());
        targets.push(if p.label == 1 {
            (positives + 1.) / (positives + 2.)
        } else {
            1. / (negatives + 2.)
        });
    }
    let p = if logits.iter().all(|x| *x == logits[0]) {
        let x = logits[0];
        let q = (positives * (positives + 1.) / (positives + 2.) + negatives / (negatives + 2.))
            / (positives + negatives);
        let delta = ((q / (1. - q)).ln() - x) / (1. + x * x);
        let p = [delta, 1. + x * delta];
        let mut gradient = [0.; 2];
        crate::logistic_fit::objective(&logits, &targets, &p, &mut gradient, 0., deadline)
            .map_err(|e| BayesError::InvalidSpec(e.to_string()))?;
        if gradient.iter().any(|g| !g.is_finite() || g.abs() > 1e-8) {
            return Err(BayesError::InvalidSpec(
                "constant fusion calibrator is not stationary".into(),
            ));
        }
        Ok(p.to_vec())
    } else {
        crate::logistic_fit::fit_with_initial(&logits, &targets, &[0., 1.], 0., deadline)
    }
    .map_err(|e| BayesError::InvalidSpec(format!("late-fusion calibration: {e}")))?;
    Ok(LateFusionCalibrator {
        intercept: p[0],
        slope: p[1],
    })
}
fn raw_probability(
    model: &LateFusionModel,
    values: &[Option<f64>],
    ablate: Option<usize>,
) -> Result<f64, BayesError> {
    let sum = model
        .coefficients
        .chunks_exact(2)
        .zip(values)
        .enumerate()
        .map(|(i, (b, value))| {
            let value = if ablate == Some(i) { None } else { *value };
            b[0] * value.unwrap_or(0.5) + b[1] * f64::from(value.is_some())
        })
        .sum::<f64>();
    let probability = sigmoid(model.intercept + sum);
    if probability.is_finite() {
        Ok(probability)
    } else {
        Err(BayesError::InvalidSpec(
            "late-fusion raw probability is nonfinite".into(),
        ))
    }
}
fn calibrated(model: &LateFusionCalibrator, raw: f64) -> Result<f64, BayesError> {
    let p = sigmoid(model.intercept + model.slope * clipped_logit(raw));
    if p.is_finite() {
        Ok(p)
    } else {
        Err(BayesError::InvalidSpec(
            "late-fusion calibrated probability is nonfinite".into(),
        ))
    }
}
fn check_deadline(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(BayesError::InvalidSpec(
            "late-fusion deadline exceeded".into(),
        ))
    } else {
        Ok(())
    }
}
fn input_identity(implementation: &str, spec: &LateFusionSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.late-fusion.spec.v1\0");
    hash_text(&mut hash, implementation);
    hash.update((spec.modalities.len() as u64).to_le_bytes());
    for name in &spec.modalities {
        hash_text(&mut hash, name);
    }
    hash.update(spec.l2_penalty.to_bits().to_le_bytes());
    hash.update(spec.timeout_seconds.to_le_bytes());
    hash.update((spec.patients.len() as u64).to_le_bytes());
    for p in &spec.patients {
        hash_text(&mut hash, &p.patient_id);
        hash_text(&mut hash, &p.split);
        hash_text(&mut hash, &p.base_prediction_source);
        hash.update([p.label]);
        for value in &p.modality_probabilities {
            hash.update([u8::from(value.is_some())]);
            if let Some(value) = value {
                hash.update(value.to_bits().to_le_bytes());
            }
        }
    }
    format!("{:x}", hash.finalize())
}
fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}
