use std::collections::HashSet;

use super::{
    compensated_sum, CohortInferenceError, InferenceDesign, MAXIMUM_PATIENTS, MAXIMUM_PERMUTATIONS,
};

const MMD_NAMESPACE: u64 = 0x6d6d_645f_7065_726d;
const MAXIMUM_KERNEL_ELEMENTS: usize = 25_000_000;
const MAXIMUM_MMD_EVALUATIONS: usize = 100_000_000;

/// One complete finite fingerprint for one independent patient.
#[derive(Clone, Debug)]
pub struct Fingerprint {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Exact ordered feature names shared by all patients.
    pub features: Vec<String>,
    /// Finite values aligned to `features`.
    pub values: Vec<f64>,
}

/// Prespecified positive-semidefinite kernel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MmdKernel {
    /// Ordinary finite dot product.
    Linear,
    /// Gaussian radial-basis kernel with fixed positive bandwidth.
    Rbf { bandwidth: f64 },
}

/// MMD-squared estimator convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmdEstimator {
    /// U-statistic excluding within-group diagonal terms.
    Unbiased,
    /// V-statistic including within-group diagonal terms.
    Biased,
}

/// Frozen patient-level MMD design.
#[derive(Clone, Debug)]
pub struct MmdPermutationSpec {
    /// First group label.
    pub group_a: String,
    /// Second group label.
    pub group_b: String,
    /// Frozen kernel.
    pub kernel: MmdKernel,
    /// Frozen estimator convention.
    pub estimator: MmdEstimator,
    /// Positive bounded permutation count.
    pub permutations: usize,
    /// Base deterministic seed.
    pub seed: u64,
}

/// Patient-level MMD permutation result.
#[derive(Clone, Debug, PartialEq)]
pub struct MmdPermutationResult {
    /// Group A patient count.
    pub group_a_count: usize,
    /// Group B patient count.
    pub group_b_count: usize,
    /// Feature dimension.
    pub feature_count: usize,
    /// Frozen kernel.
    pub kernel: MmdKernel,
    /// Frozen estimator convention.
    pub estimator: MmdEstimator,
    /// Observed MMD-squared statistic.
    pub mmd_squared: f64,
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

/// Compare two patient fingerprint distributions using a frozen reusable kernel matrix.
pub fn patient_level_mmd(
    fingerprints: &[Fingerprint],
    spec: &MmdPermutationSpec,
) -> Result<MmdPermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    validate_fingerprints(fingerprints, &spec.group_a, &spec.group_b)?;
    let matrix_elements = fingerprints
        .len()
        .checked_mul(fingerprints.len())
        .ok_or_else(kernel_limit_error)?;
    if matrix_elements > MAXIMUM_KERNEL_ELEMENTS {
        return Err(kernel_limit_error());
    }
    let work = matrix_elements
        .checked_mul(spec.permutations + 1)
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_MMD_EVALUATIONS {
        return Err(work_limit_error());
    }
    let kernel = build_kernel_matrix(fingerprints, spec.kernel)?;
    let observed_labels = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = mmd_squared(
        &kernel,
        fingerprints.len(),
        &observed_labels,
        spec.estimator,
    )?;
    let design = InferenceDesign::population_independence(
        fingerprints.len(),
        spec.permutations,
        spec.seed,
        MMD_NAMESPACE,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let labels = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
            .iter()
            .map(|source| observed_labels[*source])
            .collect::<Vec<_>>();
        let statistic = mmd_squared(&kernel, fingerprints.len(), &labels, spec.estimator)?;
        exceedances += usize::from(statistic >= observed);
    }
    let group_a_count = observed_labels.iter().filter(|label| **label).count();
    Ok(MmdPermutationResult {
        group_a_count,
        group_b_count: fingerprints.len() - group_a_count,
        feature_count: fingerprints[0].features.len(),
        kernel: spec.kernel,
        estimator: spec.estimator,
        mmd_squared: observed,
        p_value: (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
    })
}

fn validate_spec(spec: &MmdPermutationSpec) -> Result<(), CohortInferenceError> {
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
    if let MmdKernel::Rbf { bandwidth } = spec.kernel {
        if !(bandwidth.is_finite() && bandwidth > 0.0) {
            return Err(CohortInferenceError::InvalidInput(
                "RBF bandwidth must be finite and positive".into(),
            ));
        }
    }
    if spec.permutations == 0 || spec.permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_fingerprints(
    fingerprints: &[Fingerprint],
    group_a: &str,
    group_b: &str,
) -> Result<(), CohortInferenceError> {
    if fingerprints.is_empty() || fingerprints.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "MMD requires 1 to {MAXIMUM_PATIENTS} patient fingerprints"
        )));
    }
    let features = &fingerprints[0].features;
    if features.is_empty() || features.iter().any(|feature| feature.trim().is_empty()) {
        return Err(CohortInferenceError::InvalidInput(
            "MMD feature names must be non-empty".into(),
        ));
    }
    if features.iter().collect::<HashSet<_>>().len() != features.len() {
        return Err(CohortInferenceError::InvalidInput(
            "MMD feature names must be unique".into(),
        ));
    }
    let mut patient_ids = HashSet::with_capacity(fingerprints.len());
    let mut group_a_count = 0usize;
    let mut group_b_count = 0usize;
    for fingerprint in fingerprints {
        if fingerprint.patient_id.trim().is_empty()
            || fingerprint.patient_id.trim() != fingerprint.patient_id
        {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if !patient_ids.insert(fingerprint.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate patient fingerprint: {}",
                fingerprint.patient_id
            )));
        }
        if fingerprint.group == group_a {
            group_a_count += 1;
        } else if fingerprint.group == group_b {
            group_b_count += 1;
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared group {:?}",
                fingerprint.patient_id, fingerprint.group
            )));
        }
        if fingerprint.features != *features || fingerprint.values.len() != features.len() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} does not have the exact complete feature set",
                fingerprint.patient_id
            )));
        }
        if fingerprint.values.iter().any(|value| !value.is_finite()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has a non-finite feature value",
                fingerprint.patient_id
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "each group must contain at least two patient fingerprints".into(),
        ));
    }
    Ok(())
}

fn build_kernel_matrix(
    fingerprints: &[Fingerprint],
    kernel: MmdKernel,
) -> Result<Vec<f64>, CohortInferenceError> {
    let n = fingerprints.len();
    let mut matrix = vec![0.0; n * n];
    for left in 0..n {
        for right in left..n {
            let value = match kernel {
                MmdKernel::Linear => compensated_sum(
                    fingerprints[left]
                        .values
                        .iter()
                        .zip(&fingerprints[right].values)
                        .map(|(a, b)| a * b),
                ),
                MmdKernel::Rbf { bandwidth } => {
                    let squared_distance =
                        squared_euclidean(&fingerprints[left].values, &fingerprints[right].values);
                    if squared_distance.is_infinite() {
                        0.0
                    } else {
                        (-squared_distance / (2.0 * bandwidth * bandwidth)).exp()
                    }
                }
            };
            if !value.is_finite() {
                return Err(CohortInferenceError::NumericalFailure(
                    "kernel evaluation produced a non-finite value".into(),
                ));
            }
            matrix[left * n + right] = value;
            matrix[right * n + left] = value;
        }
    }
    Ok(matrix)
}

pub(crate) fn squared_euclidean(left: &[f64], right: &[f64]) -> f64 {
    let mut scale = 0.0_f64;
    let mut sum_squares = 1.0_f64;
    for (a, b) in left.iter().zip(right) {
        let difference = (a - b).abs();
        if difference.is_infinite() {
            return f64::INFINITY;
        }
        if difference != 0.0 {
            if scale < difference {
                let ratio = scale / difference;
                sum_squares = 1.0 + sum_squares * ratio * ratio;
                scale = difference;
            } else {
                let ratio = difference / scale;
                sum_squares += ratio * ratio;
            }
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * scale * sum_squares
    }
}

fn mmd_squared(
    kernel: &[f64],
    n: usize,
    labels: &[bool],
    estimator: MmdEstimator,
) -> Result<f64, CohortInferenceError> {
    let group_a = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| label.then_some(index))
        .collect::<Vec<_>>();
    let group_b = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| (!label).then_some(index))
        .collect::<Vec<_>>();
    let within_a = within_mean(kernel, n, &group_a, estimator);
    let within_b = within_mean(kernel, n, &group_b, estimator);
    let cross = compensated_sum(
        group_a
            .iter()
            .flat_map(|left| group_b.iter().map(move |right| kernel[left * n + right])),
    ) / (group_a.len() * group_b.len()) as f64;
    let value = within_a + within_b - 2.0 * cross;
    if !value.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "MMD statistic produced a non-finite value".into(),
        ));
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn within_mean(kernel: &[f64], n: usize, indices: &[usize], estimator: MmdEstimator) -> f64 {
    let include_diagonal = estimator == MmdEstimator::Biased;
    let sum = compensated_sum(indices.iter().flat_map(|left| {
        indices.iter().filter_map(move |right| {
            (include_diagonal || left != right).then_some(kernel[left * n + right])
        })
    }));
    let denominator = if include_diagonal {
        indices.len() * indices.len()
    } else {
        indices.len() * (indices.len() - 1)
    };
    sum / denominator as f64
}

fn kernel_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "MMD kernel matrix exceeds the {MAXIMUM_KERNEL_ELEMENTS}-element limit"
    ))
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "MMD kernel-by-permutation work exceeds the {MAXIMUM_MMD_EVALUATIONS}-evaluation limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_unbiased_mmd_matches_hand_oracle() {
        let fingerprints = [
            ("a-1", "A", 3.0),
            ("a-2", "A", 5.0),
            ("b-1", "B", 0.0),
            ("b-2", "B", 1.0),
        ]
        .into_iter()
        .map(|(patient, group, x)| Fingerprint {
            patient_id: patient.into(),
            group: group.into(),
            features: vec!["x".into(), "y".into()],
            values: vec![x, 0.0],
        })
        .collect::<Vec<_>>();
        let result = patient_level_mmd(
            &fingerprints,
            &MmdPermutationSpec {
                group_a: "A".into(),
                group_b: "B".into(),
                kernel: MmdKernel::Linear,
                estimator: MmdEstimator::Unbiased,
                permutations: 99,
                seed: 47,
            },
        )
        .expect("MMD result");
        assert_eq!(result.mmd_squared, 11.0);
    }

    #[test]
    fn rbf_kernel_has_unit_diagonal() {
        let fingerprints = [
            ("a-1", "A", 3.0),
            ("a-2", "A", 5.0),
            ("b-1", "B", 0.0),
            ("b-2", "B", 1.0),
        ]
        .into_iter()
        .map(|(patient, group, x)| Fingerprint {
            patient_id: patient.into(),
            group: group.into(),
            features: vec!["x".into()],
            values: vec![x],
        })
        .collect::<Vec<_>>();
        let matrix =
            build_kernel_matrix(&fingerprints, MmdKernel::Rbf { bandwidth: 2.0 }).expect("kernel");
        for index in 0..fingerprints.len() {
            assert_eq!(matrix[index * fingerprints.len() + index], 1.0);
        }
    }
}
