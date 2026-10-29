//! Coregionalized replicated multitype LGCP with cross-type correlated latent fields (NumPyro).
//!
//! Likelihood, quadrature, fixed effects, hierarchy priors and centring are those of
//! `replicated_arbitrary_window_multitype_lgcp`. In each pattern `latent = diag(field_scale) C Z`,
//! where `Z = field_raw chol(K + jitter I)^T` is centred within the pattern and `K` is a
//! unit-variance Matern-3/2 kernel with length `field_length_scale_um`, so `Cov(f_t(x), f_u(y))
//! = field_scale_t field_scale_u (C C^T)_tu (K + jitter I)(x, y)` and jitter scales with
//! `field_scale^2`. Priors: `field_scale_t ~ HalfNormal(marginal_field_scale)`, `C ~
//! LKJCholesky(types, lkj_concentration)` with concentration at least 1, and
//! `field_length_scale_um ~ HalfNormal(field_length_scale_scale_um)`. Posterior correlation
//! matrices must have unit diagonal and be positive definite. A complete fit claims
//! `experimental_correlated_multitype_latent_field`; the zero-correlation null is reported as a
//! descriptor and is not tested.

use super::posterior_validation::{diagnostics_pass, summary_valid};
use super::replicated_arbitrary_window_multitype_lgcp::pair_differences_valid;
use super::replicated_arbitrary_window_multitype_lgcp::type_posteriors_valid;
use super::replicated_arbitrary_window_multitype_lgcp::PairDifference;
use super::replicated_arbitrary_window_multitype_lgcp::PatternTypePredictive;
use super::replicated_arbitrary_window_multitype_lgcp::TypePosterior;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel as inferred_kernel;
use crate::backend_versions::JAX_VERSION;
use crate::sha256_hex;
use crate::BackendContract;
use crate::BayesError;
use crate::DiagnosticPolicy;
use crate::FitState;
use crate::McseScalarSummary;
use crate::PosteriorDiagnostics;
use crate::SamplingSummary;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;

pub struct ValidationContext<'a> {
    pub source_value: &'a serde_json::Value,
    pub backend: &'a BackendContract,
    pub source_request_bytes: &'a [u8],
    pub request_sha256: &'a str,
    pub input_sha256: &'a str,
    pub correlation_prior: &'a CorrelationPrior,
    pub resources: &'a Resources,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorrelationPrior {
    pub lkj_concentration: f64,
    pub marginal_field_scale: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub coregionalization_work: u64,
    pub maximum_coregionalization_work: u64,
    pub maximum_output_bytes: usize,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: &'a str,
    pub source_request: serde_json::Value,
    pub correlation_prior: CorrelationPrior,
    pub resources: Resources,
    pub maximum_tree_depth: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeFieldScale {
    pub type_id: String,
    pub scale: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CrossTypeCorrelation {
    pub type_a: String,
    pub type_b: String,
    pub correlation: McseScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoregionalizationOracle {
    pub target_correlation_matrix: Vec<f64>,
    pub reconstructed_correlation_matrix: Vec<f64>,
    pub maximum_absolute_error: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub field_length_scale_um: McseScalarSummary,
    pub type_field_scales: Vec<TypeFieldScale>,
    pub cross_type_correlations: Vec<CrossTypeCorrelation>,
    pub posterior_mean_correlation_matrix: Vec<f64>,
    pub type_posteriors: Vec<TypePosterior>,
    pub group_effect_differences: Vec<PairDifference>,
    pub pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub coregionalization_oracle: CoregionalizationOracle,
    pub coregionalization_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub input_sha256: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub field_length_scale_um: McseScalarSummary,
    pub type_field_scales: Vec<TypeFieldScale>,
    pub cross_type_correlations: Vec<CrossTypeCorrelation>,
    pub posterior_mean_correlation_matrix: Vec<f64>,
    pub type_posteriors: Vec<TypePosterior>,
    pub group_effect_differences: Vec<PairDifference>,
    pub pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    pub diagnostics: PosteriorDiagnostics,
    pub coregionalization_oracle: CoregionalizationOracle,
    pub correlation_prior: CorrelationPrior,
    pub resources: Resources,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub type_count: usize,
    pub total_node_count: usize,
    pub total_event_count: u64,
    pub statistical_unit: StatisticalUnit,
    pub pattern_unit: String,
    pub likelihood: String,
    pub null_model: NullHypothesis,
    pub correlation_parameterization: String,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        if self.format != "marklab.correlated_replicated_arbitrary_window_multitype_lgcp"
            || self.version != 1
            || self.request_sha256 != expected.request_sha256
            || self.source_request_sha256 != sha256_hex(expected.source_request_bytes)
            || self.input_sha256 != expected.input_sha256
            || self.correlation_prior != *expected.correlation_prior
            || self.resources != *expected.resources
            || self.statistical_unit != StatisticalUnit::Patient
            || self.correlation_parameterization
                != "lower_triangular_lkj_cholesky_with_positive_marginal_field_scales"
        {
            return Err(BayesError::WorkerContract(
                "durable correlated multitype LGCP result differs".into(),
            ));
        }
        validate_correlation_matrix(&self.posterior_mean_correlation_matrix, self.type_count)
    }
}

pub fn validate_worker(
    result: &WorkerResult,
    expected: &ValidationContext<'_>,
) -> Result<(), BayesError> {
    let fixed = &expected.source_value["source_request"];
    let types = fixed["type_ids"]
        .as_array()
        .ok_or_else(|| BayesError::WorkerContract("correlated type identities differ".into()))?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let complete_draws = fixed["sampling"]["chains"].as_u64().unwrap_or(0)
        * fixed["sampling"]["draws_per_chain"].as_u64().unwrap_or(0);
    let policy = DiagnosticPolicy::default();
    let diagnostics_pass = diagnostics_pass(&result.diagnostics, &policy);
    let expected_pairs = types.len() * (types.len() - 1) / 2;
    let correlation_pairs = result.cross_type_correlations.len() == expected_pairs
        && result.cross_type_correlations.iter().all(|row| {
            types.contains(&row.type_a)
                && types.contains(&row.type_b)
                && row.type_a < row.type_b
                && summary_valid(&row.correlation)
                && (-1.0..=1.0).contains(&row.correlation.mean)
                && (-1.0..=1.0).contains(&row.correlation.interval_lower)
                && (-1.0..=1.0).contains(&row.correlation.interval_upper)
        });
    let scale_types = result
        .type_field_scales
        .iter()
        .map(|row| row.type_id.as_str())
        .collect::<BTreeSet<_>>();
    if result.format
        != "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_result"
        || result.version != 1
        || !result.backend.matches(expected.backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != expected.input_sha256
        || result.request_sha256 != expected.request_sha256
        || result.source_request_sha256 != sha256_hex(expected.source_request_bytes)
        || result.sampling.completed_draws != complete_draws
        || result.coregionalization_work != expected.resources.coregionalization_work
        || !summary_valid(&result.field_length_scale_um)
        || result.field_length_scale_um.mean <= 0.0
        || result.type_field_scales.len() != types.len()
        || scale_types.len() != types.len()
        || !result.type_field_scales.iter().all(|row| {
            types.contains(&row.type_id) && summary_valid(&row.scale) && row.scale.mean > 0.0
        })
        || !correlation_pairs
        || validate_correlation_matrix(&result.posterior_mean_correlation_matrix, types.len())
            .is_err()
        || !type_posteriors_valid(&result.type_posteriors, &types)
        || !pair_differences_valid(&result.group_effect_differences, &types)
        || !inferred_kernel::predictive_valid(
            &result.pattern_type_posterior_predictive,
            fixed,
            complete_draws,
        )
        || result
            .coregionalization_oracle
            .target_correlation_matrix
            .len()
            != 9
        || result
            .coregionalization_oracle
            .reconstructed_correlation_matrix
            .len()
            != 9
        || !result
            .coregionalization_oracle
            .maximum_absolute_error
            .is_finite()
        || result.coregionalization_oracle.maximum_absolute_error > 1e-12
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesError::WorkerContract(
            "correlated multitype LGCP backend result differs".into(),
        ));
    }
    Ok(())
}

fn validate_correlation_matrix(matrix: &[f64], dimension: usize) -> Result<(), BayesError> {
    if !(2..=8).contains(&dimension) || matrix.len() != dimension * dimension {
        return Err(BayesError::WorkerContract(
            "correlation matrix dimensions differ".into(),
        ));
    }
    let mut cholesky = vec![0.0; matrix.len()];
    for row in 0..dimension {
        for column in 0..=row {
            let value = matrix[row * dimension + column];
            if !value.is_finite()
                || (matrix[column * dimension + row] - value).abs() > 1e-10
                || (row == column && (value - 1.0).abs() > 1e-10)
            {
                return Err(BayesError::WorkerContract(
                    "correlation matrix is not finite symmetric unit diagonal".into(),
                ));
            }
            let prior = (0..column)
                .map(|index| {
                    cholesky[row * dimension + index] * cholesky[column * dimension + index]
                })
                .sum::<f64>();
            if row == column {
                let diagonal = value - prior;
                if diagonal <= 1e-12 {
                    return Err(BayesError::WorkerContract(
                        "correlation matrix is not positive definite".into(),
                    ));
                }
                cholesky[row * dimension + column] = diagonal.sqrt();
            } else {
                cholesky[row * dimension + column] =
                    (value - prior) / cholesky[column * dimension + column];
            }
        }
    }
    Ok(())
}
