use std::collections::BTreeMap;

use super::{
    compensated_sum, derive_seed_in_namespace, splitmix64, CohortInferenceError,
    PermutationAlternative, MAXIMUM_PATIENTS, MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS,
    MAXIMUM_PERMUTATIONS,
};

const PAIRED_SIGN_FLIP_NAMESPACE: u64 = 0x7061_6972_5f73_6967;

/// One condition-specific scalar endpoint belonging to one patient pair.
#[derive(Clone, Debug)]
pub struct PairedPatientEndpoint {
    /// Stable patient identifier supplied by the caller.
    pub patient_id: String,
    /// Exact declared condition label.
    pub condition: String,
    /// Prespecified finite scalar endpoint.
    pub endpoint: f64,
}

/// Frozen design and deterministic settings for paired scalar sign-flip inference.
#[derive(Clone, Debug)]
pub struct PairedPatientPermutationSpec {
    /// First condition label; effects are condition B minus condition A.
    pub condition_a: String,
    /// Second condition label.
    pub condition_b: String,
    /// Positive bounded number of requested sign-flip replicates.
    pub permutations: usize,
    /// Base seed for domain-separated replicate streams.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: PermutationAlternative,
}

/// Summary of one condition across complete patient pairs.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedConditionSummary {
    /// Exact declared condition label.
    pub label: String,
    /// Stable arithmetic mean over complete pairs.
    pub mean: f64,
}

/// Cohort-valid paired patient sign-flip result.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedPatientPermutationResult {
    /// Number of complete independent patient pairs.
    pub pair_count: usize,
    /// Condition A summary.
    pub condition_a: PairedConditionSummary,
    /// Condition B summary.
    pub condition_b: PairedConditionSummary,
    /// Mean of exact condition-B-minus-condition-A patient differences.
    pub effect_condition_b_minus_condition_a: f64,
    /// Studentized mean paired difference.
    pub studentized_statistic: f64,
    /// Inclusive-plus-one permutation p-value.
    pub p_value: f64,
    /// Requested replicate count.
    pub permutations_requested: usize,
    /// Attempted replicate count.
    pub permutations_attempted: usize,
    /// Successfully completed replicate count.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: PermutationAlternative,
}

/// Compare two complete scalar observations per patient using deterministic sign flips.
pub fn paired_patient_permutation_test(
    records: &[PairedPatientEndpoint],
    spec: &PairedPatientPermutationSpec,
) -> Result<PairedPatientPermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    let pairs = build_pairs(records, spec)?;
    let evaluations = pairs.len().checked_mul(spec.permutations).ok_or_else(|| {
        CohortInferenceError::InvalidInput(
            "pair-by-permutation work exceeds the supported limit".into(),
        )
    })?;
    if evaluations > MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "pair-by-permutation work exceeds the {MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS}-evaluation limit"
        )));
    }

    let condition_a_values = pairs
        .iter()
        .map(|pair| pair.condition_a)
        .collect::<Vec<_>>();
    let condition_b_values = pairs
        .iter()
        .map(|pair| pair.condition_b)
        .collect::<Vec<_>>();
    let differences = pairs
        .iter()
        .map(|pair| pair.condition_b - pair.condition_a)
        .collect::<Vec<_>>();
    if differences.iter().any(|difference| !difference.is_finite()) {
        return Err(CohortInferenceError::NumericalFailure(
            "paired subtraction produced a non-finite difference".into(),
        ));
    }
    let observed = studentized_mean(&differences)?;

    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    let mut signed = vec![0.0; differences.len()];
    for replicate in 0..spec.permutations {
        let mut state = derive_seed_in_namespace(spec.seed, PAIRED_SIGN_FLIP_NAMESPACE, replicate);
        for (index, (target, difference)) in signed.iter_mut().zip(&differences).enumerate() {
            state = splitmix64(state ^ index as u64);
            *target = if state & 1 == 0 {
                *difference
            } else {
                -*difference
            };
        }
        let statistic = studentized_mean(&signed)?;
        lower_tail += usize::from(statistic.statistic <= observed.statistic);
        upper_tail += usize::from(statistic.statistic >= observed.statistic);
    }
    let denominator = (spec.permutations + 1) as f64;
    let p_value = match spec.alternative {
        PermutationAlternative::Less => (lower_tail as f64 + 1.0) / denominator,
        PermutationAlternative::Greater => (upper_tail as f64 + 1.0) / denominator,
        PermutationAlternative::TwoSided => {
            (2.0 * ((lower_tail.min(upper_tail) as f64 + 1.0) / denominator)).min(1.0)
        }
    };

    Ok(PairedPatientPermutationResult {
        pair_count: pairs.len(),
        condition_a: PairedConditionSummary {
            label: spec.condition_a.clone(),
            mean: stable_mean(&condition_a_values),
        },
        condition_b: PairedConditionSummary {
            label: spec.condition_b.clone(),
            mean: stable_mean(&condition_b_values),
        },
        effect_condition_b_minus_condition_a: observed.mean,
        studentized_statistic: observed.statistic,
        p_value,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
        alternative: spec.alternative,
    })
}

fn validate_spec(spec: &PairedPatientPermutationSpec) -> Result<(), CohortInferenceError> {
    if spec.condition_a.trim().is_empty() || spec.condition_b.trim().is_empty() {
        return Err(CohortInferenceError::InvalidInput(
            "condition labels must be non-empty".into(),
        ));
    }
    if spec.condition_a.trim() != spec.condition_a || spec.condition_b.trim() != spec.condition_b {
        return Err(CohortInferenceError::InvalidInput(
            "condition labels may not have surrounding whitespace".into(),
        ));
    }
    if spec.condition_a == spec.condition_b {
        return Err(CohortInferenceError::InvalidInput(
            "condition labels must be distinct".into(),
        ));
    }
    if spec.permutations == 0 || spec.permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default)]
struct PairBuilder {
    condition_a: Option<f64>,
    condition_b: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct Pair {
    condition_a: f64,
    condition_b: f64,
}

fn build_pairs(
    records: &[PairedPatientEndpoint],
    spec: &PairedPatientPermutationSpec,
) -> Result<Vec<Pair>, CohortInferenceError> {
    if records.len() > MAXIMUM_PATIENTS.saturating_mul(2) {
        return Err(CohortInferenceError::InvalidInput(format!(
            "input exceeds the {}-row paired limit",
            MAXIMUM_PATIENTS * 2
        )));
    }
    let mut builders = BTreeMap::<&str, PairBuilder>::new();
    for record in records {
        let patient_id = record.patient_id.trim();
        if patient_id.is_empty() {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty".into(),
            ));
        }
        if patient_id != record.patient_id {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient_id may not have surrounding whitespace: {:?}",
                record.patient_id
            )));
        }
        if !record.endpoint.is_finite() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {patient_id} has a non-finite endpoint"
            )));
        }
        let builder = builders.entry(patient_id).or_default();
        let slot = if record.condition == spec.condition_a {
            &mut builder.condition_a
        } else if record.condition == spec.condition_b {
            &mut builder.condition_b
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {patient_id} has undeclared condition {:?}",
                record.condition
            )));
        };
        if slot.replace(record.endpoint).is_some() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {patient_id} has a duplicate condition row"
            )));
        }
    }
    if builders.len() < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "paired inference requires at least two complete patient pairs".into(),
        ));
    }
    builders
        .into_iter()
        .map(
            |(patient_id, builder)| match (builder.condition_a, builder.condition_b) {
                (Some(condition_a), Some(condition_b)) => Ok(Pair {
                    condition_a,
                    condition_b,
                }),
                _ => Err(CohortInferenceError::InvalidInput(format!(
                    "patient {patient_id} is missing a declared condition"
                ))),
            },
        )
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct StudentizedMean {
    mean: f64,
    statistic: f64,
}

fn studentized_mean(values: &[f64]) -> Result<StudentizedMean, CohortInferenceError> {
    let mean = stable_mean(values);
    let variance = compensated_sum(values.iter().map(|value| {
        let centered = value - mean;
        centered * centered
    })) / (values.len() - 1) as f64;
    let standard_error = (variance / values.len() as f64).sqrt();
    if !standard_error.is_finite() || standard_error == 0.0 {
        return Err(CohortInferenceError::NumericalFailure(
            "paired contrast has zero or non-finite standard error".into(),
        ));
    }
    let statistic = mean / standard_error;
    if !mean.is_finite() || !statistic.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "paired contrast produced a non-finite result".into(),
        ));
    }
    Ok(StudentizedMean { mean, statistic })
}

fn stable_mean(values: &[f64]) -> f64 {
    let scale = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return 0.0;
    }
    (compensated_sum(values.iter().map(|value| value / scale)) / values.len() as f64) * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> Vec<PairedPatientEndpoint> {
        [
            ("p-1", 1.0, 3.0),
            ("p-2", 2.0, 5.0),
            ("p-3", 3.0, 7.0),
            ("p-4", 4.0, 9.0),
        ]
        .into_iter()
        .flat_map(|(patient, a, b)| {
            [
                PairedPatientEndpoint {
                    patient_id: patient.into(),
                    condition: "A".into(),
                    endpoint: a,
                },
                PairedPatientEndpoint {
                    patient_id: patient.into(),
                    condition: "B".into(),
                    endpoint: b,
                },
            ]
        })
        .collect()
    }

    fn spec() -> PairedPatientPermutationSpec {
        PairedPatientPermutationSpec {
            condition_a: "A".into(),
            condition_b: "B".into(),
            permutations: 99,
            seed: 23,
            alternative: PermutationAlternative::Greater,
        }
    }

    #[test]
    fn paired_result_matches_hand_oracle() {
        let result = paired_patient_permutation_test(&records(), &spec()).expect("result");
        assert_eq!(result.condition_a.mean, 2.5);
        assert_eq!(result.condition_b.mean, 6.0);
        assert_eq!(result.effect_condition_b_minus_condition_a, 3.5);
        assert!((result.studentized_statistic - 5.422_176_684_690_384).abs() < 1e-12);
        assert_eq!(result.pair_count, 4);
    }

    #[test]
    fn incomplete_and_duplicate_pairs_are_rejected() {
        let mut incomplete = records();
        incomplete.pop();
        assert!(
            matches!(paired_patient_permutation_test(&incomplete, &spec()), Err(CohortInferenceError::InvalidInput(message)) if message.contains("missing"))
        );
        let mut duplicate = records();
        duplicate.push(duplicate[0].clone());
        assert!(
            matches!(paired_patient_permutation_test(&duplicate, &spec()), Err(CohortInferenceError::InvalidInput(message)) if message.contains("duplicate"))
        );
    }
}
