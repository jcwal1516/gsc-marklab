use std::collections::BTreeMap;

use crate::{
    common::seeds::splitmix64,
    errors::{MarklabError, Result},
    multimodal::cells::CellSection,
    permutation::labels::deterministic_shuffle,
};

#[derive(Clone, Copy, Debug)]
enum SeedPolicy {
    SourceSections,
    ExplicitStrata,
}

#[derive(Clone, Debug)]
struct PermutationGroup {
    ordinal: usize,
    indices: Box<[usize]>,
}

/// Fixed grouping plan for repeated compact-label permutations.
#[derive(Clone, Debug)]
pub(crate) struct LabelPermutationPlan {
    len: usize,
    groups: Box<[PermutationGroup]>,
    maximum_group_size: usize,
    seed_policy: SeedPolicy,
}

impl LabelPermutationPlan {
    pub(crate) fn for_source_sections(sections: &[CellSection]) -> Self {
        let strata = sections
            .iter()
            .map(|section| match section {
                CellSection::He => 0_u8,
                CellSection::Ihc => 1_u8,
            })
            .collect::<Vec<_>>();
        Self::new(&strata, SeedPolicy::SourceSections)
    }

    pub(crate) fn for_explicit_strata<T: Ord>(strata: &[T]) -> Self {
        Self::new(strata, SeedPolicy::ExplicitStrata)
    }

    fn new<T: Ord>(strata: &[T], seed_policy: SeedPolicy) -> Self {
        let mut by_stratum = BTreeMap::<&T, Vec<usize>>::new();
        for (index, stratum) in strata.iter().enumerate() {
            by_stratum.entry(stratum).or_default().push(index);
        }
        let maximum_group_size = by_stratum.values().map(Vec::len).max().unwrap_or(0);
        let groups = by_stratum
            .into_values()
            .enumerate()
            .map(|(ordinal, indices)| PermutationGroup {
                ordinal,
                indices: indices.into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            len: strata.len(),
            groups,
            maximum_group_size,
            seed_policy,
        }
    }

    pub(crate) fn maximum_group_size(&self) -> usize {
        self.maximum_group_size
    }

    pub(crate) fn permute_into<T: Copy>(
        &self,
        labels: &[T],
        seed: u64,
        output: &mut Vec<T>,
        group_scratch: &mut Vec<T>,
    ) -> Result<()> {
        if labels.len() != self.len {
            return Err(MarklabError::Validation(format!(
                "label permutation plan has {} entries for {} labels",
                self.len,
                labels.len()
            )));
        }
        output.clear();
        output.extend_from_slice(labels);
        for group in &self.groups {
            group_scratch.clear();
            group_scratch.extend(group.indices.iter().map(|index| labels[*index]));
            deterministic_shuffle(group_scratch, self.seed_for(seed, group.ordinal));
            for (index, label) in group
                .indices
                .iter()
                .copied()
                .zip(group_scratch.iter().copied())
            {
                output[index] = label;
            }
        }
        Ok(())
    }

    fn seed_for(&self, seed: u64, ordinal: usize) -> u64 {
        match self.seed_policy {
            SeedPolicy::SourceSections if ordinal == 0 => seed,
            SeedPolicy::SourceSections => splitmix64(seed),
            SeedPolicy::ExplicitStrata => splitmix64(seed ^ ordinal as u64),
        }
    }
}
