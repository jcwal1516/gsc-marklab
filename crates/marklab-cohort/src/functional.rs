use std::collections::HashSet;

use super::{
    compensated_sum, inference_design::compile_blocked_population_independence,
    CohortInferenceError, InferenceDesign, PatientExchangeabilityBlock, MAXIMUM_PATIENTS,
    MAXIMUM_PERMUTATIONS,
};

const FUNCTIONAL_PERMUTATION_NAMESPACE: u64 = 0x6675_6e63_5f70_6572;
const MAXIMUM_FUNCTIONAL_EVALUATIONS: usize = 100_000_000;

/// One common-axis finite curve for one independent patient.
#[derive(Clone, Debug)]
pub struct FunctionalCurve {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Strictly increasing common physical axis.
    pub axis: Vec<f64>,
    /// Finite curve values aligned to `axis`.
    pub values: Vec<f64>,
}

/// Joint curve statistic used by a functional permutation test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionalTestStatistic {
    /// Trapezoidal integral of the squared group-mean difference.
    L2,
}

/// Frozen independent-groups functional permutation design.
#[derive(Clone, Debug)]
pub struct FunctionalPermutationSpec {
    /// First group label; differences are group A minus group B.
    pub group_a: String,
    /// Second group label.
    pub group_b: String,
    /// Joint statistic.
    pub statistic: FunctionalTestStatistic,
    /// Positive bounded permutation count.
    pub permutations: usize,
    /// Base deterministic seed.
    pub seed: u64,
}

/// Patient-level functional two-sample permutation result.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionalPermutationResult {
    /// Exact common axis.
    pub axis: Vec<f64>,
    /// Group A patient count.
    pub group_a_count: usize,
    /// Group B patient count.
    pub group_b_count: usize,
    /// Pointwise group A mean.
    pub group_a_mean: Vec<f64>,
    /// Pointwise group B mean.
    pub group_b_mean: Vec<f64>,
    /// Pointwise group-A-minus-group-B difference.
    pub observed_difference: Vec<f64>,
    /// Joint statistic.
    pub statistic: FunctionalTestStatistic,
    /// Observed joint-statistic value.
    pub observed_statistic: f64,
    /// Inclusive-plus-one one-sided-high p-value.
    pub p_value: f64,
    /// Requested replicate count.
    pub permutations_requested: usize,
    /// Attempted replicate count.
    pub permutations_attempted: usize,
    /// Completed replicate count.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
}

/// Functional permutation result together with its exact exchangeability design.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockedFunctionalPermutationResult {
    result: FunctionalPermutationResult,
    design: InferenceDesign,
}

impl BlockedFunctionalPermutationResult {
    /// Functional result computed under the blocked design.
    pub fn result(&self) -> &FunctionalPermutationResult {
        &self.result
    }

    /// Exact patient-level exchangeability design used for every replicate.
    pub fn design(&self) -> &InferenceDesign {
        &self.design
    }

    /// Consume the wrapper into its result and design.
    pub fn into_parts(self) -> (FunctionalPermutationResult, InferenceDesign) {
        (self.result, self.design)
    }
}

/// Compare one common-axis curve per patient using whole-patient label permutations.
pub fn functional_two_sample_permutation(
    curves: &[FunctionalCurve],
    spec: &FunctionalPermutationSpec,
) -> Result<FunctionalPermutationResult, CohortInferenceError> {
    validate_functional_inputs(curves, spec)?;
    let design = InferenceDesign::population_independence(
        curves.len(),
        spec.permutations,
        spec.seed,
        FUNCTIONAL_PERMUTATION_NAMESPACE,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
    execute_functional(curves, spec, &design)
}

/// Compare one common-axis curve per patient within exact exchangeability blocks.
pub fn functional_two_sample_blocked_permutation(
    curves: &[FunctionalCurve],
    assignments: &[PatientExchangeabilityBlock],
    spec: &FunctionalPermutationSpec,
) -> Result<BlockedFunctionalPermutationResult, CohortInferenceError> {
    validate_functional_inputs(curves, spec)?;
    let observed_labels = curves
        .iter()
        .map(|curve| curve.group == spec.group_a)
        .collect::<Vec<_>>();
    let patient_ids = curves
        .iter()
        .map(|curve| curve.patient_id.clone())
        .collect::<Vec<_>>();
    let design = compile_blocked_population_independence(
        &patient_ids,
        &observed_labels,
        assignments,
        spec.permutations,
        spec.seed,
        FUNCTIONAL_PERMUTATION_NAMESPACE,
        crate::InferenceAlternative::Greater,
    )?;
    let result = execute_functional(curves, spec, &design)?;
    Ok(BlockedFunctionalPermutationResult { result, design })
}

fn validate_functional_inputs(
    curves: &[FunctionalCurve],
    spec: &FunctionalPermutationSpec,
) -> Result<(), CohortInferenceError> {
    validate_spec(spec)?;
    validate_curves(curves, spec)?;
    let axis_len = curves[0].axis.len();
    let work = curves
        .len()
        .checked_mul(axis_len)
        .and_then(|value| value.checked_mul(spec.permutations + 1))
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_FUNCTIONAL_EVALUATIONS {
        return Err(work_limit_error());
    }
    Ok(())
}

fn execute_functional(
    curves: &[FunctionalCurve],
    spec: &FunctionalPermutationSpec,
    design: &InferenceDesign,
) -> Result<FunctionalPermutationResult, CohortInferenceError> {
    let observed_labels = curves
        .iter()
        .map(|curve| curve.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = evaluate(curves, &observed_labels, spec.statistic)?;
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let labels = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
            .iter()
            .map(|source| observed_labels[*source])
            .collect::<Vec<_>>();
        let permuted = evaluate(curves, &labels, spec.statistic)?;
        exceedances += usize::from(permuted.statistic >= observed.statistic);
    }
    let p_value = (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64;

    Ok(FunctionalPermutationResult {
        axis: curves[0].axis.clone(),
        group_a_count: observed.group_a_count,
        group_b_count: observed.group_b_count,
        group_a_mean: observed.group_a_mean,
        group_b_mean: observed.group_b_mean,
        observed_difference: observed.difference,
        statistic: spec.statistic,
        observed_statistic: observed.statistic,
        p_value,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
    })
}

fn validate_spec(spec: &FunctionalPermutationSpec) -> Result<(), CohortInferenceError> {
    if spec.group_a.trim().is_empty() || spec.group_b.trim().is_empty() {
        return Err(CohortInferenceError::InvalidInput(
            "group labels must be non-empty".into(),
        ));
    }
    if spec.group_a.trim() != spec.group_a || spec.group_b.trim() != spec.group_b {
        return Err(CohortInferenceError::InvalidInput(
            "group labels may not have surrounding whitespace".into(),
        ));
    }
    if spec.group_a == spec.group_b {
        return Err(CohortInferenceError::InvalidInput(
            "group labels must be distinct".into(),
        ));
    }
    if spec.permutations == 0 || spec.permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    Ok(())
}

fn validate_curves(
    curves: &[FunctionalCurve],
    spec: &FunctionalPermutationSpec,
) -> Result<(), CohortInferenceError> {
    if curves.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "input exceeds the {MAXIMUM_PATIENTS}-patient limit"
        )));
    }
    if curves.is_empty() {
        return Err(CohortInferenceError::InvalidInput(
            "input must contain patient curves".into(),
        ));
    }
    let expected_axis = &curves[0].axis;
    if expected_axis.len() < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "functional inference requires at least two axis points".into(),
        ));
    }
    if expected_axis.iter().any(|value| !value.is_finite())
        || expected_axis.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(CohortInferenceError::InvalidInput(
            "functional axis must be finite and strictly increasing".into(),
        ));
    }
    let expected_axis_bits = expected_axis
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    let mut patients = HashSet::with_capacity(curves.len());
    let mut group_a_count = 0usize;
    let mut group_b_count = 0usize;
    for curve in curves {
        if curve.patient_id.trim().is_empty() || curve.patient_id.trim() != curve.patient_id {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if !patients.insert(curve.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate patient curve: {}",
                curve.patient_id
            )));
        }
        if curve.group == spec.group_a {
            group_a_count += 1;
        } else if curve.group == spec.group_b {
            group_b_count += 1;
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared group {:?}",
                curve.patient_id, curve.group
            )));
        }
        if curve.values.len() != expected_axis.len()
            || curve
                .axis
                .iter()
                .map(|value| value.to_bits())
                .ne(expected_axis_bits.iter().copied())
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} does not use the exact common axis",
                curve.patient_id
            )));
        }
        if curve.values.iter().any(|value| !value.is_finite()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} curve contains a non-finite value",
                curve.patient_id
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "each group must contain at least two patient curves".into(),
        ));
    }
    Ok(())
}

struct Evaluation {
    group_a_count: usize,
    group_b_count: usize,
    group_a_mean: Vec<f64>,
    group_b_mean: Vec<f64>,
    difference: Vec<f64>,
    statistic: f64,
}

fn evaluate(
    curves: &[FunctionalCurve],
    labels: &[bool],
    statistic: FunctionalTestStatistic,
) -> Result<Evaluation, CohortInferenceError> {
    let group_a_count = labels.iter().filter(|label| **label).count();
    let group_b_count = labels.len() - group_a_count;
    let axis_len = curves[0].axis.len();
    let mut group_a_mean = vec![0.0; axis_len];
    let mut group_b_mean = vec![0.0; axis_len];
    for axis_index in 0..axis_len {
        group_a_mean[axis_index] = compensated_sum(
            curves
                .iter()
                .zip(labels)
                .filter_map(|(curve, label)| label.then_some(curve.values[axis_index])),
        ) / group_a_count as f64;
        group_b_mean[axis_index] = compensated_sum(
            curves
                .iter()
                .zip(labels)
                .filter_map(|(curve, label)| (!label).then_some(curve.values[axis_index])),
        ) / group_b_count as f64;
    }
    let difference = group_a_mean
        .iter()
        .zip(&group_b_mean)
        .map(|(a, b)| a - b)
        .collect::<Vec<_>>();
    let value = match statistic {
        FunctionalTestStatistic::L2 => curves[0]
            .axis
            .windows(2)
            .zip(difference.windows(2))
            .map(|(axis, values)| {
                let width = axis[1] - axis[0];
                width * (values[0] * values[0] + values[1] * values[1]) / 2.0
            })
            .sum::<f64>(),
    };
    if difference.iter().any(|value| !value.is_finite()) || !value.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "functional statistic produced a non-finite result".into(),
        ));
    }
    Ok(Evaluation {
        group_a_count,
        group_b_count,
        group_a_mean,
        group_b_mean,
        difference,
        statistic: value,
    })
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "functional patient-by-axis-by-permutation work exceeds the {MAXIMUM_FUNCTIONAL_EVALUATIONS}-evaluation limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(patient: &str, group: &str, value: f64) -> FunctionalCurve {
        FunctionalCurve {
            patient_id: patient.into(),
            group: group.into(),
            axis: vec![0.0, 1.0, 2.0],
            values: vec![value; 3],
        }
    }

    #[test]
    fn l2_matches_hand_oracle() {
        let curves = vec![
            curve("a-1", "A", 2.0),
            curve("a-2", "A", 4.0),
            curve("b-1", "B", 0.0),
            curve("b-2", "B", 1.0),
        ];
        let result = functional_two_sample_permutation(
            &curves,
            &FunctionalPermutationSpec {
                group_a: "A".into(),
                group_b: "B".into(),
                statistic: FunctionalTestStatistic::L2,
                permutations: 99,
                seed: 31,
            },
        )
        .expect("functional result");
        assert_eq!(result.observed_difference, vec![2.5, 2.5, 2.5]);
        assert_eq!(result.observed_statistic, 12.5);
    }

    #[test]
    fn mismatched_axis_is_rejected() {
        let mut curves = vec![
            curve("a-1", "A", 2.0),
            curve("a-2", "A", 4.0),
            curve("b-1", "B", 0.0),
            curve("b-2", "B", 1.0),
        ];
        curves[3].axis[2] = 3.0;
        assert!(
            matches!(functional_two_sample_permutation(&curves, &FunctionalPermutationSpec {
            group_a: "A".into(), group_b: "B".into(), statistic: FunctionalTestStatistic::L2,
            permutations: 9, seed: 31,
        }), Err(CohortInferenceError::InvalidInput(message)) if message.contains("common axis"))
        );
    }
}
