use std::collections::HashSet;

use super::{
    derive_seed_in_namespace, numeric::stable_mean, splitmix64, CohortInferenceError,
    MAXIMUM_PATIENTS, MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS, MAXIMUM_PERMUTATIONS,
};

const FUNCTIONAL_EQUIVALENCE_NAMESPACE: u64 = 0x6675_6e63_5f65_7176;

#[derive(Clone, Debug)]
pub struct FunctionalDifferenceCurve {
    pub patient_id: String,
    pub axis: Vec<f64>,
    pub differences: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct FunctionalEquivalenceSpec {
    pub margin_curve: Vec<f64>,
    pub alpha: f64,
    pub replicates: usize,
    pub seed: u64,
    pub margin_rationale: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FunctionalEquivalencePoint {
    pub axis: f64,
    pub mean_difference: f64,
    pub lower_band: f64,
    pub upper_band: f64,
    pub margin: f64,
    pub equivalent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionalEquivalenceResult {
    pub patient_count: usize,
    pub simultaneous_level: f64,
    pub max_deviation_critical: f64,
    pub points: Vec<FunctionalEquivalencePoint>,
    pub failing_scales: Vec<f64>,
    pub equivalent_at_all_scales: bool,
    pub replicates_requested: usize,
    pub replicates_completed: usize,
    pub seed: u64,
    pub margin_rationale: String,
}

pub fn functional_equivalence_band(
    curves: &[FunctionalDifferenceCurve],
    spec: &FunctionalEquivalenceSpec,
) -> Result<FunctionalEquivalenceResult, CohortInferenceError> {
    validate_spec(spec)?;
    let (axis, values) = validate_curves(curves, &spec.margin_curve)?;
    let evaluations = curves
        .len()
        .checked_mul(axis.len())
        .and_then(|value| value.checked_mul(spec.replicates))
        .ok_or_else(work_error)?;
    if evaluations > MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS {
        return Err(work_error());
    }
    let observed = (0..axis.len())
        .map(|column| stable_mean(&values.iter().map(|row| row[column]).collect::<Vec<_>>()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut max_deviations = Vec::with_capacity(spec.replicates);
    let mut sampled = vec![0.0; axis.len()];
    for replicate in 0..spec.replicates {
        sampled.fill(0.0);
        let mut state =
            derive_seed_in_namespace(spec.seed, FUNCTIONAL_EQUIVALENCE_NAMESPACE, replicate);
        for draw in 0..values.len() {
            state = splitmix64(state ^ draw as u64);
            let selected = &values[state as usize % values.len()];
            for (sum, value) in sampled.iter_mut().zip(selected) {
                *sum += value;
            }
        }
        let inverse = 1.0 / values.len() as f64;
        let maximum = sampled
            .iter()
            .zip(&observed)
            .map(|(sum, mean)| (sum * inverse - mean).abs())
            .fold(0.0_f64, f64::max);
        if !maximum.is_finite() {
            return Err(CohortInferenceError::NumericalFailure(
                "functional bootstrap produced a non-finite maximum deviation".into(),
            ));
        }
        max_deviations.push(maximum);
    }
    max_deviations.sort_by(f64::total_cmp);
    let rank = ((1.0 - spec.alpha) * spec.replicates as f64).ceil() as usize;
    let critical = max_deviations[rank.saturating_sub(1).min(spec.replicates - 1)];
    let points = axis
        .iter()
        .zip(observed)
        .zip(&spec.margin_curve)
        .map(|((&axis, mean_difference), &margin)| {
            let lower_band = mean_difference - critical;
            let upper_band = mean_difference + critical;
            FunctionalEquivalencePoint {
                axis,
                mean_difference,
                lower_band,
                upper_band,
                margin,
                equivalent: lower_band > -margin && upper_band < margin,
            }
        })
        .collect::<Vec<_>>();
    let failing_scales = points
        .iter()
        .filter_map(|point| (!point.equivalent).then_some(point.axis))
        .collect::<Vec<_>>();
    Ok(FunctionalEquivalenceResult {
        patient_count: curves.len(),
        simultaneous_level: 1.0 - spec.alpha,
        max_deviation_critical: critical,
        equivalent_at_all_scales: failing_scales.is_empty(),
        points,
        failing_scales,
        replicates_requested: spec.replicates,
        replicates_completed: spec.replicates,
        seed: spec.seed,
        margin_rationale: spec.margin_rationale.clone(),
    })
}

fn validate_spec(spec: &FunctionalEquivalenceSpec) -> Result<(), CohortInferenceError> {
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 0.5) {
        return Err(CohortInferenceError::InvalidInput(
            "functional equivalence alpha must be in (0, 0.5)".into(),
        ));
    }
    if spec.replicates == 0 || spec.replicates > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "bootstrap replicates must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    if spec.margin_rationale.is_empty()
        || spec.margin_rationale.trim() != spec.margin_rationale
        || spec.margin_curve.is_empty()
        || spec
            .margin_curve
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(CohortInferenceError::InvalidInput(
            "functional margins require positive finite values and an exact rationale".into(),
        ));
    }
    Ok(())
}

fn validate_curves(
    curves: &[FunctionalDifferenceCurve],
    margins: &[f64],
) -> Result<(Vec<f64>, Vec<Vec<f64>>), CohortInferenceError> {
    if !(4..=MAXIMUM_PATIENTS).contains(&curves.len()) {
        return Err(CohortInferenceError::InvalidInput(
            "functional equivalence requires at least four patient curves".into(),
        ));
    }
    let axis = curves[0].axis.clone();
    if axis.len() != margins.len()
        || axis.iter().any(|value| !value.is_finite())
        || axis.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(CohortInferenceError::InvalidInput(
            "functional axis and margin curve must be finite, aligned, and increasing".into(),
        ));
    }
    let mut patients = HashSet::new();
    let mut values = Vec::with_capacity(curves.len());
    for curve in curves {
        if curve.patient_id.is_empty()
            || curve.patient_id.trim() != curve.patient_id
            || !patients.insert(curve.patient_id.as_str())
            || curve.axis != axis
            || curve.differences.len() != axis.len()
            || curve.differences.iter().any(|value| !value.is_finite())
        {
            return Err(CohortInferenceError::InvalidInput(
                "functional patient curves must have unique IDs and one common finite axis".into(),
            ));
        }
        values.push(curve.differences.clone());
    }
    Ok((axis, values))
}

fn work_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "functional bootstrap exceeds the {MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS}-evaluation limit"
    ))
}
