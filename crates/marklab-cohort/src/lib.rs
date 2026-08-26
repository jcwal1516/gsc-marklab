#![forbid(unsafe_code)]
//! Cohort-valid population inference for Marklab scientific workflows.

use std::collections::{BTreeMap, HashSet};

use thiserror::Error;

mod energy;
mod equivalence;
mod fingerprint;
mod functional;
mod functional_equivalence;
mod hierarchical_bootstrap;
mod max_t;
mod mmd;
mod multisite;
mod noninferiority;
mod numeric;
mod paired;
mod repeated;

use numeric::welch_contrast;

pub use energy::{
    patient_level_energy_distance, EnergyDistanceResult, EnergyDistanceSpec, EnergyMetric,
};
pub use equivalence::{
    tost_equivalence, EquivalenceInterval, PatientEffect, TostEquivalenceResult,
    TostEquivalenceSpec,
};
pub use fingerprint::{
    build_spatial_fingerprint, fingerprint_distance, region_compatibility,
    FingerprintComponentDistance, FingerprintDistanceResult, RegionCompatibilityComponent,
    RegionCompatibilityResult, SpatialFingerprint, SpatialFingerprintComponent,
    SpatialFingerprintInput,
};
pub use functional::{
    functional_two_sample_permutation, FunctionalCurve, FunctionalPermutationResult,
    FunctionalPermutationSpec, FunctionalTestStatistic,
};
pub use functional_equivalence::{
    functional_equivalence_band, FunctionalDifferenceCurve, FunctionalEquivalencePoint,
    FunctionalEquivalenceResult, FunctionalEquivalenceSpec,
};
pub use hierarchical_bootstrap::{
    bootstrap_equivalence, hierarchical_bootstrap, BootstrapEquivalenceResult,
    BootstrapEquivalenceSpec, HierarchicalBootstrapInterval, HierarchicalBootstrapResult,
    HierarchicalBootstrapSpec, HierarchicalScalarRecord,
};
pub use max_t::{
    max_t_multiple_endpoint_permutation, MaxTEndpointResult, MaxTPermutationResult,
    MaxTPermutationSpec, PatientEndpointVector,
};
pub use mmd::{
    patient_level_mmd, Fingerprint, MmdEstimator, MmdKernel, MmdPermutationResult,
    MmdPermutationSpec,
};
pub use multisite::{
    multisite_spatial_inference, MultisiteEffectModel, MultisiteInferenceResult,
    MultisiteInferenceSpec, SiteEffect, SiteSensitivityResult,
};
pub use noninferiority::{
    noninferiority_test, NoninferiorityDirection, NoninferiorityResult, NoninferioritySpec,
};
pub use paired::{
    paired_patient_permutation_test, PairedConditionSummary, PairedPatientEndpoint,
    PairedPatientPermutationResult, PairedPatientPermutationSpec,
};
pub use repeated::{
    repeated_measures_freedman_lane, RepeatedFreedmanLaneResult, RepeatedFreedmanLaneSpec,
    RepeatedMeasureRecord,
};

const MAXIMUM_PATIENTS: usize = 1_000_000;
const MAXIMUM_PERMUTATIONS: usize = 1_000_000;
const MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS: usize = 100_000_000;
const PATIENT_PERMUTATION_NAMESPACE: u64 = 0x7061_7469_656e_745f;

/// Direction of a patient-level permutation alternative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermutationAlternative {
    /// Smaller studentized group-A-minus-group-B contrasts are more extreme.
    Less,
    /// Larger studentized group-A-minus-group-B contrasts are more extreme.
    Greater,
    /// Either tail is extreme, using the inclusive equal-tail definition.
    TwoSided,
}

/// One prespecified finite scalar endpoint for one independent patient.
#[derive(Clone, Debug)]
pub struct PatientEndpoint {
    /// Stable patient identifier supplied by the caller.
    pub patient_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Prespecified scalar endpoint.
    pub endpoint: f64,
    /// Optional exact exchangeability block; `None` is the common block.
    pub block: Option<String>,
}

/// Frozen design and deterministic execution settings for a scalar permutation test.
#[derive(Clone, Debug)]
pub struct PatientPermutationSpec {
    /// First group label; effects are group A minus group B.
    pub group_a: String,
    /// Second group label.
    pub group_b: String,
    /// Positive bounded number of requested permutations.
    pub permutations: usize,
    /// Base seed for domain-separated replicate streams.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: PermutationAlternative,
}

/// Summary of one declared group.
#[derive(Clone, Debug, PartialEq)]
pub struct PatientGroupSummary {
    /// Exact declared label.
    pub label: String,
    /// Number of independent patients.
    pub patient_count: usize,
    /// Stable arithmetic mean of patient endpoints.
    pub mean: f64,
}

/// Cohort-valid scalar patient-level permutation result.
#[derive(Clone, Debug, PartialEq)]
pub struct PatientPermutationResult {
    /// Whether labels were restricted within at least one declared block.
    pub blocked: bool,
    /// Number of exact exchangeability blocks.
    pub block_count: usize,
    /// Total independent patient count.
    pub patient_count: usize,
    /// Group A summary.
    pub group_a: PatientGroupSummary,
    /// Group B summary.
    pub group_b: PatientGroupSummary,
    /// Signed group-A-minus-group-B mean effect.
    pub effect_group_a_minus_group_b: f64,
    /// Welch-style studentized group contrast.
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

/// Invalid design, input, or numerical state from cohort inference.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CohortInferenceError {
    /// The caller supplied an invalid design or input table.
    #[error("{0}")]
    InvalidInput(String),
    /// A finite input produced an undefined or non-finite statistic.
    #[error("{0}")]
    NumericalFailure(String),
}

/// Compare one finite scalar endpoint per patient between two independent groups.
///
/// Labels are permuted only as whole-patient labels and only within exact declared
/// blocks. Every requested replicate must complete; an undefined observed or null
/// studentized contrast returns an error rather than being removed.
pub fn patient_level_permutation_test(
    records: &[PatientEndpoint],
    spec: &PatientPermutationSpec,
) -> Result<PatientPermutationResult, CohortInferenceError> {
    validate_spec(spec)?;
    let records = validate_records(records, spec)?;
    let evaluations = records
        .len()
        .checked_mul(spec.permutations)
        .ok_or_else(|| {
            CohortInferenceError::InvalidInput(
                "patient-by-permutation work exceeds the supported limit".into(),
            )
        })?;
    if evaluations > MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "patient-by-permutation work exceeds the {MAXIMUM_PATIENT_PERMUTATION_EVALUATIONS}-evaluation limit"
        )));
    }
    let plan = build_exchangeability_plan(&records)?;
    let observed_labels = records
        .iter()
        .map(|record| record.is_group_a)
        .collect::<Vec<_>>();
    let endpoint_values = records
        .iter()
        .map(|record| record.endpoint)
        .collect::<Vec<_>>();
    let observed = welch_contrast(&endpoint_values, &observed_labels)?;

    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    for replicate in 0..spec.permutations {
        let labels = restricted_shuffle(&records, &plan, derive_seed(spec.seed, replicate));
        let statistic = welch_contrast(&endpoint_values, &labels)?.studentized;
        lower_tail += usize::from(statistic <= observed.studentized);
        upper_tail += usize::from(statistic >= observed.studentized);
    }
    let denominator = (spec.permutations + 1) as f64;
    let p_value = match spec.alternative {
        PermutationAlternative::Less => (lower_tail as f64 + 1.0) / denominator,
        PermutationAlternative::Greater => (upper_tail as f64 + 1.0) / denominator,
        PermutationAlternative::TwoSided => {
            (2.0 * ((lower_tail.min(upper_tail) as f64 + 1.0) / denominator)).min(1.0)
        }
    };

    Ok(PatientPermutationResult {
        blocked: plan.blocked,
        block_count: plan.blocks.len(),
        patient_count: records.len(),
        group_a: PatientGroupSummary {
            label: spec.group_a.clone(),
            patient_count: observed.group_a_count,
            mean: observed.group_a_mean,
        },
        group_b: PatientGroupSummary {
            label: spec.group_b.clone(),
            patient_count: observed.group_b_count,
            mean: observed.group_b_mean,
        },
        effect_group_a_minus_group_b: observed.effect,
        studentized_statistic: observed.studentized,
        p_value,
        permutations_requested: spec.permutations,
        permutations_attempted: spec.permutations,
        permutations_completed: spec.permutations,
        seed: spec.seed,
        alternative: spec.alternative,
    })
}

fn validate_spec(spec: &PatientPermutationSpec) -> Result<(), CohortInferenceError> {
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

#[derive(Clone, Debug)]
struct ValidatedRecord {
    is_group_a: bool,
    endpoint: f64,
    block: String,
}

fn validate_records(
    records: &[PatientEndpoint],
    spec: &PatientPermutationSpec,
) -> Result<Vec<ValidatedRecord>, CohortInferenceError> {
    if records.is_empty() {
        return Err(CohortInferenceError::InvalidInput(
            "input must contain at least one patient".into(),
        ));
    }
    if records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "input exceeds the {MAXIMUM_PATIENTS}-patient limit"
        )));
    }

    let mut patient_ids = HashSet::with_capacity(records.len());
    let mut validated = Vec::with_capacity(records.len());
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
        if !patient_ids.insert(patient_id) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate patient_id: {patient_id}"
            )));
        }
        let is_group_a = if record.group == spec.group_a {
            true
        } else if record.group == spec.group_b {
            false
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {patient_id} has undeclared group {:?}",
                record.group
            )));
        };
        if !record.endpoint.is_finite() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {patient_id} has a non-finite endpoint"
            )));
        }
        let block = record.block.as_deref().unwrap_or_default();
        if block.trim() != block {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {patient_id} block may not have surrounding whitespace"
            )));
        }
        validated.push(ValidatedRecord {
            is_group_a,
            endpoint: record.endpoint,
            block: block.to_owned(),
        });
    }
    Ok(validated)
}

#[derive(Debug)]
struct ExchangeabilityPlan {
    blocks: Vec<Vec<usize>>,
    blocked: bool,
}

fn build_exchangeability_plan(
    records: &[ValidatedRecord],
) -> Result<ExchangeabilityPlan, CohortInferenceError> {
    let mut by_block: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        by_block.entry(&record.block).or_default().push(index);
    }
    let blocked = by_block.len() > 1 || by_block.keys().any(|block| !block.is_empty());
    if blocked
        && !by_block.values().any(|indices| {
            indices.iter().any(|index| records[*index].is_group_a)
                && indices.iter().any(|index| !records[*index].is_group_a)
        })
    {
        return Err(CohortInferenceError::InvalidInput(
            "declared blocks are fully confounded with group".into(),
        ));
    }
    Ok(ExchangeabilityPlan {
        blocks: by_block.into_values().collect(),
        blocked,
    })
}

fn restricted_shuffle(
    records: &[ValidatedRecord],
    plan: &ExchangeabilityPlan,
    seed: u64,
) -> Vec<bool> {
    let mut labels = records
        .iter()
        .map(|record| record.is_group_a)
        .collect::<Vec<_>>();
    let mut state = seed;
    for indices in &plan.blocks {
        let mut block_labels = indices
            .iter()
            .map(|index| labels[*index])
            .collect::<Vec<_>>();
        for index in (1..block_labels.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            block_labels.swap(index, other);
        }
        for (index, label) in indices.iter().zip(block_labels) {
            labels[*index] = label;
        }
    }
    labels
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

fn derive_seed(base_seed: u64, replicate: usize) -> u64 {
    derive_seed_in_namespace(base_seed, PATIENT_PERMUTATION_NAMESPACE, replicate)
}

fn derive_seed_in_namespace(base_seed: u64, namespace: u64, replicate: usize) -> u64 {
    splitmix64(splitmix64(base_seed ^ namespace) ^ replicate as u64)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

fn shuffled_labels(labels: &[bool], seed: u64) -> Vec<bool> {
    let mut shuffled = labels.to_vec();
    let mut state = seed;
    for index in (1..shuffled.len()).rev() {
        state = splitmix64(state ^ index as u64);
        let other = (state % (index as u64 + 1)) as usize;
        shuffled.swap(index, other);
    }
    shuffled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoints() -> Vec<PatientEndpoint> {
        vec![
            PatientEndpoint {
                patient_id: "a-1".into(),
                group: "A".into(),
                endpoint: 8.0,
                block: None,
            },
            PatientEndpoint {
                patient_id: "a-2".into(),
                group: "A".into(),
                endpoint: 9.0,
                block: None,
            },
            PatientEndpoint {
                patient_id: "b-1".into(),
                group: "B".into(),
                endpoint: 1.0,
                block: None,
            },
            PatientEndpoint {
                patient_id: "b-2".into(),
                group: "B".into(),
                endpoint: 2.0,
                block: None,
            },
        ]
    }

    fn spec() -> PatientPermutationSpec {
        PatientPermutationSpec {
            group_a: "A".into(),
            group_b: "B".into(),
            permutations: 99,
            seed: 17,
            alternative: PermutationAlternative::Less,
        }
    }

    #[test]
    fn patient_level_result_matches_hand_oracle() {
        let result = patient_level_permutation_test(&endpoints(), &spec()).expect("result");
        assert_eq!(result.group_a.mean, 8.5);
        assert_eq!(result.group_b.mean, 1.5);
        assert_eq!(result.effect_group_a_minus_group_b, 7.0);
        assert!((result.studentized_statistic - 9.899_494_936_611_665).abs() < 1e-12);
        assert_eq!(result.p_value, 1.0);
        assert_eq!(result.permutations_completed, 99);
    }

    #[test]
    fn blocked_shuffles_preserve_each_blocks_group_counts() {
        let mut endpoints = endpoints();
        endpoints[0].block = Some("north".into());
        endpoints[2].block = Some("north".into());
        endpoints[1].block = Some("south".into());
        endpoints[3].block = Some("south".into());
        let validated = validate_records(&endpoints, &spec()).expect("records");
        let plan = build_exchangeability_plan(&validated).expect("plan");
        for replicate in 0..50 {
            let labels = restricted_shuffle(&validated, &plan, derive_seed(17, replicate));
            for block in &plan.blocks {
                let before = block
                    .iter()
                    .filter(|index| validated[**index].is_group_a)
                    .count();
                let after = block.iter().filter(|index| labels[**index]).count();
                assert_eq!(after, before);
            }
        }
    }

    #[test]
    fn duplicate_patient_and_confounded_blocks_are_rejected() {
        let mut duplicate = endpoints();
        duplicate[1].patient_id = duplicate[0].patient_id.clone();
        assert!(
            matches!(patient_level_permutation_test(&duplicate, &spec()), Err(CohortInferenceError::InvalidInput(message)) if message.contains("duplicate patient_id"))
        );
        let mut confounded = endpoints();
        for row in &mut confounded {
            row.block = Some(row.group.clone());
        }
        assert!(
            matches!(patient_level_permutation_test(&confounded, &spec()), Err(CohortInferenceError::InvalidInput(message)) if message.contains("fully confounded"))
        );
    }
}
