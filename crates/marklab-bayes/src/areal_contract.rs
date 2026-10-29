use serde::{Deserialize, Serialize};

use crate::sar_fit::design_full_rank;
use crate::validation::{all_finite, diagnostics_satisfy_policy};
use crate::{
    BayesError, DiagnosticPolicy, NutsSamplingSpec, PosteriorDiagnostics, SamplingSummary,
    ScalarSummary,
};

pub(crate) struct ArealSpec<'a> {
    pub region_ids: &'a [String],
    pub weights_digest_sha256: &'a str,
    pub transform: &'a [f64],
    pub rank_deficiency: usize,
    pub components: &'a [Vec<usize>],
    pub counts_len: usize,
    pub expected: &'a [f64],
    pub design: &'a [f64],
    pub predictor_names: &'a [String],
}

pub(crate) fn validate_spec(spec: ArealSpec<'_>, model: &str) -> Result<(), BayesError> {
    let dimension = spec.region_ids.len();
    let predictors = spec.predictor_names.len();
    let constrained_dimension = dimension.saturating_sub(spec.rank_deficiency);
    if !(6..=64).contains(&dimension)
        || !(1..=16).contains(&predictors)
        || spec.counts_len != dimension
        || spec.expected.len() != dimension
        || spec.design.len() != dimension * predictors
        || spec.transform.len() != dimension * constrained_dimension
        || spec.rank_deficiency == 0
        || spec.rank_deficiency >= dimension
        || spec.components.len() != spec.rank_deficiency
    {
        return Err(BayesError::InvalidSpec(format!(
            "{model} input dimensions violate the region, predictor, or ICAR contract"
        )));
    }
    for (index, region_id) in spec.region_ids.iter().enumerate() {
        if region_id.is_empty()
            || region_id.trim() != region_id
            || (index > 0 && spec.region_ids[index - 1] >= *region_id)
        {
            return Err(BayesError::InvalidSpec(format!(
                "{model} region IDs must be exact and strictly increasing"
            )));
        }
    }
    for (index, predictor) in spec.predictor_names.iter().enumerate() {
        if predictor.is_empty()
            || predictor.trim() != predictor
            || predictor == "intercept"
            || spec.predictor_names[..index].contains(predictor)
        {
            return Err(BayesError::InvalidSpec(format!(
                "{model} predictor names must be exact, unique, and not intercept"
            )));
        }
    }
    if spec
        .expected
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || spec.design.iter().any(|value| !value.is_finite())
        || spec.transform.iter().any(|value| !value.is_finite())
        || !design_full_rank(spec.design, dimension, predictors)
    {
        return Err(BayesError::InvalidSpec(format!(
            "{model} expected counts, design, transform, or design rank are invalid"
        )));
    }
    let mut covered = vec![false; dimension];
    for component in spec.components {
        if component.len() < 2 {
            return Err(BayesError::InvalidSpec(format!(
                "{model} ICAR components must contain at least two regions"
            )));
        }
        for &index in component {
            if index >= dimension || std::mem::replace(&mut covered[index], true) {
                return Err(BayesError::InvalidSpec(format!(
                    "{model} ICAR components must partition all regions exactly once"
                )));
            }
        }
        for column in 0..constrained_dimension {
            let sum = component
                .iter()
                .map(|&row| spec.transform[row * constrained_dimension + column])
                .sum::<f64>();
            if sum.abs() > 1e-10 {
                return Err(BayesError::InvalidSpec(format!(
                    "{model} ICAR transform violates a component sum-to-zero constraint"
                )));
            }
        }
    }
    if covered.iter().any(|covered| !covered) {
        return Err(BayesError::InvalidSpec(format!(
            "{model} ICAR components do not cover every region"
        )));
    }
    if spec.weights_digest_sha256.len() != 64
        || !spec
            .weights_digest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(BayesError::InvalidSpec(format!(
            "{model} requires a SHA-256 weights identity"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BymPosteriorPredictive {
    pub observed_total_count: u64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
    pub observed_zero_regions: u64,
    pub replicated_zero_regions_mean: f64,
}

pub(crate) fn validate_sampling(
    actual: &SamplingSummary,
    expected: &NutsSamplingSpec,
    coefficient_count: usize,
    expected_coefficients: usize,
    region_count: usize,
    expected_regions: usize,
    model: &str,
) -> Result<(), BayesError> {
    let expected_draws = u64::from(expected.chains) * u64::from(expected.draws_per_chain);
    if actual.chains != expected.chains
        || actual.tune_per_chain != expected.tune_per_chain
        || actual.draws_per_chain != expected.draws_per_chain
        || actual.completed_draws != expected_draws
        || coefficient_count != expected_coefficients
        || region_count != expected_regions
    {
        return Err(BayesError::WorkerContract(format!(
            "{model} sampling, coefficient, or region counts mismatch"
        )));
    }
    Ok(())
}

pub(crate) fn validate_scalar(
    summary: &ScalarSummary,
    positive: bool,
    model: &str,
) -> Result<(), BayesError> {
    if !all_finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || (positive && (summary.mean <= 0.0 || summary.interval_lower <= 0.0))
    {
        return Err(BayesError::WorkerContract(format!(
            "invalid {model} scalar summary"
        )));
    }
    Ok(())
}

pub(crate) fn validate_predictive_and_diagnostics(
    predictive: &BymPosteriorPredictive,
    diagnostics: &PosteriorDiagnostics,
    counts: &[u64],
    policy: &DiagnosticPolicy,
    model: &str,
) -> Result<bool, BayesError> {
    let observed_total = counts.iter().sum::<u64>();
    let observed_zeros = counts.iter().filter(|&&count| count == 0).count() as u64;
    if predictive.observed_total_count != observed_total
        || predictive.observed_zero_regions != observed_zeros
        || !all_finite(&[
            predictive.replicated_total_mean,
            predictive.replicated_total_sd,
            predictive.replicated_zero_regions_mean,
            diagnostics.r_hat,
            diagnostics.ess_bulk,
            diagnostics.ess_tail,
            diagnostics.mcse_mean,
            diagnostics.mcse_sd,
            diagnostics.minimum_ebfmi,
        ])
        || predictive.replicated_total_sd < 0.0
        || predictive.replicated_zero_regions_mean < 0.0
        || diagnostics.r_hat <= 0.0
        || diagnostics.ess_bulk <= 0.0
        || diagnostics.ess_tail <= 0.0
        || diagnostics.mcse_mean < 0.0
        || diagnostics.mcse_sd < 0.0
        || diagnostics.minimum_ebfmi < 0.0
    {
        return Err(BayesError::WorkerContract(format!(
            "{model} posterior predictive or diagnostics are invalid"
        )));
    }
    Ok(diagnostics_satisfy_policy(diagnostics, policy))
}

pub(crate) fn component_constraints(
    components: &[Vec<usize>],
    region_ids: &[String],
) -> Vec<String> {
    components
        .iter()
        .map(|component| {
            format!(
                "sum_to_zero:{}",
                component
                    .iter()
                    .map(|&index| region_ids[index].as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect()
}
