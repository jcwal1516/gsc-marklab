use std::collections::HashSet;

use super::{
    inference_design::compile_blocked_population_independence, welch_contrast,
    CohortInferenceError, InferenceAlternative, InferenceDesign, PatientExchangeabilityBlock,
    MAXIMUM_PATIENTS, MAXIMUM_PERMUTATIONS,
};

const MAX_T_NAMESPACE: u64 = 0x6d61_785f_745f_7065;
const MAXIMUM_MAX_T_EVALUATIONS: usize = 100_000_000;

/// One complete prespecified endpoint vector for one independent patient.
#[derive(Clone, Debug)]
pub struct PatientEndpointVector {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Exact ordered endpoint names shared by all patients.
    pub endpoints: Vec<String>,
    /// Finite values aligned to `endpoints`.
    pub values: Vec<f64>,
}

/// Frozen independent-groups Max-T permutation design.
#[derive(Clone, Debug)]
pub struct MaxTPermutationSpec {
    /// First group label; effects are group A minus group B.
    pub group_a: String,
    /// Second group label.
    pub group_b: String,
    /// Positive bounded permutation count.
    pub permutations: usize,
    /// Base deterministic seed.
    pub seed: u64,
    /// Family-wise error rate used for the critical value.
    pub alpha: f64,
}

/// One endpoint's observed and family-wise adjusted result.
#[derive(Clone, Debug, PartialEq)]
pub struct MaxTEndpointResult {
    /// Exact endpoint name.
    pub endpoint: String,
    /// Signed group-A-minus-group-B mean effect.
    pub effect_group_a_minus_group_b: f64,
    /// Welch-style observed statistic.
    pub studentized_statistic: f64,
    /// Inclusive-plus-one Max-T adjusted p-value under the declared correction.
    pub adjusted_p_value: f64,
}

/// Multiplicity correction applied to the complete endpoint family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaxTCorrection {
    /// Compare every endpoint with the maximum over the complete family.
    SingleStep,
    /// Remove more extreme hypotheses successively while preserving monotone adjusted p-values.
    StepDown,
}

impl MaxTCorrection {
    /// Stable result identity used by the version-one CLI document.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleStep => "single_step_max_t",
            Self::StepDown => "step_down_max_t",
        }
    }
}

/// Patient-level Max-T family result.
#[derive(Clone, Debug, PartialEq)]
pub struct MaxTPermutationResult {
    /// Exact whole-patient multiplicity design.
    pub inference_design: InferenceDesign,
    /// Exact family-wise multiplicity correction.
    pub correction: MaxTCorrection,
    /// Group A patient count.
    pub group_a_count: usize,
    /// Group B patient count.
    pub group_b_count: usize,
    /// Ordered endpoint results.
    pub endpoints: Vec<MaxTEndpointResult>,
    /// Conservative empirical `(1-alpha)` complete-family null maximum threshold.
    pub critical_value: f64,
    /// Family-wise alpha.
    pub alpha: f64,
    /// Requested replicate count.
    pub permutations_requested: usize,
    /// Attempted replicate count.
    pub permutations_attempted: usize,
    /// Completed replicate count.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
}

/// Jointly test a complete scalar endpoint family using whole-patient Max-T permutations.
pub fn max_t_multiple_endpoint_permutation(
    patients: &[PatientEndpointVector],
    spec: &MaxTPermutationSpec,
) -> Result<MaxTPermutationResult, CohortInferenceError> {
    validate_max_t_inputs(patients, spec)?;
    let design = InferenceDesign::population_independence_with_alternative(
        patients.len(),
        spec.permutations,
        spec.seed,
        MAX_T_NAMESPACE,
        InferenceAlternative::TwoSided,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
    .declare_complete_endpoint_family_max_t();
    execute_max_t(patients, spec, design, MaxTCorrection::SingleStep)
}

/// Jointly test a complete scalar endpoint family using step-down whole-patient Max-T.
pub fn max_t_multiple_endpoint_step_down_permutation(
    patients: &[PatientEndpointVector],
    spec: &MaxTPermutationSpec,
) -> Result<MaxTPermutationResult, CohortInferenceError> {
    validate_max_t_inputs(patients, spec)?;
    let design = InferenceDesign::population_independence_with_alternative(
        patients.len(),
        spec.permutations,
        spec.seed,
        MAX_T_NAMESPACE,
        InferenceAlternative::TwoSided,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
    .declare_complete_endpoint_family_max_t();
    execute_max_t(patients, spec, design, MaxTCorrection::StepDown)
}

/// Jointly test an endpoint family while moving patient labels only within exact blocks.
pub fn max_t_multiple_endpoint_blocked_permutation(
    patients: &[PatientEndpointVector],
    assignments: &[PatientExchangeabilityBlock],
    spec: &MaxTPermutationSpec,
) -> Result<MaxTPermutationResult, CohortInferenceError> {
    validate_max_t_inputs(patients, spec)?;
    let observed_labels = patients
        .iter()
        .map(|patient| patient.group == spec.group_a)
        .collect::<Vec<_>>();
    let patient_ids = patients
        .iter()
        .map(|patient| patient.patient_id.clone())
        .collect::<Vec<_>>();
    let design = compile_blocked_population_independence(
        &patient_ids,
        &observed_labels,
        assignments,
        spec.permutations,
        spec.seed,
        MAX_T_NAMESPACE,
        InferenceAlternative::TwoSided,
    )?
    .declare_complete_endpoint_family_max_t();
    execute_max_t(patients, spec, design, MaxTCorrection::SingleStep)
}

/// Apply step-down Max-T while moving patient labels only within exact blocks.
pub fn max_t_multiple_endpoint_blocked_step_down_permutation(
    patients: &[PatientEndpointVector],
    assignments: &[PatientExchangeabilityBlock],
    spec: &MaxTPermutationSpec,
) -> Result<MaxTPermutationResult, CohortInferenceError> {
    validate_max_t_inputs(patients, spec)?;
    let observed_labels = patients
        .iter()
        .map(|patient| patient.group == spec.group_a)
        .collect::<Vec<_>>();
    let patient_ids = patients
        .iter()
        .map(|patient| patient.patient_id.clone())
        .collect::<Vec<_>>();
    let design = compile_blocked_population_independence(
        &patient_ids,
        &observed_labels,
        assignments,
        spec.permutations,
        spec.seed,
        MAX_T_NAMESPACE,
        InferenceAlternative::TwoSided,
    )?
    .declare_complete_endpoint_family_max_t();
    execute_max_t(patients, spec, design, MaxTCorrection::StepDown)
}

fn validate_max_t_inputs(
    patients: &[PatientEndpointVector],
    spec: &MaxTPermutationSpec,
) -> Result<(), CohortInferenceError> {
    validate_spec(spec)?;
    validate_patients(patients, spec)?;
    let endpoint_count = patients[0].endpoints.len();
    let work = patients
        .len()
        .checked_mul(endpoint_count)
        .and_then(|value| value.checked_mul(spec.permutations + 1))
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_MAX_T_EVALUATIONS {
        return Err(work_limit_error());
    }
    Ok(())
}

fn execute_max_t(
    patients: &[PatientEndpointVector],
    spec: &MaxTPermutationSpec,
    design: InferenceDesign,
    correction: MaxTCorrection,
) -> Result<MaxTPermutationResult, CohortInferenceError> {
    let observed_labels = patients
        .iter()
        .map(|patient| patient.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = endpoint_contrasts(patients, &observed_labels)?;
    let observed_statistics = observed
        .iter()
        .map(|contrast| contrast.studentized)
        .collect::<Vec<_>>();
    let observed_order = max_t_observed_order(&observed_statistics);
    let mut exceedances = vec![0usize; observed.len()];
    let mut null_maxima = Vec::with_capacity(spec.permutations);
    for replicate in 0..spec.permutations {
        let labels = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
            .iter()
            .map(|source| observed_labels[*source])
            .collect::<Vec<_>>();
        let contrasts = endpoint_contrasts(patients, &labels)?;
        let null_statistics = contrasts
            .iter()
            .map(|contrast| contrast.studentized.abs())
            .collect::<Vec<_>>();
        let maximum = null_statistics.iter().copied().fold(0.0_f64, f64::max);
        if !maximum.is_finite() {
            return Err(CohortInferenceError::NumericalFailure(
                "Max-T null maximum is non-finite".into(),
            ));
        }
        null_maxima.push(maximum);
        match correction {
            MaxTCorrection::SingleStep => {
                for (index, contrast) in observed.iter().enumerate() {
                    exceedances[index] += usize::from(maximum >= contrast.studentized.abs());
                }
            }
            MaxTCorrection::StepDown => accumulate_step_down_exceedances(
                &observed_statistics,
                &observed_order,
                &null_statistics,
                &mut exceedances,
            ),
        }
    }
    let mut ordered_maxima = null_maxima.clone();
    ordered_maxima.sort_by(f64::total_cmp);
    let rank = ((1.0 - spec.alpha) * (spec.permutations + 1) as f64).ceil() as usize;
    let critical_value = ordered_maxima[rank.saturating_sub(1).min(ordered_maxima.len() - 1)];
    let mut adjusted_p_values = exceedances
        .into_iter()
        .map(|count| (count as f64 + 1.0) / (spec.permutations + 1) as f64)
        .collect::<Vec<_>>();
    if correction == MaxTCorrection::StepDown {
        let mut previous = 0.0_f64;
        for endpoint_index in &observed_order {
            previous = previous.max(adjusted_p_values[*endpoint_index]);
            adjusted_p_values[*endpoint_index] = previous;
        }
    }
    let endpoints = patients[0]
        .endpoints
        .iter()
        .zip(observed.iter().zip(adjusted_p_values))
        .map(
            |(endpoint, (contrast, adjusted_p_value))| MaxTEndpointResult {
                endpoint: endpoint.clone(),
                effect_group_a_minus_group_b: contrast.effect,
                studentized_statistic: contrast.studentized,
                adjusted_p_value,
            },
        )
        .collect();

    Ok(MaxTPermutationResult {
        inference_design: design,
        correction,
        group_a_count: observed[0].group_a_count,
        group_b_count: observed[0].group_b_count,
        endpoints,
        critical_value,
        alpha: spec.alpha,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
    })
}

pub(crate) fn max_t_observed_order(observed_statistics: &[f64]) -> Vec<usize> {
    let mut order = (0..observed_statistics.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        observed_statistics[*right]
            .abs()
            .total_cmp(&observed_statistics[*left].abs())
            .then_with(|| left.cmp(right))
    });
    order
}

pub(crate) fn accumulate_step_down_exceedances(
    observed_statistics: &[f64],
    observed_order: &[usize],
    null_statistics: &[f64],
    exceedances: &mut [usize],
) {
    let mut running_maximum = 0.0_f64;
    let mut tied_end = observed_order.len();
    while tied_end > 0 {
        let observed_statistic = observed_statistics[observed_order[tied_end - 1]].abs();
        let mut tied_start = tied_end - 1;
        while tied_start > 0
            && observed_statistics[observed_order[tied_start - 1]]
                .abs()
                .total_cmp(&observed_statistic)
                .is_eq()
        {
            tied_start -= 1;
        }
        for endpoint_index in &observed_order[tied_start..tied_end] {
            running_maximum = running_maximum.max(null_statistics[*endpoint_index]);
        }
        let exceeds = running_maximum >= observed_statistic;
        for endpoint_index in &observed_order[tied_start..tied_end] {
            exceedances[*endpoint_index] += usize::from(exceeds);
        }
        tied_end = tied_start;
    }
}

fn validate_spec(spec: &MaxTPermutationSpec) -> Result<(), CohortInferenceError> {
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
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 1.0) {
        return Err(CohortInferenceError::InvalidInput(
            "Max-T alpha must be finite and strictly between zero and one".into(),
        ));
    }
    Ok(())
}

fn validate_patients(
    patients: &[PatientEndpointVector],
    spec: &MaxTPermutationSpec,
) -> Result<(), CohortInferenceError> {
    if patients.is_empty() || patients.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "Max-T requires 1 to {MAXIMUM_PATIENTS} patient vectors"
        )));
    }
    let endpoints = &patients[0].endpoints;
    if endpoints.is_empty() || endpoints.iter().any(|endpoint| endpoint.trim().is_empty()) {
        return Err(CohortInferenceError::InvalidInput(
            "Max-T requires non-empty endpoint names".into(),
        ));
    }
    let unique_endpoints = endpoints.iter().collect::<HashSet<_>>();
    if unique_endpoints.len() != endpoints.len() {
        return Err(CohortInferenceError::InvalidInput(
            "Max-T endpoint names must be unique".into(),
        ));
    }
    let mut patient_ids = HashSet::with_capacity(patients.len());
    let mut group_a_count = 0usize;
    let mut group_b_count = 0usize;
    for patient in patients {
        if patient.patient_id.trim().is_empty() || patient.patient_id.trim() != patient.patient_id {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if !patient_ids.insert(patient.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate patient vector: {}",
                patient.patient_id
            )));
        }
        if patient.group == spec.group_a {
            group_a_count += 1;
        } else if patient.group == spec.group_b {
            group_b_count += 1;
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared group {:?}",
                patient.patient_id, patient.group
            )));
        }
        if patient.endpoints != *endpoints || patient.values.len() != endpoints.len() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} does not have the exact complete endpoint family",
                patient.patient_id
            )));
        }
        if patient.values.iter().any(|value| !value.is_finite()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has a non-finite endpoint value",
                patient.patient_id
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "each group must contain at least two patients".into(),
        ));
    }
    Ok(())
}

fn endpoint_contrasts(
    patients: &[PatientEndpointVector],
    labels: &[bool],
) -> Result<Vec<super::numeric::WelchContrast>, CohortInferenceError> {
    (0..patients[0].endpoints.len())
        .map(|endpoint_index| {
            let values = patients
                .iter()
                .map(|patient| patient.values[endpoint_index])
                .collect::<Vec<_>>();
            welch_contrast(&values, labels)
        })
        .collect()
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "Max-T patient-by-endpoint-by-permutation work exceeds the {MAXIMUM_MAX_T_EVALUATIONS}-evaluation limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_statistics_match_hand_oracle() {
        let patients = [
            ("a-1", "A", [8.0, 4.0]),
            ("a-2", "A", [9.0, 7.0]),
            ("b-1", "B", [1.0, 2.0]),
            ("b-2", "B", [2.0, 3.0]),
        ]
        .into_iter()
        .map(|(patient, group, values)| PatientEndpointVector {
            patient_id: patient.into(),
            group: group.into(),
            endpoints: vec!["e1".into(), "e2".into()],
            values: values.into(),
        })
        .collect::<Vec<_>>();
        let result = max_t_multiple_endpoint_permutation(
            &patients,
            &MaxTPermutationSpec {
                group_a: "A".into(),
                group_b: "B".into(),
                permutations: 99,
                seed: 41,
                alpha: 0.05,
            },
        )
        .expect("Max-T result");
        assert_eq!(result.endpoints[0].effect_group_a_minus_group_b, 7.0);
        assert!((result.endpoints[0].studentized_statistic - 9.899_494_936_611_665).abs() < 1e-12);
        assert_eq!(result.endpoints[1].effect_group_a_minus_group_b, 3.0);
        assert!(
            (result.endpoints[1].studentized_statistic - 1.897_366_596_101_027_5).abs() < 1e-12
        );
    }
}
