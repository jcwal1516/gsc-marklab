pub fn marked_count(labels: &[u8]) -> usize {
    labels.iter().filter(|value| **value == 1).count()
}

use crate::errors::{MarklabError, Result};

use super::rng::splitmix64;

pub fn permute_fixed_count(n: usize, n_marked: usize, seed: u64) -> Result<Vec<u8>> {
    let mut indices = Vec::with_capacity(n);
    let mut labels = Vec::with_capacity(n);
    permute_fixed_count_into(n, n_marked, seed, &mut indices, &mut labels)?;
    Ok(labels)
}

pub(crate) fn permute_fixed_count_into(
    n: usize,
    n_marked: usize,
    seed: u64,
    indices: &mut Vec<usize>,
    labels: &mut Vec<u8>,
) -> Result<()> {
    permute_fixed_count_indices_into(n, n_marked, seed, indices)?;
    labels.clear();
    labels.resize(n, 0);
    for index in indices.iter().copied() {
        labels[index] = 1;
    }
    Ok(())
}

#[cfg(test)]
pub fn permute_fixed_count_indices(n: usize, n_marked: usize, seed: u64) -> Result<Vec<usize>> {
    let mut indices = Vec::with_capacity(n);
    permute_fixed_count_indices_into(n, n_marked, seed, &mut indices)?;
    Ok(indices)
}

pub fn permute_fixed_count_indices_into(
    n: usize,
    n_marked: usize,
    seed: u64,
    indices: &mut Vec<usize>,
) -> Result<()> {
    if n_marked > n {
        return Err(MarklabError::Validation(
            "n_marked cannot exceed n in fixed-count permutation".into(),
        ));
    }

    indices.clear();
    indices.extend(0..n);
    deterministic_shuffle(indices.as_mut_slice(), seed);
    indices.truncate(n_marked);
    indices.sort_unstable();
    Ok(())
}

pub(crate) fn deterministic_shuffle<T>(values: &mut [T], seed: u64) {
    let mut state = seed;
    for i in (1..values.len()).rev() {
        state = splitmix64(state ^ i as u64);
        let j = (state % (i as u64 + 1)) as usize;
        values.swap(i, j);
    }
}
