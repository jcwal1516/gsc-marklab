//! Patient-OOF calibration and held-out diagnostics, independent of application I/O.
use super::{
    CalibratedPrediction, PlattCalibrator, PredictionCalibrationMetrics, PredictionCalibrationSpec,
    ReliabilityBin,
};
use crate::{probability_transform::sigmoid, BayesError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[path = "logistic.rs"]
mod logistic;

/// Native execution identity; never claims a Python interpreter or environment lock.
#[derive(Debug, Serialize)]
pub struct NativeCalibrationBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}

/// Source-identified native calibration with unchanged patient-level scientific fields.
#[derive(Debug, Serialize)]
pub struct PredictionCalibrationFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativeCalibrationBackend,
    pub request_sha256: String,
    pub method: &'static str,
    pub fit_split: &'static str,
    pub evaluation_split: &'static str,
    pub calibrator: PlattCalibrator,
    pub predictions: Vec<CalibratedPrediction>,
    pub metrics: PredictionCalibrationMetrics,
}

/// Fit Platt calibration on raw training_oof scores and evaluate only on test patients.
///
/// Accepts 16..=100000 unique patients, at least eight and both labels per split, finite raw
/// scores, 2..=20 bins and 1..=3600 seconds. Rows are sorted by exact ID. The smoothed Platt
/// likelihood is unpenalized; held-out diagnostic regressions retain ridge 1e-8. Each regression
/// permits 1000 Newton iterations with gradient 1e-10 or relative objective 1e-14 termination.
/// Errors cover admission, finite arithmetic, convergence, 16 MiB output and cooperative timeout.
/// Uses O(n) memory. Applications requiring hard cancellation use the native child boundary.
pub fn fit_prediction_calibration(
    spec: PredictionCalibrationSpec,
) -> Result<PredictionCalibrationFit, BayesError> {
    let start = Instant::now();
    let spec = spec.validated()?;
    let deadline = start + Duration::from_secs(spec.timeout_seconds);
    let implementation_sha256 = crate::sha256_hex(
        concat!(
            include_str!("native.rs"),
            include_str!("logistic.rs"),
            include_str!("../prediction_calibration.rs"),
            include_str!("../probability_transform.rs")
        )
        .as_bytes(),
    );
    let request_sha256 = input_identity(&implementation_sha256, &spec);
    let train = spec.rows.iter().filter(|r| r.split == "training_oof");
    let positives = train.clone().filter(|r| r.label == 1).count() as f64;
    let negatives = train.clone().count() as f64 - positives;
    let x = train.clone().map(|r| r.score).collect::<Vec<_>>();
    let targets = train
        .map(|r| {
            if r.label == 1 {
                (positives + 1.) / (positives + 2.)
            } else {
                1. / (negatives + 2.)
            }
        })
        .collect::<Vec<_>>();
    let [intercept, slope] = logistic::fit(
        &x,
        &targets,
        [((positives + 1.) / (negatives + 1.)).ln(), 0.],
        0.,
        false,
        deadline,
    )?;
    let mut predictions = Vec::new();
    let mut logits = Vec::new();
    let mut labels = Vec::new();
    for row in spec.rows.iter().filter(|r| r.split == "test") {
        logistic::check_deadline(deadline)?;
        let z = intercept + slope * row.score;
        // The reference permits an overflowing held-out product: expit(±inf) is 1 or 0.
        let probability = sigmoid(z);
        if !probability.is_finite() {
            return Err(BayesError::InvalidSpec(
                "calibration prediction is nonfinite".into(),
            ));
        }
        // Preserve the reference's independently clipped numerator and denominator.
        logits.push(
            (probability.clamp(1e-12, 1. - 1e-12) / (1. - probability).clamp(1e-12, 1.)).ln(),
        );
        labels.push(f64::from(row.label));
        predictions.push(CalibratedPrediction {
            patient_id: row.patient_id.clone(),
            score: row.score,
            label: row.label,
            probability,
        });
    }
    let calibration_in_the_large =
        logistic::fit(&logits, &labels, [0., 1.], 1e-8, true, deadline)?[0];
    let calibration_slope = logistic::fit(&logits, &labels, [0., 1.], 1e-8, false, deadline)?[1];
    let (reliability_bins, expected_calibration_error, brier_score) =
        reliability(&predictions, spec.bins);
    let fit = PredictionCalibrationFit {
        format: "marklab.prediction_calibration",
        version: 2,
        backend: NativeCalibrationBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        method: "platt_logistic",
        fit_split: "training_oof",
        evaluation_split: "test",
        calibrator: PlattCalibrator {
            intercept,
            slope,
            target_smoothing: "platt_class_count".into(),
        },
        predictions,
        metrics: PredictionCalibrationMetrics {
            brier_score,
            expected_calibration_error,
            calibration_in_the_large,
            calibration_slope,
            reliability_bins,
        },
    };
    if serde_json::to_vec(&fit)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec(
            "calibration result exceeds 16 MiB".into(),
        ));
    }
    logistic::check_deadline(deadline)?;
    Ok(fit)
}

fn reliability(predictions: &[CalibratedPrediction], bins: u32) -> (Vec<ReliabilityBin>, f64, f64) {
    let mut counts = vec![(0_u32, 0., 0.); bins as usize];
    let mut squared = 0.;
    for row in predictions {
        let index = ((row.probability * f64::from(bins)).floor() as usize).min(bins as usize - 1);
        let bin = &mut counts[index];
        bin.0 += 1;
        bin.1 += row.probability;
        bin.2 += f64::from(row.label);
        squared += (row.probability - f64::from(row.label)).powi(2);
    }
    let mut ece = 0.;
    let mut rows = Vec::new();
    let z = 1.959963984540054_f64;
    for (index, (count, sum, positive)) in counts.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        let n = f64::from(count);
        let mean = sum / n;
        let rate = positive / n;
        let denominator = 1. + z * z / n;
        let center = (rate + z * z / (2. * n)) / denominator;
        let half = z * ((rate * (1. - rate) + z * z / (4. * n)) / n).sqrt() / denominator;
        ece += n / predictions.len() as f64 * (mean - rate).abs();
        rows.push(ReliabilityBin {
            lower: index as f64 / f64::from(bins),
            upper: (index + 1) as f64 / f64::from(bins),
            count,
            mean_probability: mean,
            observed_rate: rate,
            wilson_95_lower: (center - half).max(0.),
            wilson_95_upper: (center + half).min(1.),
        });
    }
    (rows, ece, squared / predictions.len() as f64)
}

fn input_identity(implementation: &str, spec: &PredictionCalibrationSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.prediction-calibration.spec.v1\0");
    hash_text(&mut hash, implementation);
    hash.update(spec.bins.to_le_bytes());
    hash.update(spec.timeout_seconds.to_le_bytes());
    hash.update((spec.rows.len() as u64).to_le_bytes());
    for row in &spec.rows {
        hash_text(&mut hash, &row.patient_id);
        hash_text(&mut hash, &row.split);
        hash.update(row.score.to_bits().to_le_bytes());
        hash.update([row.label]);
    }
    format!("{:x}", hash.finalize())
}
fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}
