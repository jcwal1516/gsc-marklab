use std::collections::BTreeMap;

use thiserror::Error;

use super::{
    splitmix64, CohortInferenceError, PatientPermutationSpec, PermutationAlternative,
    ValidatedRecord, MAXIMUM_PERMUTATIONS, PATIENT_PERMUTATION_NAMESPACE,
};

const RANDOM_LABELING_NAMESPACE: u64 = 0x7261_6e64_6c61_6265;
const STRATIFIED_RANDOM_LABELING_NAMESPACE: u64 = 0x7374_7261_746c_6162;

pub(crate) fn validate_two_group_labels(
    group_a: &str,
    group_b: &str,
) -> Result<(), CohortInferenceError> {
    if group_a.trim().is_empty() || group_b.trim().is_empty() {
        return Err(CohortInferenceError::InvalidInput(
            "group labels must be non-empty".into(),
        ));
    }
    if group_a.trim() != group_a || group_b.trim() != group_b {
        return Err(CohortInferenceError::InvalidInput(
            "group labels may not have surrounding whitespace".into(),
        ));
    }
    if group_a == group_b {
        return Err(CohortInferenceError::InvalidInput(
            "group labels must be distinct".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_permutation_count(permutations: usize) -> Result<(), CohortInferenceError> {
    if permutations == 0 || permutations > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    Ok(())
}

mod declarations;
mod patient_blocks;
mod schedule;

pub use declarations::{
    InferenceAlternative, InferenceAnalysisLevel, InferenceDesign, InferenceDesignError,
    InferenceMultiplicity, InferenceNullFamily, InferencePermutationUnit,
    PatientExchangeabilityBlock,
};
pub(super) use patient_blocks::PatientPermutationDesign;
pub(crate) use patient_blocks::{align_patient_blocks, compile_blocked_population_independence};

#[cfg(test)]
mod tests;
