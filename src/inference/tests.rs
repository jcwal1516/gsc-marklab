use crate::{
    inference::scalar_pvalues::{permutation_p_value, Tail},
    permutation::{
        envelopes::GlobalEnvelope,
        labels::{marked_count, permute_fixed_count, permute_fixed_count_indices},
        rng::splitmix64,
        stratified::permute_within_strata,
    },
};
use approx::assert_abs_diff_eq;
use serde::Deserialize;
use statrs::distribution::{Binomial, DiscreteCDF};

#[derive(Debug, Deserialize)]
struct GetErlOracle {
    source: String,
    alternative: String,
    alpha: f64,
    observed: Vec<f64>,
    permutations: Vec<Vec<f64>>,
    erl_depths: Vec<f64>,
    critical_depth: f64,
    lower: Vec<f64>,
    upper: Vec<f64>,
    p_global: f64,
}

#[test]
fn splitmix64_is_deterministic_and_index_dependent() {
    assert_eq!(splitmix64(123), splitmix64(123));
    assert_ne!(splitmix64(123), splitmix64(124));
}

#[test]
fn fixed_count_permutation_preserves_marked_count_and_seed_reproducibility() {
    let first = permute_fixed_count(10, 3, 42).expect("permutation");
    let second = permute_fixed_count(10, 3, 42).expect("permutation");
    let different = permute_fixed_count(10, 3, 43).expect("permutation");

    assert_eq!(marked_count(&first), 3);
    assert_eq!(first, second);
    assert_ne!(first, different);
}

#[test]
fn fixed_count_permutation_indices_match_label_permutation() {
    let labels = permute_fixed_count(12, 5, 123).expect("labels");
    let indices = permute_fixed_count_indices(12, 5, 123).expect("indices");

    assert_eq!(indices.len(), 5);
    assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(indices.iter().all(|index| *index < labels.len()));
    assert_eq!(
        labels
            .iter()
            .enumerate()
            .filter_map(|(index, label)| (*label == 1).then_some(index))
            .collect::<Vec<_>>(),
        indices
    );
}

#[test]
fn fixed_count_permutation_indices_can_reuse_caller_scratch() {
    let labels = permute_fixed_count(12, 5, 123).expect("labels");
    let mut indices = Vec::with_capacity(12);

    crate::permutation::labels::permute_fixed_count_indices_into(12, 5, 123, &mut indices)
        .expect("indices");

    assert_eq!(indices.len(), 5);
    assert!(indices.capacity() >= 12);
    assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        labels
            .iter()
            .enumerate()
            .filter_map(|(index, label)| (*label == 1).then_some(index))
            .collect::<Vec<_>>(),
        indices
    );
}

#[test]
fn stratified_permutation_preserves_mark_count_per_stratum() {
    let labels = [1, 1, 0, 0, 1, 0, 0, 0];
    let strata = [10_u16, 10, 10, 10, 20, 20, 20, 20];

    let permuted = permute_within_strata(&labels, &strata, 123).expect("stratified permutation");

    assert_eq!(permuted.len(), labels.len());
    assert_eq!(marked_count(&permuted[0..4]), 2);
    assert_eq!(marked_count(&permuted[4..8]), 1);
}

#[test]
fn scalar_permutation_p_values_use_plus_one_correction_and_declared_tail() {
    let null = [1.0, 2.0, 3.0, 4.0];

    assert_abs_diff_eq!(
        permutation_p_value(3.5, &null, Tail::OneSidedHigh, 0.5).expect("high-tail p-value"),
        0.4,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        permutation_p_value(1.5, &null, Tail::OneSidedLow, 0.5).expect("low-tail p-value"),
        0.4,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        permutation_p_value(4.0, &null, Tail::TwoSided, 0.5).expect("two-sided p-value"),
        0.8,
        epsilon = 1e-12
    );
}

#[test]
fn scalar_permutation_p_values_reject_undefined_or_underpowered_inputs() {
    assert!(permutation_p_value(f64::NAN, &[1.0], Tail::OneSidedHigh, 0.5).is_err());
    assert!(permutation_p_value(1.0, &[f64::INFINITY], Tail::OneSidedHigh, 0.5).is_err());
    assert!(permutation_p_value(1.0, &[1.0, 2.0], Tail::TwoSided, 0.5).is_err());
}

#[test]
fn global_envelope_matches_get_1_0_7_erl_oracle() {
    let oracle: GetErlOracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/get_erl_1_0_7_oracle.json"
    ))
    .expect("valid checked-in GET oracle");
    assert_eq!(oracle.source, "CRAN GET 1.0-7 R/forder.r and R/envelopes.r");
    assert_eq!(oracle.alternative, "two.sided");

    let envelope =
        GlobalEnvelope::from_curves(&oracle.observed, &oracle.permutations, oracle.alpha)
            .expect("ERL envelope");

    assert_eq!(envelope.lower, oracle.lower);
    assert_eq!(envelope.upper, oracle.upper);
    assert_eq!(envelope.n_permutations, oracle.permutations.len());
    assert_abs_diff_eq!(envelope.erl_depth, oracle.erl_depths[0], epsilon = 1e-12);
    assert_abs_diff_eq!(
        envelope.critical_depth,
        oracle.critical_depth,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(envelope.p_global, oracle.p_global, epsilon = 1e-12);
}

#[test]
fn global_envelope_rejects_nonfinite_curves_and_unresolvable_alpha() {
    let permutations = vec![vec![1.0], vec![2.0], vec![3.0]];
    assert!(GlobalEnvelope::from_curves(&[f64::NAN], &permutations, 0.25).is_err());
    assert!(GlobalEnvelope::from_curves(&[1.0], &[vec![f64::INFINITY]], 0.5).is_err());
    assert!(GlobalEnvelope::from_curves(&[1.0], &permutations, 0.2).is_err());
}

#[test]
fn deterministic_exchangeable_null_p_values_are_super_uniform_at_exact_binomial_bounds() {
    const SIMULATIONS: usize = 2_000;
    const NULL_DRAWS: usize = 199;
    const CONFIDENCE: f64 = 0.999;

    for tail in [Tail::OneSidedHigh, Tail::TwoSided] {
        let p_values = (0..SIMULATIONS)
            .map(|simulation| {
                let base = (simulation * (NULL_DRAWS + 1)) as u64;
                let observed = unit_interval(splitmix64(base));
                let null = (1..=NULL_DRAWS)
                    .map(|offset| unit_interval(splitmix64(base + offset as u64)))
                    .collect::<Vec<_>>();
                permutation_p_value(observed, &null, tail, 0.05).expect("resolved null p-value")
            })
            .collect::<Vec<_>>();

        for threshold in [0.05, 0.10, 0.20] {
            let rejection_count = p_values
                .iter()
                .filter(|p_value| **p_value <= threshold)
                .count() as u64;
            let exact_upper_count = Binomial::new(threshold, SIMULATIONS as u64)
                .expect("binomial reference")
                .inverse_cdf(CONFIDENCE);
            assert!(
                rejection_count <= exact_upper_count,
                "tail={tail:?}, threshold={threshold}, rejections={rejection_count}, exact upper count={exact_upper_count}"
            );
        }
    }
}

fn unit_interval(bits: u64) -> f64 {
    (bits >> 11) as f64 / (1_u64 << 53) as f64
}
