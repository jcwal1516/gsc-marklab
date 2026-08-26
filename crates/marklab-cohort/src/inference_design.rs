use std::collections::BTreeMap;

use super::{
    derive_seed, splitmix64, CohortInferenceError, PatientPermutationSpec, PermutationAlternative,
    ValidatedRecord,
};

/// Validated patient hierarchy and execution controls for one scalar endpoint.
///
/// The type is deliberately private: it compiles the existing public scalar
/// permutation inputs into the one design that workflow actually supports.
pub(super) struct PatientPermutationDesign {
    blocks: Vec<Vec<usize>>,
    blocked: bool,
    permutations: usize,
    seed: u64,
    alternative: PermutationAlternative,
}

impl PatientPermutationDesign {
    pub(super) fn compile(
        records: &[ValidatedRecord],
        spec: &PatientPermutationSpec,
    ) -> Result<Self, CohortInferenceError> {
        let declared_blocks = records
            .iter()
            .filter(|record| {
                record
                    .block
                    .as_deref()
                    .is_some_and(|block| !block.is_empty())
            })
            .count();
        if declared_blocks != 0 && declared_blocks != records.len() {
            return Err(CohortInferenceError::InvalidInput(
                "exchangeability blocks must be declared for every patient or no patients".into(),
            ));
        }

        let mut by_block = BTreeMap::<&str, Vec<usize>>::new();
        for (index, record) in records.iter().enumerate() {
            by_block
                .entry(record.block.as_deref().unwrap_or_default())
                .or_default()
                .push(index);
        }
        let blocked = declared_blocks != 0;
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

        Ok(Self {
            blocks: by_block.into_values().collect(),
            blocked,
            permutations: spec.permutations,
            seed: spec.seed,
            alternative: spec.alternative,
        })
    }

    pub(super) fn blocked(&self) -> bool {
        self.blocked
    }

    pub(super) fn block_count(&self) -> usize {
        self.blocks.len()
    }

    #[cfg(test)]
    pub(super) fn blocks(&self) -> &[Vec<usize>] {
        &self.blocks
    }

    pub(super) fn permutations(&self) -> usize {
        self.permutations
    }

    pub(super) fn seed(&self) -> u64 {
        self.seed
    }

    pub(super) fn alternative(&self) -> PermutationAlternative {
        self.alternative
    }

    pub(super) fn permuted_labels(
        &self,
        records: &[ValidatedRecord],
        replicate: usize,
    ) -> Vec<bool> {
        let mut labels = records
            .iter()
            .map(|record| record.is_group_a)
            .collect::<Vec<_>>();
        let mut state = derive_seed(self.seed, replicate);
        for indices in &self.blocks {
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
}
