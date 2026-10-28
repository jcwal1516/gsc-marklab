//! Ensembles, plug-in joint entropy and co-information (R `entropy` and `measure_cisMI`).

/// Binomial coefficient, saturating at `u64::MAX`.
pub(crate) fn binomial(n: usize, k: usize) -> u64 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut value = 1_u128;
    for step in 0..k {
        value = value * (n - step) as u128 / (step + 1) as u128;
        if value > u64::MAX as u128 {
            return u64::MAX;
        }
    }
    value as u64
}

/// Every `size`-subset of `0..count` in lexicographic order (R `combn`), flattened.
pub(crate) fn combinations(count: usize, size: usize) -> Vec<u16> {
    let total = binomial(count, size) as usize;
    let mut flat = Vec::with_capacity(total * size);
    if size == 0 || size > count {
        return flat;
    }
    let mut current = (0..size as u16).collect::<Vec<_>>();
    loop {
        flat.extend_from_slice(&current);
        let Some(position) = (0..size)
            .rev()
            .find(|&position| current[position] as usize != count - size + position)
        else {
            return flat;
        };
        current[position] += 1;
        for next in position + 1..size {
            current[next] = current[next - 1] + 1;
        }
    }
}

/// Position of a sorted subset of `0..count` in [`combinations`] order.
pub(crate) fn lexicographic_rank(subset: &[u16], count: usize) -> usize {
    let size = subset.len();
    let mut rank = 0_u64;
    let mut start = 0_usize;
    for (position, &element) in subset.iter().enumerate() {
        let remaining = size - position;
        rank += binomial(count - start, remaining) - binomial(count - element as usize, remaining);
        start = element as usize + 1;
    }
    rank as usize
}

/// Plug-in joint entropy in bits of binned columns over the same discs (R `entropy`).
///
/// Bins combine into the code `sum(bin_i * bins^i)`; counts are accumulated in a dense
/// histogram and summed in increasing code order, the order of R's `table`.
pub(crate) fn joint_entropy(columns: &[&[u8]], bins: usize, histogram: &mut Vec<u32>) -> f64 {
    let size = bins.pow(columns.len() as u32);
    histogram.clear();
    histogram.resize(size, 0);
    let rows = columns[0].len();
    match columns {
        [first] => first.iter().for_each(|&bin| histogram[bin as usize] += 1),
        [first, second] => {
            for row in 0..rows {
                histogram[first[row] as usize + bins * second[row] as usize] += 1;
            }
        }
        _ => {
            for row in 0..rows {
                let mut code = 0_usize;
                let mut scale = 1_usize;
                for column in columns {
                    code += scale * column[row] as usize;
                    scale *= bins;
                }
                histogram[code] += 1;
            }
        }
    }
    let total = rows as f64;
    -histogram
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let probability = count as f64 / total;
            probability * probability.log2()
        })
        .sum::<f64>()
}

/// Co-information of an ensemble from the entropies of all its nonempty subsets:
/// `sum over subsets T of (-1)^(|T| - 1) H(T)`, accumulated by subset size as R does.
/// `entropies(subset)` returns one entropy per census.
pub(crate) fn co_information<'a>(
    ensemble: &[u16],
    censuses: usize,
    mut entropies: impl FnMut(&[u16]) -> &'a [f64],
    output: &mut [f64],
) {
    output.fill(0.0);
    let mut by_size = vec![0.0; censuses];
    let mut subset = Vec::with_capacity(ensemble.len());
    for size in 1..=ensemble.len() {
        by_size.fill(0.0);
        let positions = combinations(ensemble.len(), size);
        for chosen in positions.chunks_exact(size) {
            subset.clear();
            subset.extend(chosen.iter().map(|&position| ensemble[position as usize]));
            for (sum, entropy) in by_size.iter_mut().zip(entropies(&subset)) {
                *sum += entropy;
            }
        }
        let sign = if size % 2 == 1 { 1.0 } else { -1.0 };
        for (value, sum) in output.iter_mut().zip(&by_size) {
            *value += sign * sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinations_are_lexicographic_and_ranked() {
        let flat = combinations(4, 2);
        assert_eq!(flat, vec![0, 1, 0, 2, 0, 3, 1, 2, 1, 3, 2, 3]);
        for (rank, subset) in flat.chunks_exact(2).enumerate() {
            assert_eq!(lexicographic_rank(subset, 4), rank);
        }
        let flat = combinations(7, 3);
        assert_eq!(flat.len() / 3, 35);
        for (rank, subset) in flat.chunks_exact(3).enumerate() {
            assert_eq!(lexicographic_rank(subset, 7), rank);
        }
        assert_eq!(binomial(1024, 3), 178_433_024);
    }

    #[test]
    fn entropy_and_co_information_match_hand_calculation() {
        let mut histogram = Vec::new();
        // Two independent fair bits: H(A) = H(B) = 1, H(A, B) = 2, I(A; B) = 0.
        let a = [0_u8, 0, 1, 1];
        let b = [0_u8, 1, 0, 1];
        assert_eq!(joint_entropy(&[&a], 2, &mut histogram), 1.0);
        assert_eq!(joint_entropy(&[&a, &b], 2, &mut histogram), 2.0);
        // A copy: H(A, A) = 1, I(A; A) = 1.
        assert_eq!(joint_entropy(&[&a, &a], 2, &mut histogram), 1.0);
        // XOR: pairwise independent, three-way co-information -1.
        let c = [0_u8, 1, 1, 0];
        let columns = [&a[..], &b[..], &c[..]];
        let mut entropy = |subset: &[u16]| -> f64 {
            let selected = subset
                .iter()
                .map(|&index| columns[index as usize])
                .collect::<Vec<_>>();
            joint_entropy(&selected, 2, &mut Vec::new())
        };
        let mut table = std::collections::BTreeMap::new();
        for size in 1..=3 {
            for subset in combinations(3, size).chunks_exact(size) {
                table.insert(subset.to_vec(), vec![entropy(subset)]);
            }
        }
        let mut output = [0.0];
        co_information(&[0, 1, 2], 1, |subset| &table[subset], &mut output);
        assert_eq!(output, [-1.0]);
        co_information(&[0, 1], 1, |subset| &table[subset], &mut output);
        assert_eq!(output, [0.0]);
    }
}
