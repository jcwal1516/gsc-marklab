use super::{
    compensated_sum,
    inference_design::compile_blocked_population_independence,
    mmd::{squared_euclidean, validate_fingerprints},
    CohortInferenceError, Fingerprint, InferenceDesign, PatientExchangeabilityBlock,
    MAXIMUM_PERMUTATIONS,
};

const ENERGY_NAMESPACE: u64 = 0x656e_6572_6779_5f70;
const MAXIMUM_DISTANCE_ELEMENTS: usize = 25_000_000;
const MAXIMUM_ENERGY_EVALUATIONS: usize = 100_000_000;

/// Prespecified negative-type metric for energy distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnergyMetric {
    /// Exact Euclidean distance over the complete fingerprint.
    Euclidean,
}

/// Frozen patient-level energy-distance design.
#[derive(Clone, Debug)]
pub struct EnergyDistanceSpec {
    /// First group label.
    pub group_a: String,
    /// Second group label.
    pub group_b: String,
    /// Frozen negative-type metric.
    pub metric: EnergyMetric,
    /// Positive bounded permutation count.
    pub permutations: usize,
    /// Base deterministic seed.
    pub seed: u64,
}

/// Patient-level energy-distance permutation result.
#[derive(Clone, Debug, PartialEq)]
pub struct EnergyDistanceResult {
    /// Group A patient count.
    pub group_a_count: usize,
    /// Group B patient count.
    pub group_b_count: usize,
    /// Feature dimension.
    pub feature_count: usize,
    /// Frozen metric.
    pub metric: EnergyMetric,
    /// Observed energy V-statistic.
    pub energy_distance: f64,
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

/// Patient-level energy-distance result with its exact exchangeability design.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockedEnergyDistanceResult {
    result: EnergyDistanceResult,
    design: InferenceDesign,
}

impl BlockedEnergyDistanceResult {
    /// Permutation-test result computed under the blocked design.
    pub fn result(&self) -> &EnergyDistanceResult {
        &self.result
    }

    /// Exact patient-level exchangeability design used for every replicate.
    pub fn design(&self) -> &InferenceDesign {
        &self.design
    }

    /// Consume the wrapper into its result and design.
    pub fn into_parts(self) -> (EnergyDistanceResult, InferenceDesign) {
        (self.result, self.design)
    }
}

/// Compare patient fingerprint distributions with a frozen exact distance matrix.
pub fn patient_level_energy_distance(
    fingerprints: &[Fingerprint],
    spec: &EnergyDistanceSpec,
) -> Result<EnergyDistanceResult, CohortInferenceError> {
    validate_energy_inputs(fingerprints, spec)?;
    let design = InferenceDesign::population_independence(
        fingerprints.len(),
        spec.permutations,
        spec.seed,
        ENERGY_NAMESPACE,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
    execute_energy(fingerprints, spec, &design)
}

/// Compare patient fingerprint distributions within exact exchangeability blocks.
pub fn patient_level_blocked_energy_distance(
    fingerprints: &[Fingerprint],
    assignments: &[PatientExchangeabilityBlock],
    spec: &EnergyDistanceSpec,
) -> Result<BlockedEnergyDistanceResult, CohortInferenceError> {
    validate_energy_inputs(fingerprints, spec)?;
    let observed_labels = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.group == spec.group_a)
        .collect::<Vec<_>>();
    let patient_ids = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.patient_id.clone())
        .collect::<Vec<_>>();
    let design = compile_blocked_population_independence(
        &patient_ids,
        &observed_labels,
        assignments,
        spec.permutations,
        spec.seed,
        ENERGY_NAMESPACE,
    )?;
    let result = execute_energy(fingerprints, spec, &design)?;
    Ok(BlockedEnergyDistanceResult { result, design })
}

fn validate_energy_inputs(
    fingerprints: &[Fingerprint],
    spec: &EnergyDistanceSpec,
) -> Result<(), CohortInferenceError> {
    validate_spec(spec)?;
    validate_fingerprints(fingerprints, &spec.group_a, &spec.group_b)?;
    let elements = fingerprints
        .len()
        .checked_mul(fingerprints.len())
        .ok_or_else(distance_limit_error)?;
    if elements > MAXIMUM_DISTANCE_ELEMENTS {
        return Err(distance_limit_error());
    }
    let work = elements
        .checked_mul(spec.permutations + 1)
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_ENERGY_EVALUATIONS {
        return Err(work_limit_error());
    }
    Ok(())
}

fn execute_energy(
    fingerprints: &[Fingerprint],
    spec: &EnergyDistanceSpec,
    design: &InferenceDesign,
) -> Result<EnergyDistanceResult, CohortInferenceError> {
    let distances = build_distance_matrix(fingerprints, spec.metric)?;
    let observed_labels = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = energy_distance(&distances, fingerprints.len(), &observed_labels)?;
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let labels = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
            .iter()
            .map(|source| observed_labels[*source])
            .collect::<Vec<_>>();
        let statistic = energy_distance(&distances, fingerprints.len(), &labels)?;
        exceedances += usize::from(statistic >= observed);
    }
    let group_a_count = observed_labels.iter().filter(|label| **label).count();
    Ok(EnergyDistanceResult {
        group_a_count,
        group_b_count: fingerprints.len() - group_a_count,
        feature_count: fingerprints[0].features.len(),
        metric: spec.metric,
        energy_distance: observed,
        p_value: (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
    })
}

fn validate_spec(spec: &EnergyDistanceSpec) -> Result<(), CohortInferenceError> {
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

fn build_distance_matrix(
    fingerprints: &[Fingerprint],
    metric: EnergyMetric,
) -> Result<Vec<f64>, CohortInferenceError> {
    let n = fingerprints.len();
    let mut matrix = vec![0.0; n * n];
    for left in 0..n {
        for right in left..n {
            let distance = match metric {
                EnergyMetric::Euclidean => {
                    squared_euclidean(&fingerprints[left].values, &fingerprints[right].values)
                        .sqrt()
                }
            };
            if !distance.is_finite() {
                return Err(CohortInferenceError::NumericalFailure(
                    "energy metric produced a non-finite distance".into(),
                ));
            }
            matrix[left * n + right] = distance;
            matrix[right * n + left] = distance;
        }
    }
    Ok(matrix)
}

fn energy_distance(
    distances: &[f64],
    n: usize,
    labels: &[bool],
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
    let cross = matrix_mean(distances, n, &group_a, &group_b);
    let within_a = matrix_mean(distances, n, &group_a, &group_a);
    let within_b = matrix_mean(distances, n, &group_b, &group_b);
    let value = 2.0 * cross - within_a - within_b;
    if !value.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "energy statistic produced a non-finite value".into(),
        ));
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn matrix_mean(distances: &[f64], n: usize, left: &[usize], right: &[usize]) -> f64 {
    compensated_sum(
        left.iter()
            .flat_map(|left| right.iter().map(move |right| distances[left * n + right])),
    ) / (left.len() * right.len()) as f64
}

fn distance_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "energy distance matrix exceeds the {MAXIMUM_DISTANCE_ELEMENTS}-element limit"
    ))
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "energy matrix-by-permutation work exceeds the {MAXIMUM_ENERGY_EVALUATIONS}-evaluation limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euclidean_energy_matches_hand_oracle() {
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
        let result = patient_level_energy_distance(
            &fingerprints,
            &EnergyDistanceSpec {
                group_a: "A".into(),
                group_b: "B".into(),
                metric: EnergyMetric::Euclidean,
                permutations: 99,
                seed: 53,
            },
        )
        .expect("energy result");
        assert_eq!(result.energy_distance, 5.5);
    }
}
