use std::collections::BTreeMap;

use crate::errors::{MmrspaceError, Result};

use super::labels::{deterministic_shuffle, marked_count};
use super::rng::splitmix64;

pub fn permute_within_strata<T>(labels: &[u8], strata: &[T], seed: u64) -> Result<Vec<u8>>
where
    T: Copy + Ord + Into<u64>,
{
    if labels.len() != strata.len() {
        return Err(MmrspaceError::Validation(
            "labels and strata must have equal length".into(),
        ));
    }
    if labels.iter().any(|value| *value != 0 && *value != 1) {
        return Err(MmrspaceError::Validation(
            "labels must be binary for stratified permutation".into(),
        ));
    }

    let mut by_stratum: BTreeMap<T, Vec<usize>> = BTreeMap::new();
    for (index, stratum) in strata.iter().copied().enumerate() {
        by_stratum.entry(stratum).or_default().push(index);
    }

    let mut output = vec![0_u8; labels.len()];
    for (stratum, indices) in by_stratum {
        let mut stratum_labels = indices
            .iter()
            .map(|index| labels[*index])
            .collect::<Vec<_>>();
        let stratum_seed = splitmix64(seed ^ stratum.into());
        deterministic_shuffle(&mut stratum_labels, stratum_seed);

        debug_assert_eq!(
            marked_count(&stratum_labels),
            indices.iter().filter(|index| labels[**index] == 1).count()
        );

        for (index, label) in indices.into_iter().zip(stratum_labels) {
            output[index] = label;
        }
    }

    Ok(output)
}
