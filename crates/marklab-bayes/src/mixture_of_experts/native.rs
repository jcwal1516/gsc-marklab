//! Context/availability gate fit, disjoint calibration, and test evaluation.
use super::{
    MixtureCalibrator, MixtureGateModel, MixtureMetrics, MixtureOfExpertsPatient,
    MixtureOfExpertsSpec, MixturePrediction,
};
use crate::{probability_transform::sigmoid, BayesError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

#[path = "gate.rs"]
mod gate;

static IMPLEMENTATION_SHA256: LazyLock<String> = LazyLock::new(|| {
    let mut hash = Sha256::new();
    hash.update(
        concat!(
            include_str!("native.rs"),
            include_str!("gate.rs"),
            include_str!("../mixture_of_experts.rs"),
            include_str!("../logistic_fit.rs"),
            include_str!("../linalg.rs"),
            include_str!("../probability_transform.rs")
        )
        .as_bytes(),
    );
    hash.update(marklab_numerics::LBFGS_IMPLEMENTATION_SOURCE.as_bytes());
    format!("{:x}", hash.finalize())
});

/// Source-identified native execution, independent of the frozen Python oracle.
#[derive(Debug, Serialize)]
pub struct NativeMixtureOfExpertsBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}

/// Complete native context-gated mixture result.
#[derive(Debug, Serialize)]
pub struct MixtureOfExpertsFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativeMixtureOfExpertsBackend,
    pub request_sha256: String,
    pub expert_prediction_source: &'static str,
    pub gate_fit_split: &'static str,
    pub calibration_split: &'static str,
    pub evaluation_split: &'static str,
    pub experts: Vec<String>,
    pub gating_features: Vec<String>,
    pub model: MixtureGateModel,
    pub calibrator: MixtureCalibrator,
    pub ood_threshold: f64,
    pub predictions: Vec<MixturePrediction>,
    pub metrics: MixtureMetrics,
    pub claim_status: &'static str,
}

/// Fit the gate on gate-train patients, calibrate on separate patients, and evaluate test patients.
///
/// Preserves 30..=10,000 unique patient-OOF rows, 1..=16 context columns, 2..=8 experts,
/// training-only population standardization, exact unavailable-expert zeros, positive L2 and entropy
/// controls, disjoint Platt calibration, and a calibration-only nearest-rank OOD threshold. The gate
/// uses at most 2,000 L-BFGS iterations and stops at a 1e-7 gradient bound or 1e-12 adjacent
/// relative objective change; calibration uses the existing 1,000-iteration, 1e-8-gradient
/// logistic owner. Memory is O(n*(context+experts)+parameters*history).
/// Invalid inputs, degenerate training context, nonconvergence, nonfinite results, output above
/// 16 MiB, and cooperative deadline exhaustion are errors. Hard cancellation belongs to runtime.
pub fn fit_mixture_of_experts(
    spec: MixtureOfExpertsSpec,
) -> Result<MixtureOfExpertsFit, BayesError> {
    let start = Instant::now();
    let spec = spec.validated()?;
    let deadline = start + Duration::from_secs(spec.timeout_seconds);
    let implementation_sha256 = LazyLock::force(&IMPLEMENTATION_SHA256).clone();
    let request_sha256 = input_identity(&implementation_sha256, &spec);
    let (mean, sd) = context_scaling(&spec, deadline)?;
    let prepared = prepare_gate(&spec, &mean, &sd, deadline)?;
    let coefficients = gate::fit(
        &prepared,
        spec.l2_penalty,
        spec.entropy_regularization,
        deadline,
    )
    .map_err(|error| BayesError::InvalidSpec(format!("mixture gate fit: {error}")))?;
    let gating_features = spec
        .context_names
        .iter()
        .cloned()
        .chain(
            spec.expert_names
                .iter()
                .map(|name| format!("{name}_available")),
        )
        .collect::<Vec<_>>();
    let model = MixtureGateModel {
        context_training_mean: mean,
        context_training_population_sd: sd,
        gating_feature_names: gating_features.clone(),
        coefficients_row_major: coefficients,
        coefficient_columns: (1 + gating_features.len()) as u32,
        l2_penalty: spec.l2_penalty,
        entropy_regularization: spec.entropy_regularization,
    };
    let calibrator = fit_calibrator(&spec, &model, deadline)?;
    let mut calibration_ood = spec
        .patients
        .iter()
        .filter(|patient| patient.split == "calibration")
        .map(|patient| ood_score(&model, &patient.context))
        .collect::<Result<Vec<_>, _>>()?;
    calibration_ood.sort_by(f64::total_cmp);
    let rank = (spec.ood_validation_quantile * calibration_ood.len() as f64).ceil() as usize;
    let ood_threshold = calibration_ood[rank - 1];
    let mut predictions = Vec::with_capacity(
        spec.patients
            .iter()
            .filter(|patient| patient.split == "test")
            .count(),
    );
    let mut squared = 0.0;
    for patient in spec
        .patients
        .iter()
        .filter(|patient| patient.split == "test")
    {
        check_deadline(deadline)?;
        let (raw_probability, gating_weights) = raw_probability(&model, patient)?;
        let probability = calibrated_probability(&calibrator, raw_probability)?;
        let ood_score = ood_score(&model, &patient.context)?;
        squared += (probability - f64::from(patient.label)).powi(2);
        predictions.push(MixturePrediction {
            patient_id: patient.patient_id.clone(),
            label: patient.label,
            availability: patient
                .expert_probabilities
                .iter()
                .map(Option::is_some)
                .collect(),
            gating_weights,
            raw_probability,
            probability,
            ood_score,
            exceeds_ood_threshold: ood_score > ood_threshold,
        });
    }
    let mean_gate_entropy = mean_gate_entropy(&spec, &model, deadline)?;
    let brier_score = squared / predictions.len() as f64;
    if !brier_score.is_finite() || !mean_gate_entropy.is_finite() || !ood_threshold.is_finite() {
        return Err(BayesError::InvalidSpec(
            "mixture-of-experts final diagnostics are nonfinite".into(),
        ));
    }
    let result = MixtureOfExpertsFit {
        format: "marklab.mixture_of_experts_fusion",
        version: 2,
        backend: NativeMixtureOfExpertsBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        expert_prediction_source: "patient_level_out_of_fold",
        gate_fit_split: "gate_train",
        calibration_split: "calibration",
        evaluation_split: "test",
        experts: spec.expert_names,
        gating_features,
        model,
        calibrator,
        ood_threshold,
        predictions,
        metrics: MixtureMetrics {
            brier_score,
            mean_gate_entropy,
        },
        claim_status: "experimental_context_gated_predictive_mixture",
    };
    if serde_json::to_vec(&result)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec(
            "mixture-of-experts output exceeds 16 MiB".into(),
        ));
    }
    check_deadline(deadline)?;
    Ok(result)
}

fn context_scaling(
    spec: &MixtureOfExpertsSpec,
    deadline: Instant,
) -> Result<(Vec<f64>, Vec<f64>), BayesError> {
    let rows = spec
        .patients
        .iter()
        .filter(|patient| patient.split == "gate_train");
    let count = rows.clone().count() as f64;
    let mut mean = vec![0.0; spec.context_names.len()];
    for patient in rows.clone() {
        check_deadline(deadline)?;
        for (total, value) in mean.iter_mut().zip(&patient.context) {
            *total += value;
        }
    }
    for value in &mut mean {
        *value /= count;
    }
    let mut sd = vec![0.0; mean.len()];
    for patient in rows {
        for ((total, value), center) in sd.iter_mut().zip(&patient.context).zip(&mean) {
            *total += (value - center).powi(2);
        }
    }
    for value in &mut sd {
        *value = (*value / count).sqrt();
        if !value.is_finite() || *value <= 1e-14 {
            return Err(BayesError::InvalidSpec(
                "every gating context feature must vary on gate_train".into(),
            ));
        }
    }
    Ok((mean, sd))
}

fn prepare_gate(
    spec: &MixtureOfExpertsSpec,
    mean: &[f64],
    sd: &[f64],
    deadline: Instant,
) -> Result<gate::PreparedGate, BayesError> {
    let rows = spec
        .patients
        .iter()
        .filter(|patient| patient.split == "gate_train");
    let count = rows.clone().count();
    let feature_columns = mean.len() + spec.expert_names.len();
    let mut features = Vec::with_capacity(count * feature_columns);
    let mut probabilities = Vec::with_capacity(count * spec.expert_names.len());
    let mut labels = Vec::with_capacity(count);
    for patient in rows {
        check_deadline(deadline)?;
        features.extend(
            patient
                .context
                .iter()
                .zip(mean)
                .zip(sd)
                .map(|((value, center), scale)| (value - center) / scale),
        );
        features.extend(
            patient
                .expert_probabilities
                .iter()
                .map(|value| f64::from(value.is_some())),
        );
        probabilities.extend(
            patient
                .expert_probabilities
                .iter()
                .map(|value| value.unwrap_or(f64::NAN)),
        );
        labels.push(f64::from(patient.label));
    }
    Ok(gate::PreparedGate {
        features,
        probabilities,
        labels,
        feature_columns,
        experts: spec.expert_names.len(),
    })
}

fn fit_calibrator(
    spec: &MixtureOfExpertsSpec,
    model: &MixtureGateModel,
    deadline: Instant,
) -> Result<MixtureCalibrator, BayesError> {
    let rows = spec
        .patients
        .iter()
        .filter(|patient| patient.split == "calibration");
    let positives = rows.clone().filter(|patient| patient.label == 1).count() as f64;
    let negatives = rows.clone().count() as f64 - positives;
    let mut logits = Vec::with_capacity(rows.clone().count());
    let mut targets = Vec::with_capacity(rows.clone().count());
    for patient in rows {
        check_deadline(deadline)?;
        let raw = raw_probability(model, patient)?.0;
        logits.push(reference_logit(raw));
        targets.push(if patient.label == 1 {
            (positives + 1.0) / (positives + 2.0)
        } else {
            1.0 / (negatives + 2.0)
        });
    }
    let parameters =
        crate::logistic_fit::fit_with_initial(&logits, &targets, &[0.0, 1.0], 0.0, deadline)
            .map_err(|error| BayesError::InvalidSpec(format!("mixture calibration: {error}")))?;
    Ok(MixtureCalibrator {
        intercept: parameters[0],
        slope: parameters[1],
    })
}

fn raw_probability(
    model: &MixtureGateModel,
    patient: &MixtureOfExpertsPatient,
) -> Result<(f64, Vec<f64>), BayesError> {
    let mut features = Vec::with_capacity(model.gating_feature_names.len());
    features.extend(
        patient
            .context
            .iter()
            .zip(&model.context_training_mean)
            .zip(&model.context_training_population_sd)
            .map(|((value, mean), sd)| (value - mean) / sd),
    );
    features.extend(
        patient
            .expert_probabilities
            .iter()
            .map(|value| f64::from(value.is_some())),
    );
    let mut weights = vec![0.0; patient.expert_probabilities.len()];
    gate::weights(
        &model.coefficients_row_major,
        model.coefficient_columns as usize,
        &features,
        &patient.expert_probabilities,
        &mut weights,
    )
    .map_err(|error| BayesError::InvalidSpec(error.to_string()))?;
    let raw = weights
        .iter()
        .zip(&patient.expert_probabilities)
        .map(|(weight, probability)| weight * probability.unwrap_or(0.0))
        .sum::<f64>();
    if !raw.is_finite() || !(0.0..=1.0).contains(&raw) {
        return Err(BayesError::InvalidSpec(
            "mixture raw probability is invalid".into(),
        ));
    }
    Ok((raw, weights))
}

fn reference_logit(raw: f64) -> f64 {
    let numerator = raw.clamp(1e-12, 1.0 - 1e-12);
    let denominator = (1.0 - raw).clamp(1e-12, 1.0);
    (numerator / denominator).ln()
}

fn calibrated_probability(calibrator: &MixtureCalibrator, raw: f64) -> Result<f64, BayesError> {
    let value = sigmoid(calibrator.intercept + calibrator.slope * reference_logit(raw));
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BayesError::InvalidSpec(
            "mixture calibrated probability is nonfinite".into(),
        ))
    }
}

fn ood_score(model: &MixtureGateModel, context: &[f64]) -> Result<f64, BayesError> {
    let score = context
        .iter()
        .zip(&model.context_training_mean)
        .zip(&model.context_training_population_sd)
        .map(|((value, mean), sd)| ((value - mean) / sd).powi(2))
        .sum::<f64>()
        .sqrt();
    if score.is_finite() {
        Ok(score)
    } else {
        Err(BayesError::InvalidSpec(
            "mixture OOD score is nonfinite".into(),
        ))
    }
}

fn mean_gate_entropy(
    spec: &MixtureOfExpertsSpec,
    model: &MixtureGateModel,
    deadline: Instant,
) -> Result<f64, BayesError> {
    let rows = spec
        .patients
        .iter()
        .filter(|patient| patient.split == "gate_train");
    let count = rows.clone().count() as f64;
    let mut total = 0.0;
    for patient in rows {
        check_deadline(deadline)?;
        total += raw_probability(model, patient)?
            .1
            .into_iter()
            .filter(|weight| *weight > 0.0)
            .map(|weight| -weight * weight.ln())
            .sum::<f64>();
    }
    Ok(total / count)
}

fn input_identity(implementation: &str, spec: &MixtureOfExpertsSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.mixture-of-experts.spec.v1\0");
    hash_text(&mut hash, implementation);
    hash.update((spec.context_names.len() as u64).to_le_bytes());
    for name in &spec.context_names {
        hash_text(&mut hash, name);
    }
    hash.update((spec.expert_names.len() as u64).to_le_bytes());
    for name in &spec.expert_names {
        hash_text(&mut hash, name);
    }
    hash.update((spec.patients.len() as u64).to_le_bytes());
    for patient in &spec.patients {
        hash_text(&mut hash, &patient.patient_id);
        hash_text(&mut hash, &patient.split);
        hash_text(&mut hash, &patient.expert_prediction_source);
        hash.update([patient.label]);
        for value in &patient.context {
            hash.update(value.to_bits().to_le_bytes());
        }
        for value in &patient.expert_probabilities {
            match value {
                Some(value) => {
                    hash.update([1]);
                    hash.update(value.to_bits().to_le_bytes());
                }
                None => hash.update([0]),
            }
        }
    }
    hash.update(spec.l2_penalty.to_bits().to_le_bytes());
    hash.update(spec.entropy_regularization.to_bits().to_le_bytes());
    hash.update(spec.ood_validation_quantile.to_bits().to_le_bytes());
    hash.update(spec.timeout_seconds.to_le_bytes());
    format!("{:x}", hash.finalize())
}

fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}

fn check_deadline(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(BayesError::InvalidSpec(
            "mixture-of-experts deadline exceeded".into(),
        ))
    } else {
        Ok(())
    }
}
