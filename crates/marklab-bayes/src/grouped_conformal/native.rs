//! Native grouped-conformal scientific flow; no file or process discovery.
use super::{
    probability, CoverageRow, GroupedConformalModel, GroupedConformalPrediction,
    GroupedConformalSpec, GroupedCoverage,
};
use crate::BayesError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
#[path = "logistic.rs"]
mod logistic;

/// Native implementation identity; this never represents execution by a Python reference.
#[derive(Debug, Serialize)]
pub struct NativeConformalBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}

/// Native grouped-conformal result. Version 2 carries native provenance and unchanged science.
#[derive(Debug, Serialize)]
pub struct GroupedConformalFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativeConformalBackend,
    pub request_sha256: String,
    pub feature_names: Vec<String>,
    pub alpha: f64,
    pub fit_split: &'static str,
    pub quantile_split: &'static str,
    pub prediction_split: &'static str,
    pub model: GroupedConformalModel,
    pub calibration_count: u32,
    pub corrected_rank: u32,
    pub nonconformity_threshold: f64,
    pub predictions: Vec<GroupedConformalPrediction>,
    pub coverage: GroupedCoverage,
    pub claim_status: &'static str,
}

/// Fit training-only L2 logistic prediction and calibrate inclusive binary sets on separate patients.
///
/// Preserves the legacy admission, unpenalized intercept, population standardization, 1e-14
/// training-variation cutoff, 1000 total optimizer iterations and 1e-8 gradient criterion. Patient rows
/// are sorted by exact ID. Failures include invalid inputs, nonfinite arithmetic, nonconvergence,
/// result-size exhaustion and deadline exhaustion. No Python/environment/filesystem is needed.
/// Uses O(n*d+d²) memory; the caller's timeout also bounds the optimizer and prediction loops.
/// Applications needing forcibly killable deadlines must invoke this through a native child.
pub fn fit_grouped_conformal(
    spec: GroupedConformalSpec,
) -> Result<GroupedConformalFit, BayesError> {
    let start = Instant::now();
    let spec = spec.validated()?;
    let deadline = start + Duration::from_secs(spec.timeout_seconds);
    let mut identity = Sha256::new();
    identity.update(
        concat!(
            include_str!("native.rs"),
            include_str!("logistic.rs"),
            include_str!("../grouped_conformal.rs"),
            include_str!("../linalg.rs"),
            include_str!("../logistic_fit.rs")
        )
        .as_bytes(),
    );
    identity.update(marklab_numerics::BFGS_IMPLEMENTATION_SOURCE.as_bytes());
    let implementation_sha256 = format!("{:x}", identity.finalize());
    let request_sha256 = input_identity(&implementation_sha256, &spec);
    let model = logistic::fit(&spec, deadline)
        .map_err(|error| BayesError::InvalidSpec(error.to_string()))?;
    let mut scores = Vec::new();
    for p in spec.patients.iter().filter(|p| p.split == "calibration") {
        check_deadline(deadline)?;
        let probability = checked_probability(&model, &p.features)?;
        scores.push(if p.label == 1 {
            1.0 - probability
        } else {
            probability
        });
    }
    scores.sort_by(f64::total_cmp);
    let rank = ((scores.len() as f64 + 1.0) * (1.0 - spec.alpha)).ceil() as usize;
    let threshold = scores[rank - 1];
    let mut predictions = Vec::new();
    for p in spec.patients.iter().filter(|p| p.split == "test") {
        check_deadline(deadline)?;
        let probability_one = checked_probability(&model, &p.features)?;
        let prediction_set = [
            (probability_one <= threshold).then_some(0),
            (1.0 - probability_one <= threshold).then_some(1),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let covered = prediction_set.contains(&p.label);
        predictions.push(GroupedConformalPrediction {
            patient_id: p.patient_id.clone(),
            site: p.site.clone(),
            subgroup: p.subgroup.clone(),
            label: p.label,
            probability_one,
            prediction_set,
            covered,
        });
    }
    let coverage = GroupedCoverage {
        overall: coverage_row(
            "all",
            predictions.len() as u32,
            predictions.iter().filter(|p| p.covered).count() as u32,
        ),
        by_site: grouped_coverage(&predictions, |p| &p.site),
        by_subgroup: grouped_coverage(&predictions, |p| &p.subgroup),
    };
    let fit = GroupedConformalFit {
        format: "marklab.grouped_conformal_prediction",
        version: 2,
        backend: NativeConformalBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        feature_names: spec.feature_names,
        alpha: spec.alpha,
        fit_split: "train",
        quantile_split: "calibration",
        prediction_split: "test",
        model,
        calibration_count: scores.len() as u32,
        corrected_rank: rank as u32,
        nonconformity_threshold: threshold,
        predictions,
        coverage,
        claim_status: "exchangeability_conditional_coverage_not_guaranteed_under_shift",
    };
    if serde_json::to_vec(&fit)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec(
            "grouped conformal result exceeds 16 MiB".into(),
        ));
    }
    check_deadline(deadline)?;
    Ok(fit)
}

// Canonical semantic identity: length-delimited UTF-8 and little-endian integer/f64 bits.
// Admission has already fixed row order, matrix dimensions and finite values. Hash directly
// from borrowed inputs instead of allocating and formatting a second JSON copy of the matrix.
fn input_identity(implementation: &str, spec: &GroupedConformalSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.grouped-conformal.spec.v1\0");
    hash_text(&mut hash, implementation);
    hash.update((spec.feature_names.len() as u64).to_le_bytes());
    for name in &spec.feature_names {
        hash_text(&mut hash, name);
    }
    hash.update(spec.alpha.to_bits().to_le_bytes());
    hash.update(spec.l2_penalty.to_bits().to_le_bytes());
    hash.update(spec.timeout_seconds.to_le_bytes());
    hash.update((spec.patients.len() as u64).to_le_bytes());
    for patient in &spec.patients {
        for text in [
            &patient.patient_id,
            &patient.split,
            &patient.site,
            &patient.subgroup,
        ] {
            hash_text(&mut hash, text);
        }
        hash.update([patient.label]);
        for value in &patient.features {
            hash.update(value.to_bits().to_le_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}
fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}

fn checked_probability(model: &GroupedConformalModel, features: &[f64]) -> Result<f64, BayesError> {
    // Admission guarantees dimensions; derived standardization must also remain representable.
    if features
        .iter()
        .zip(&model.training_mean)
        .zip(&model.training_population_sd)
        .any(|((x, m), s)| !((x - m) / s).is_finite())
    {
        return Err(BayesError::InvalidSpec(
            "grouped conformal standardized prediction is nonfinite".into(),
        ));
    }
    let p = probability(model, features);
    if p.is_finite() {
        Ok(p)
    } else {
        Err(BayesError::InvalidSpec(
            "grouped conformal prediction is nonfinite".into(),
        ))
    }
}
fn check_deadline(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(BayesError::InvalidSpec(
            "grouped conformal deadline exceeded".into(),
        ))
    } else {
        Ok(())
    }
}
fn coverage_row(group: &str, count: u32, covered: u32) -> CoverageRow {
    CoverageRow {
        group: group.into(),
        count,
        covered,
        coverage: f64::from(covered) / f64::from(count),
    }
}
fn grouped_coverage<'a>(
    predictions: &'a [GroupedConformalPrediction],
    group: impl Fn(&'a GroupedConformalPrediction) -> &'a str,
) -> Vec<CoverageRow> {
    let mut rows = BTreeMap::<&str, (u32, u32)>::new();
    for p in predictions {
        let r = rows.entry(group(p)).or_default();
        r.0 += 1;
        r.1 += u32::from(p.covered);
    }
    rows.into_iter()
        .map(|(g, (n, k))| coverage_row(g, n, k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GroupedConformalPatient;

    #[test]
    fn prediction_standardizes_before_multiplication_to_avoid_spurious_overflow() {
        let model = GroupedConformalModel {
            training_mean: vec![0.0, 0.0],
            training_population_sd: vec![100.0, 100.0],
            intercept: 0.0,
            coefficients: vec![2.0, -2.0],
            l2_penalty: 0.1,
        };
        // The reference first forms [1e306,1e306]; its finite products cancel to zero.
        assert_eq!(checked_probability(&model, &[1e308, 1e308]).unwrap(), 0.5);
    }

    #[test]
    fn typed_request_identity_matches_independent_struct_pack_oracle() {
        let mut spec = GroupedConformalSpec {
            patients: vec![GroupedConformalPatient {
                patient_id: "a|b\0".into(),
                split: "train".into(),
                site: "site".into(),
                subgroup: "group".into(),
                label: 1,
                features: vec![-0.0, 1.5],
            }],
            feature_names: vec!["feature_x".into(), "feature_y".into()],
            alpha: 0.2,
            l2_penalty: 0.1,
            timeout_seconds: 120,
        };
        let hash = input_identity("reference-build", &spec);
        assert_eq!(
            hash,
            "26eb390703914d0ecbad7c2059da56ab782c6c88b1c81e696b2b5194d3e61111"
        );
        spec.patients[0].features[0] = 0.0;
        assert_ne!(hash, input_identity("reference-build", &spec));
        assert_ne!(
            input_identity("other-build", &spec),
            input_identity("reference-build", &spec)
        );
    }
}
