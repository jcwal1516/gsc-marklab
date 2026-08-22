use crate::{
    common::seeds::{derive_seed, SeedEndpoint},
    comparison::curves::{max_abs_standardized_difference, validate_curves},
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value_with_spec, PermutationTestSpec, Tail},
    output::{CurveTestAvailability, CurveTestResult},
    permutation::labels::deterministic_shuffle,
};

/// Compare two already-aggregated curves with a pooled-bin permutation diagnostic.
///
/// The returned p-value permutes pooled curve-bin values. It is an approximate
/// diagnostic for curve-level difference, not a spatial or per-cell permutation
/// test, and a non-significant result is not proof that curves are the same.
pub fn curve_difference_test(
    comparison_name: &str,
    a: &[f64],
    b: &[f64],
    permutations: usize,
    seed: u64,
) -> Result<CurveTestResult> {
    validate_curves(a, b)?;
    if permutations == 0 {
        return Err(MarklabError::Config(
            "curve difference test permutations must be greater than zero".into(),
        ));
    }

    let statistic = max_abs_standardized_difference(a, b)?;
    let null_statistics = permuted_statistics(a, b, permutations, seed)?;
    let p_difference = permutation_p_value_with_spec(
        statistic,
        &null_statistics,
        PermutationTestSpec::new(Tail::OneSidedHigh, 1),
    )?;

    Ok(CurveTestResult {
        comparison_name: comparison_name.to_owned(),
        metric: "max_abs_standardized_difference".into(),
        availability: CurveTestAvailability::Available,
        statistic: Some(statistic),
        unavailable_reason: None,
        p_difference: Some(p_difference),
        margin: None,
        within_margin: None,
        interpretation: if p_difference < 0.05 {
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
            derive_seed(seed, SeedEndpoint::CurveDifference, permutation),
        );
        let left = &shuffled[..curve_len];
        let right = &shuffled[curve_len..];
        null_statistics.push(max_abs_standardized_difference(left, right)?);
    }
    Ok(null_statistics)
}
