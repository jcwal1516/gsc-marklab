//! Complete grouped predictive workflow; no file discovery, process execution or publication.
use super::{
    PredictiveStackingSpec, StackingPatientDensity, StackingWeight, StackingWeightSensitivity,
};
use crate::BayesError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[path = "optimizer.rs"]
mod optimizer;

/// Native implementation provenance, independent of Python reference identity.
#[derive(Debug, Serialize)]
pub struct NativeStackingBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}
/// Full-fit weights, every patient mixture, and sensitivity over every leave-one-patient refit.
#[derive(Debug, Serialize)]
pub struct PredictiveStackingFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativeStackingBackend,
    pub request_sha256: String,
    pub held_out_unit: &'static str,
    pub patient_count: u32,
    pub weights: Vec<StackingWeight>,
    pub objective_sum_log_predictive_density: f64,
    pub grouped_mixture_log_predictive_density: Vec<StackingPatientDensity>,
    pub leave_one_patient_out_sensitivity: Vec<StackingWeightSensitivity>,
    pub weight_interpretation: &'static str,
}
/// Optimize the simplex from patient-held-out log densities, then refit excluding every patient.
///
/// Canonicalizes patient order and preserves model order. Requires 8..=500 patients, 2..=16 models,
/// finite densities, declared patient held-out units and 1..=3600 seconds. Each fit starts uniformly
/// and retains exact zero weights, within 2000 iterations and the 1e-12 objective-change target.
/// Uses O(patients*models + models²) scratch plus outputs; full jackknife work remains bounded by the
/// existing admission contract. Invalid input, nonfinite results, nonconvergence, output over 16 MiB
/// and cooperative deadline exhaustion are errors. Process cancellation belongs to the runtime.
pub fn fit_predictive_stacking(
    spec: PredictiveStackingSpec,
) -> Result<PredictiveStackingFit, BayesError> {
    let start = Instant::now();
    let spec = spec.validated()?;
    let deadline = start + Duration::from_secs(spec.timeout_seconds);
    // The required executable identity is immutable within this process.
    static IMPLEMENTATION_SHA256: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        crate::sha256_hex(
            concat!(
                include_str!("native.rs"),
                include_str!("optimizer.rs"),
                include_str!("../predictive_stacking.rs"),
                include_str!("../linalg.rs")
            )
            .as_bytes(),
        )
    });
    let implementation_sha256 = IMPLEMENTATION_SHA256.clone();
    let request_sha256 = identity(&implementation_sha256, &spec);
    let n = spec.patients.len();
    let d = spec.model_names.len();
    let mut matrix = Vec::with_capacity(n * d);
    let mut shifts = Vec::with_capacity(n);
    for row in &spec.patients {
        let shift = row
            .log_predictive_densities
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        shifts.push(shift);
        matrix.extend(
            row.log_predictive_densities
                .iter()
                .map(|v| (v - shift).exp()),
        );
    }
    let prepared = optimizer::Prepared {
        matrix,
        shifts,
        columns: d,
    };
    let fitted = optimizer::fit(&prepared, None, deadline)?;
    let mut minima = vec![1_f64; d];
    let mut maxima = vec![0_f64; d];
    for patient in 0..n {
        let weights = optimizer::fit(&prepared, Some(patient), deadline)?;
        for j in 0..d {
            minima[j] = minima[j].min(weights[j]);
            maxima[j] = maxima[j].max(weights[j]);
        }
    }
    let grouped_mixture_log_predictive_density = spec
        .patients
        .iter()
        .map(|p| StackingPatientDensity {
            patient_id: p.patient_id.clone(),
            mixture_log_predictive_density: super::log_mixture(
                &fitted,
                &p.log_predictive_densities,
            ),
        })
        .collect::<Vec<_>>();
    let objective_sum_log_predictive_density = grouped_mixture_log_predictive_density
        .iter()
        .map(|p| p.mixture_log_predictive_density)
        .sum::<f64>();
    if !objective_sum_log_predictive_density.is_finite() {
        return Err(BayesError::InvalidSpec(
            "predictive stacking final objective is nonfinite".into(),
        ));
    }
    let weights = spec
        .model_names
        .iter()
        .zip(fitted)
        .map(|(model, weight)| StackingWeight {
            model: model.clone(),
            weight,
        })
        .collect();
    let leave_one_patient_out_sensitivity = spec
        .model_names
        .iter()
        .enumerate()
        .map(|(i, model)| StackingWeightSensitivity {
            model: model.clone(),
            leave_one_patient_out_minimum: minima[i],
            leave_one_patient_out_maximum: maxima[i],
        })
        .collect();
    let result = PredictiveStackingFit {
        format: "marklab.predictive_stacking",
        version: 2,
        backend: NativeStackingBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        held_out_unit: "patient",
        patient_count: n as u32,
        weights,
        objective_sum_log_predictive_density,
        grouped_mixture_log_predictive_density,
        leave_one_patient_out_sensitivity,
        weight_interpretation: "predictive_optimization_weights_not_posterior_model_probabilities",
    };
    if serde_json::to_vec(&result)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec(
            "predictive stacking output exceeds 16 MiB".into(),
        ));
    }
    optimizer::deadline_check(deadline)?;
    Ok(result)
}
fn identity(implementation: &str, spec: &PredictiveStackingSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.predictive-stacking.spec.v1\0");
    hash_text(&mut hash, implementation);
    hash.update((spec.model_names.len() as u64).to_le_bytes());
    for name in &spec.model_names {
        hash_text(&mut hash, name);
    }
    hash.update((spec.patients.len() as u64).to_le_bytes());
    for row in &spec.patients {
        hash_text(&mut hash, &row.patient_id);
        hash_text(&mut hash, &row.held_out_unit);
        for v in &row.log_predictive_densities {
            hash.update(v.to_bits().to_le_bytes());
        }
    }
    hash.update(spec.timeout_seconds.to_le_bytes());
    format!("{:x}", hash.finalize())
}
fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}
