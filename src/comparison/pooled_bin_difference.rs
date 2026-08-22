use crate::{
    common::seeds::{derive_seed, SeedEndpoint},
    comparison::curves::{max_abs_standardized_difference, validate_curves},
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value_with_spec, PermutationTestSpec, Tail},
    output::{CurveComparisonAvailability, CurveComparisonResult},
    permutation::labels::deterministic_shuffle,
};

/// Compare two already-aggregated curves with a pooled-bin permutation diagnostic.
///
/// The returned p-value permutes pooled curve-bin values. It is an approximate
/// diagnostic for curve-level difference, not a spatial or per-cell permutation
/// test, and a non-significant result is not proof that curves are the same.
pub fn pooled_bin_difference_diagnostic(
    comparison_name: &str,
    a: &[f64],
    b: &[f64],
    permutations: usize,
    seed: u64,
) -> Result<CurveComparisonResult> {
    validate_curves(a, b)?;
    if permutations == 0 {
        return Err(MarklabError::Config(
            "pooled-bin difference diagnostic permutations must be greater than zero".into(),
        ));
    }

    let statistic = max_abs_standardized_difference(a, b)?;
    let null_statistics = permuted_statistics(a, b, permutations, seed)?;
    let pooled_bin_p_value = permutation_p_value_with_spec(
        statistic,
        &null_statistics,
        PermutationTestSpec::new(Tail::OneSidedHigh, 1),
    )?;

    Ok(CurveComparisonResult {
        comparison_name: comparison_name.to_owned(),
        method: crate::output::CurveComparisonMethod::PooledBinPermutation,
        metric: "max_abs_standardized_difference".into(),
        availability: CurveComparisonAvailability::Available,
        statistic: Some(statistic),
        unavailable_reason: None,
        pooled_bin_p_value: Some(pooled_bin_p_value),
        margin: None,
        within_margin: None,
        interpretation: if pooled_bin_p_value < 0.05 {
            "difference detected by approximate pooled-bin permutation diagnostic; this is not a spatial or per-cell permutation test and does not prove biological causality".into()
        } else {
            "approximate pooled-bin permutation diagnostic was non-significant; this is not a spatial or per-cell permutation test and does not establish equivalence".into()
        },
    })
}

fn permuted_statistics(a: &[f64], b: &[f64], permutations: usize, seed: u64) -> Result<Vec<f64>> {
    let curve_len = a.len();
    let mut pooled = Vec::with_capacity(a.len() + b.len());
    pooled.extend_from_slice(a);
    pooled.extend_from_slice(b);

    let mut null_statistics = Vec::with_capacity(permutations);
    for permutation in 0..permutations {
        let mut shuffled = pooled.clone();
        deterministic_shuffle(
            &mut shuffled,
            derive_seed(seed, SeedEndpoint::PooledBinDifference, permutation),
        );
        let left = &shuffled[..curve_len];
        let right = &shuffled[curve_len..];
        null_statistics.push(max_abs_standardized_difference(left, right)?);
    }
    Ok(null_statistics)
}
