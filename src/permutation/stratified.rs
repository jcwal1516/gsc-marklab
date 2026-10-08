use std::collections::BTreeMap;

use crate::{
    common::seeds::splitmix64,
    errors::{MarklabError, Result},
};

use super::labels::{deterministic_shuffle, marked_count};

#[derive(Clone, Debug)]
pub(crate) struct StratifiedPermutationPlan {
    source_labels: Box<[u8]>,
    groups: Vec<StratumGroup>,
    maximum_stratum_size: usize,
}

#[derive(Clone, Debug)]
struct StratumGroup {
    seed_namespace: u64,
    indices: Box<[usize]>,
}

impl StratifiedPermutationPlan {
    pub(crate) fn new<T>(labels: &[u8], strata: &[T]) -> Result<Self>
    where
        T: Copy + Ord + Into<u64>,
    {
        validate_inputs(labels, strata)?;
        let mut by_stratum: BTreeMap<T, Vec<usize>> = BTreeMap::new();
        for (index, stratum) in strata.iter().copied().enumerate() {
            by_stratum.entry(stratum).or_default().push(index);
        }
        let maximum_stratum_size = by_stratum.values().map(Vec::len).max().unwrap_or(0);
        let groups = by_stratum
            .into_iter()
            .map(|(stratum, indices)| StratumGroup {
                seed_namespace: stratum.into(),
                indices: indices.into_boxed_slice(),
            })
            .collect();
        Ok(Self {
            source_labels: labels.to_vec().into_boxed_slice(),
            groups,
            maximum_stratum_size,
        })
    }

    pub(crate) fn maximum_stratum_size(&self) -> usize {
        self.maximum_stratum_size
    }

    pub(crate) fn permute_into(
        &self,
        seed: u64,
        output: &mut Vec<u8>,
        stratum_labels: &mut Vec<u8>,
    ) -> Result<()> {
        output.clear();
        output.resize(self.source_labels.len(), 0);
        for group in &self.groups {
            stratum_labels.clear();
            stratum_labels.extend(group.indices.iter().map(|index| self.source_labels[*index]));
            deterministic_shuffle(stratum_labels, splitmix64(seed ^ group.seed_namespace));

            debug_assert_eq!(
                marked_count(stratum_labels),
                group
                    .indices
                    .iter()
                    .filter(|index| self.source_labels[**index] == 1)
                    .count()
            );
            for (index, label) in group
                .indices
                .iter()
                .copied()
                .zip(stratum_labels.iter().copied())
            {
                output[index] = label;
            }
        }
        Ok(())
    }
}

pub fn permute_within_strata<T>(labels: &[u8], strata: &[T], seed: u64) -> Result<Vec<u8>>
where
    T: Copy + Ord + Into<u64>,
{
    let plan = StratifiedPermutationPlan::new(labels, strata)?;
    let mut output = Vec::with_capacity(labels.len());
    let mut stratum_labels = Vec::with_capacity(plan.maximum_stratum_size());
    plan.permute_into(seed, &mut output, &mut stratum_labels)?;
    Ok(output)
}

fn validate_inputs<T>(labels: &[u8], strata: &[T]) -> Result<()> {
    if labels.len() != strata.len() {
        return Err(MarklabError::Validation(
            "labels and strata must have equal length".into(),
        ));
    }
    if labels.iter().any(|value| *value != 0 && *value != 1) {
        return Err(MarklabError::Validation(
            "labels must be binary for stratified permutation".into(),
        ));
    }
    Ok(())
}
