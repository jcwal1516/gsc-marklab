use std::collections::{BTreeMap, HashSet};

use super::{
    max_t::{accumulate_step_down_exceedances, max_t_observed_order},
    numeric::mean_standard_error,
    CohortInferenceError, InferenceDesign, MaxTCorrection, MAXIMUM_PATIENTS, MAXIMUM_PERMUTATIONS,
};

const PAIRED_MAX_T_NAMESPACE: u64 = 0x7061_6972_6d61_7874;
const MAXIMUM_PAIRED_MAX_T_EVALUATIONS: usize = 100_000_000;

/// One complete condition-specific endpoint vector belonging to one patient pair.
#[derive(Clone, Debug)]
pub struct PairedPatientEndpointVector {
    /// Stable patient-pair identifier.
    pub patient_id: String,
    /// Exact declared condition label.
    pub condition: String,
    /// Exact ordered endpoint names shared by every condition vector.
    pub endpoints: Vec<String>,
    /// Finite values aligned to `endpoints`.
    pub values: Vec<f64>,
}

/// Frozen paired endpoint-family Max-T design.
#[derive(Clone, Debug)]
pub struct PairedMaxTPermutationSpec {
    /// First condition label; effects are condition B minus condition A.
    pub condition_a: String,
    /// Second condition label.
    pub condition_b: String,
    /// Positive bounded sign-flip count.
    pub permutations: usize,
    /// Base deterministic seed.
    pub seed: u64,
    /// Family-wise error rate used for the initial complete-family critical value.
    pub alpha: f64,
    /// Single-step or step-down Max-T adjustment.
    pub correction: MaxTCorrection,
}

/// One paired endpoint's observed and family-wise adjusted result.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedMaxTEndpointResult {
    /// Exact endpoint name.
    pub endpoint: String,
    /// Mean exact condition-B-minus-condition-A patient difference.
    pub effect_condition_b_minus_condition_a: f64,
    /// Studentized mean paired difference.
    pub studentized_statistic: f64,
    /// Inclusive-plus-one Max-T adjusted p-value.
    pub adjusted_p_value: f64,
}

/// Patient-paired endpoint-family Max-T result.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedMaxTPermutationResult {
    /// Exact paired vector sign-flip design.
    pub inference_design: InferenceDesign,
    /// Exact multiplicity correction.
    pub correction: MaxTCorrection,
    /// Number of complete independent patient pairs.
    pub pair_count: usize,
    /// Exact first condition label.
    pub condition_a: String,
    /// Exact second condition label.
    pub condition_b: String,
    /// Ordered endpoint results.
    pub endpoints: Vec<PairedMaxTEndpointResult>,
    /// Conservative empirical complete-family `(1-alpha)` null maximum threshold.
    pub critical_value: f64,
    /// Family-wise alpha.
    pub alpha: f64,
    /// Requested sign-flip count.
    pub permutations_requested: usize,
    /// Attempted sign-flip count.
    pub permutations_attempted: usize,
    /// Completed sign-flip count.
    pub permutations_completed: usize,
    /// Base seed.
    pub seed: u64,
}

/// Test a complete paired endpoint family while signing each patient's difference vector once.
pub fn paired_max_t_permutation(
    records: &[PairedPatientEndpointVector],
    spec: &PairedMaxTPermutationSpec,
) -> Result<PairedMaxTPermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    let pairs = build_pairs(records, spec)?;
    let endpoint_count = records[0].endpoints.len();
    let work = pairs
        .len()
        .checked_mul(endpoint_count)
        .and_then(|value| value.checked_mul(spec.permutations + 1))
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_PAIRED_MAX_T_EVALUATIONS {
        return Err(work_limit_error());
    }

    let differences = (0..endpoint_count)
        .map(|endpoint| {
            pairs
                .iter()
                .map(|pair| pair.condition_b[endpoint] - pair.condition_a[endpoint])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if differences.iter().flatten().any(|value| !value.is_finite()) {
        return Err(CohortInferenceError::NumericalFailure(
            "paired endpoint subtraction produced a non-finite difference".into(),
        ));
    }
    let observed = differences
        .iter()
        .map(|values| mean_standard_error(values))
        .collect::<Result<Vec<_>, _>>()?;
    let observed_statistics = observed
        .iter()
        .map(|summary| summary.mean / summary.standard_error)
        .collect::<Vec<_>>();
    let observed_order = max_t_observed_order(&observed_statistics);
    let design = InferenceDesign::paired_vector_sign_flip(
        pairs.len(),
        spec.permutations,
        spec.seed,
        PAIRED_MAX_T_NAMESPACE,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
    .declare_complete_endpoint_family_max_t();

    let mut exceedances = vec![0usize; endpoint_count];
    let mut null_maxima = Vec::with_capacity(spec.permutations);
    let mut signed = vec![0.0; pairs.len()];
    for replicate in 0..spec.permutations {
        let signs = design
            .paired_difference_vector_signs(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        let mut null_statistics = Vec::with_capacity(endpoint_count);
        for endpoint_differences in &differences {
            for ((target, difference), sign) in signed
                .iter_mut()
                .zip(endpoint_differences)
                .zip(signs.iter())
            {
                *target = *difference * f64::from(*sign);
            }
            let summary = mean_standard_error(&signed)?;
            null_statistics.push((summary.mean / summary.standard_error).abs());
        }
        let maximum = null_statistics.iter().copied().fold(0.0_f64, f64::max);
        if !maximum.is_finite() {
            return Err(CohortInferenceError::NumericalFailure(
                "paired Max-T null maximum is non-finite".into(),
            ));
        }
        null_maxima.push(maximum);
        match spec.correction {
            MaxTCorrection::SingleStep => {
                for (index, statistic) in observed_statistics.iter().enumerate() {
                    exceedances[index] += usize::from(maximum >= statistic.abs());
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

    let mut ordered_maxima = null_maxima;
    ordered_maxima.sort_by(f64::total_cmp);
    let rank = ((1.0 - spec.alpha) * (spec.permutations + 1) as f64).ceil() as usize;
    let critical_value = ordered_maxima[rank.saturating_sub(1).min(ordered_maxima.len() - 1)];
    let mut adjusted = exceedances
        .into_iter()
        .map(|count| (count as f64 + 1.0) / (spec.permutations + 1) as f64)
        .collect::<Vec<_>>();
    if spec.correction == MaxTCorrection::StepDown {
        let mut previous = 0.0_f64;
        for endpoint in &observed_order {
            previous = previous.max(adjusted[*endpoint]);
            adjusted[*endpoint] = previous;
        }
    }
    let endpoints = records[0]
        .endpoints
        .iter()
        .enumerate()
        .map(|(index, endpoint)| PairedMaxTEndpointResult {
            endpoint: endpoint.clone(),
            effect_condition_b_minus_condition_a: observed[index].mean,
            studentized_statistic: observed_statistics[index],
            adjusted_p_value: adjusted[index],
        })
        .collect();

    Ok(PairedMaxTPermutationResult {
        inference_design: design,
        correction: spec.correction,
        pair_count: pairs.len(),
        condition_a: spec.condition_a.clone(),
        condition_b: spec.condition_b.clone(),
        endpoints,
        critical_value,
        alpha: spec.alpha,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
    })
}

fn validate_spec(spec: &PairedMaxTPermutationSpec) -> Result<(), CohortInferenceError> {
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
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 1.0) {
        return Err(CohortInferenceError::InvalidInput(
            "paired Max-T alpha must be finite and strictly between zero and one".into(),
        ));
    }
    Ok(())
}

#[derive(Default)]
struct PairBuilder {
    condition_a: Option<Vec<f64>>,
    condition_b: Option<Vec<f64>>,
}

struct Pair {
    condition_a: Vec<f64>,
    condition_b: Vec<f64>,
}

fn build_pairs(
    records: &[PairedPatientEndpointVector],
    spec: &PairedMaxTPermutationSpec,
) -> Result<Vec<Pair>, CohortInferenceError> {
    if records.is_empty() || records.len() > MAXIMUM_PATIENTS.saturating_mul(2) {
        return Err(CohortInferenceError::InvalidInput(format!(
            "paired Max-T requires 1 to {} condition vectors",
            MAXIMUM_PATIENTS * 2
        )));
    }
    let endpoints = &records[0].endpoints;
    if endpoints.is_empty() || endpoints.iter().any(|endpoint| endpoint.trim().is_empty()) {
        return Err(CohortInferenceError::InvalidInput(
            "paired Max-T requires non-empty endpoint names".into(),
        ));
    }
    if endpoints.iter().collect::<HashSet<_>>().len() != endpoints.len() {
        return Err(CohortInferenceError::InvalidInput(
            "paired Max-T endpoint names must be unique".into(),
        ));
    }
    let mut builders = BTreeMap::<&str, PairBuilder>::new();
    for record in records {
        if record.patient_id.trim().is_empty() || record.patient_id.trim() != record.patient_id {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if record.endpoints != *endpoints || record.values.len() != endpoints.len() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} does not have the exact complete paired endpoint family",
                record.patient_id
            )));
        }
        if record.values.iter().any(|value| !value.is_finite()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has a non-finite paired endpoint value",
                record.patient_id
            )));
        }
        let builder = builders.entry(&record.patient_id).or_default();
        let slot = if record.condition == spec.condition_a {
            &mut builder.condition_a
        } else if record.condition == spec.condition_b {
            &mut builder.condition_b
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared condition {:?}",
                record.patient_id, record.condition
            )));
        };
        if slot.replace(record.values.clone()).is_some() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has a duplicate condition vector",
                record.patient_id
            )));
        }
    }
    if builders.len() < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "paired Max-T requires at least two complete patient pairs".into(),
        ));
    }
    builders
        .into_iter()
        .map(
            |(patient, builder)| match (builder.condition_a, builder.condition_b) {
                (Some(condition_a), Some(condition_b)) => Ok(Pair {
                    condition_a,
                    condition_b,
                }),
                _ => Err(CohortInferenceError::InvalidInput(format!(
                    "patient {patient} is missing a declared condition vector"
                ))),
            },
        )
        .collect()
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "paired Max-T pair-by-endpoint-by-permutation work exceeds the {MAXIMUM_PAIRED_MAX_T_EVALUATIONS}-evaluation limit"
    ))
}
