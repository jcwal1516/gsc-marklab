use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    model::{BackendContract, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, WorkerBackend,
};

const PSIS_BACKEND_VERSION: &str = "arviz-1.3.0+arviz-stats-1.3.1";
const MAXIMUM_LOG_LIKELIHOOD_VALUES: usize = 500_000;

#[derive(Clone, Debug)]
pub struct PointwiseLogLikelihoodDraw {
    pub chain: u32,
    pub draw: u32,
    pub unit_id: String,
    pub log_likelihood: f64,
}

#[derive(Clone, Debug)]
pub struct PsisLooSpec {
    pub model_name: String,
    pub likelihood_target: String,
    pub data_identity_sha256: String,
    pub preprocessing_identity_sha256: String,
    pub heldout_unit: String,
    pub relative_efficiency: f64,
    pub draws: Vec<PointwiseLogLikelihoodDraw>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PsisLooResourceLimits {
    pub maximum_log_likelihood_values: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PsisLooWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model_name: String,
    pub likelihood_target: String,
    pub data_identity_sha256: String,
    pub preprocessing_identity_sha256: String,
    pub heldout_unit: String,
    pub unit_ids: Vec<String>,
    pub chains: u32,
    pub draws_per_chain: u32,
    pub relative_efficiency: f64,
    pub log_likelihood: Vec<f64>,
    pub resources: PsisLooResourceLimits,
}

impl PsisLooWorkerRequest {
    pub fn new(
        mut spec: PsisLooSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !exact_name(&spec.model_name)
            || !exact_name(&spec.likelihood_target)
            || !sha256(&spec.data_identity_sha256)
            || !sha256(&spec.preprocessing_identity_sha256)
            || spec.heldout_unit.is_empty()
            || spec.heldout_unit.trim() != spec.heldout_unit
            || !spec.relative_efficiency.is_finite()
            || spec.relative_efficiency <= 0.0
            || spec.relative_efficiency > 1.0
            || !(1..=3_600).contains(&timeout_seconds)
            || spec.draws.len() > MAXIMUM_LOG_LIKELIHOOD_VALUES
        {
            return Err(BayesError::InvalidSpec(
                "PSIS-LOO held-out unit, relative efficiency, timeout, or matrix size is invalid"
                    .into(),
            ));
        }
        let unit_ids = spec
            .draws
            .iter()
            .map(|record| record.unit_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !(2..=10_000).contains(&unit_ids.len())
            || unit_ids
                .iter()
                .any(|unit| unit.is_empty() || unit.trim() != unit)
        {
            return Err(BayesError::InvalidSpec(
                "PSIS-LOO requires 2-10000 exact held-out unit IDs".into(),
            ));
        }
        if spec
            .draws
            .iter()
            .any(|record| !record.log_likelihood.is_finite())
        {
            return Err(BayesError::InvalidSpec(
                "PSIS-LOO log-likelihood values must be finite".into(),
            ));
        }
        spec.draws.sort_by(|left, right| {
            (left.chain, left.draw, &left.unit_id).cmp(&(right.chain, right.draw, &right.unit_id))
        });
        let chains = spec
            .draws
            .last()
            .and_then(|record| record.chain.checked_add(1))
            .ok_or_else(|| BayesError::InvalidSpec("PSIS-LOO matrix is empty".into()))?;
        if !(2..=8).contains(&chains) {
            return Err(BayesError::InvalidSpec(
                "PSIS-LOO requires 2-8 zero-based contiguous chains".into(),
            ));
        }
        let unit_count = unit_ids.len();
        if !spec
            .draws
            .len()
            .is_multiple_of(chains as usize * unit_count)
        {
            return Err(BayesError::InvalidSpec(
                "PSIS-LOO matrix is incomplete".into(),
            ));
        }
        let draws_per_chain = spec.draws.len() / (chains as usize * unit_count);
        if !(100..=100_000).contains(&draws_per_chain) {
            return Err(BayesError::InvalidSpec(
                "PSIS-LOO requires 100-100000 draws per chain".into(),
            ));
        }
        let mut index = 0;
        for chain in 0..chains {
            for draw in 0..draws_per_chain as u32 {
                for unit_id in &unit_ids {
                    let record = &spec.draws[index];
                    if record.chain != chain || record.draw != draw || record.unit_id != *unit_id {
                        return Err(BayesError::InvalidSpec(
                            "PSIS-LOO matrix must contain every chain/draw/unit exactly once"
                                .into(),
                        ));
                    }
                    index += 1;
                }
            }
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "arviz",
                version: PSIS_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model_name: spec.model_name,
            likelihood_target: spec.likelihood_target,
            data_identity_sha256: spec.data_identity_sha256,
            preprocessing_identity_sha256: spec.preprocessing_identity_sha256,
            heldout_unit: spec.heldout_unit,
            unit_ids,
            chains,
            draws_per_chain: draws_per_chain as u32,
            relative_efficiency: spec.relative_efficiency,
            log_likelihood: spec
                .draws
                .into_iter()
                .map(|record| record.log_likelihood)
                .collect(),
            resources: PsisLooResourceLimits {
                maximum_log_likelihood_values: MAXIMUM_LOG_LIKELIHOOD_VALUES as u32,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PsisLooPointwise {
    pub unit_id: String,
    pub elpd_loo: f64,
    pub pareto_k: f64,
    pub reliability: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PsisLooWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub elpd_loo: f64,
    pub standard_error: f64,
    pub p_loo: f64,
    pub good_pareto_k: f64,
    pub maximum_pareto_k: f64,
    pub warning: bool,
    pub pointwise: Vec<PsisLooPointwise>,
    pub refit_or_kfold_units: Vec<String>,
}

impl PsisLooWorkerResult {
    pub fn validate(
        &self,
        request: &PsisLooWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.arviz_psis_loo_worker_result"
            || self.version != 1
            || self.backend.name != "arviz"
            || self.backend.version != PSIS_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.pointwise.len() != request.unit_ids.len()
            || !finite(&[
                self.elpd_loo,
                self.standard_error,
                self.p_loo,
                self.good_pareto_k,
                self.maximum_pareto_k,
            ])
            || self.standard_error < 0.0
            || self.good_pareto_k <= 0.0
            || self.good_pareto_k > 1.0
        {
            return Err(BayesError::WorkerContract(
                "PSIS-LOO result identity or aggregate diagnostics are invalid".into(),
            ));
        }
        let mut pointwise_sum = 0.0;
        let mut maximum_pareto_k = f64::NEG_INFINITY;
        let mut refit_units = Vec::new();
        for (actual, expected_unit) in self.pointwise.iter().zip(&request.unit_ids) {
            if actual.unit_id != *expected_unit
                || !finite(&[actual.elpd_loo, actual.pareto_k])
                || actual.reliability
                    != if actual.pareto_k <= self.good_pareto_k {
                        "reliable"
                    } else {
                        "requires_refit_or_kfold"
                    }
            {
                return Err(BayesError::WorkerContract(
                    "PSIS-LOO pointwise diagnostic is invalid".into(),
                ));
            }
            pointwise_sum += actual.elpd_loo;
            maximum_pareto_k = maximum_pareto_k.max(actual.pareto_k);
            if actual.pareto_k > self.good_pareto_k {
                refit_units.push(actual.unit_id.clone());
            }
        }
        if !approximately_equal(pointwise_sum, self.elpd_loo)
            || !approximately_equal(maximum_pareto_k, self.maximum_pareto_k)
            || self.warning == refit_units.is_empty()
            || self.refit_or_kfold_units != refit_units
        {
            return Err(BayesError::WorkerContract(
                "PSIS-LOO aggregate and pointwise diagnostics disagree".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: PsisLooWorkerRequest,
        input: PsisLooInputIdentity,
    ) -> PsisLooResult {
        PsisLooResult {
            format: "marklab.bayesian_psis_loo".into(),
            version: 1,
            backend: self.backend,
            input,
            model_name: request.model_name,
            likelihood_target: request.likelihood_target,
            data_identity_sha256: request.data_identity_sha256,
            preprocessing_identity_sha256: request.preprocessing_identity_sha256,
            heldout_unit: request.heldout_unit,
            chains: request.chains,
            draws_per_chain: request.draws_per_chain,
            sample_count: u64::from(request.chains) * u64::from(request.draws_per_chain),
            relative_efficiency: request.relative_efficiency,
            elpd_loo: self.elpd_loo,
            standard_error: self.standard_error,
            p_loo: self.p_loo,
            good_pareto_k: self.good_pareto_k,
            maximum_pareto_k: self.maximum_pareto_k,
            warning: self.warning,
            reliability: if self.warning {
                "requires_refit_or_kfold".into()
            } else {
                "reliable".into()
            },
            pointwise: self.pointwise,
            refit_or_kfold_units: self.refit_or_kfold_units,
            claim_status: "experimental_predictive_diagnostic".into(),
            request_sha256: self.request_sha256,
        }
    }
}

impl PsisLooResult {
    pub(crate) fn validate_published(&self) -> Result<(), BayesError> {
        let expected_value_count = self
            .sample_count
            .checked_mul(self.pointwise.len() as u64)
            .ok_or_else(|| {
                BayesError::WorkerContract("published PSIS-LOO matrix dimensions overflow".into())
            })?;
        if self.format != "marklab.bayesian_psis_loo"
            || self.version != 1
            || self.backend.name != "arviz"
            || self.backend.version != PSIS_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || !sha256(&self.backend.environment_lock_sha256)
            || !sha256(&self.backend.worker_sha256)
            || !sha256(&self.input.log_likelihood_sha256)
            || !sha256(&self.request_sha256)
            || !sha256(&self.data_identity_sha256)
            || !sha256(&self.preprocessing_identity_sha256)
            || !exact_name(&self.model_name)
            || !exact_name(&self.likelihood_target)
            || !exact_name(&self.heldout_unit)
            || !(2..=8).contains(&self.chains)
            || !(100..=100_000).contains(&self.draws_per_chain)
            || self.sample_count != u64::from(self.chains) * u64::from(self.draws_per_chain)
            || expected_value_count > MAXIMUM_LOG_LIKELIHOOD_VALUES as u64
            || self.input.log_likelihood_value_count as u64 != expected_value_count
            || !finite(&[
                self.relative_efficiency,
                self.elpd_loo,
                self.standard_error,
                self.p_loo,
                self.good_pareto_k,
                self.maximum_pareto_k,
            ])
            || !(0.0..=1.0).contains(&self.relative_efficiency)
            || self.relative_efficiency == 0.0
            || self.standard_error < 0.0
            || self.good_pareto_k <= 0.0
            || self.good_pareto_k > 1.0
            || !(2..=10_000).contains(&self.pointwise.len())
            || self.claim_status != "experimental_predictive_diagnostic"
        {
            return Err(BayesError::WorkerContract(
                "published PSIS-LOO identity or aggregate is invalid".into(),
            ));
        }
        let mut pointwise_sum = 0.0;
        let mut maximum_pareto_k = f64::NEG_INFINITY;
        let mut prior_unit = "";
        let mut refit_units = Vec::new();
        for point in &self.pointwise {
            if !exact_name(&point.unit_id)
                || point.unit_id.as_str() <= prior_unit
                || !finite(&[point.elpd_loo, point.pareto_k])
                || point.reliability
                    != if point.pareto_k <= self.good_pareto_k {
                        "reliable"
                    } else {
                        "requires_refit_or_kfold"
                    }
            {
                return Err(BayesError::WorkerContract(
                    "published PSIS-LOO pointwise value is invalid".into(),
                ));
            }
            prior_unit = &point.unit_id;
            pointwise_sum += point.elpd_loo;
            maximum_pareto_k = maximum_pareto_k.max(point.pareto_k);
            if point.pareto_k > self.good_pareto_k {
                refit_units.push(point.unit_id.clone());
            }
        }
        let expected_reliability = if refit_units.is_empty() {
            "reliable"
        } else {
            "requires_refit_or_kfold"
        };
        if !approximately_equal(pointwise_sum, self.elpd_loo)
            || !approximately_equal(maximum_pareto_k, self.maximum_pareto_k)
            || self.warning == refit_units.is_empty()
            || self.reliability != expected_reliability
            || self.refit_or_kfold_units != refit_units
        {
            return Err(BayesError::WorkerContract(
                "published PSIS-LOO pointwise and aggregate diagnostics disagree".into(),
            ));
        }
        Ok(())
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * left.abs().max(right.abs()).max(1.0)
}

fn exact_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.trim() == value
}

pub(crate) fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PsisLooInputIdentity {
    pub path: String,
    pub log_likelihood_value_count: usize,
    pub log_likelihood_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PsisLooResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub input: PsisLooInputIdentity,
    pub model_name: String,
    pub likelihood_target: String,
    pub data_identity_sha256: String,
    pub preprocessing_identity_sha256: String,
    pub heldout_unit: String,
    pub chains: u32,
    pub draws_per_chain: u32,
    pub sample_count: u64,
    pub relative_efficiency: f64,
    pub elpd_loo: f64,
    pub standard_error: f64,
    pub p_loo: f64,
    pub good_pareto_k: f64,
    pub maximum_pareto_k: f64,
    pub warning: bool,
    pub reliability: String,
    pub pointwise: Vec<PsisLooPointwise>,
    pub refit_or_kfold_units: Vec<String>,
    pub claim_status: String,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_incomplete_chain_draw_unit_matrix() {
        let mut draws = Vec::new();
        for chain in 0..2 {
            for draw in 0..100 {
                for unit_id in ["a", "b"] {
                    draws.push(PointwiseLogLikelihoodDraw {
                        chain,
                        draw,
                        unit_id: unit_id.into(),
                        log_likelihood: -1.0,
                    });
                }
            }
        }
        draws.pop();
        assert!(matches!(
            PsisLooWorkerRequest::new(
                PsisLooSpec {
                    model_name: "model".into(),
                    likelihood_target: "observation".into(),
                    data_identity_sha256: "1".repeat(64),
                    preprocessing_identity_sha256: "2".repeat(64),
                    heldout_unit: "patient".into(),
                    relative_efficiency: 1.0,
                    draws,
                },
                "lock".into(),
                "worker".into(),
                60,
            ),
            Err(BayesError::InvalidSpec(_))
        ));
    }
}
