use serde::Serialize;

use crate::{validation::is_lower_hex_sha256 as sha256, BayesError, PsisLooResult};

pub struct PsisLooComparisonInput {
    pub path: String,
    pub artifact_sha256: String,
    pub result: PsisLooResult,
}

#[derive(Debug, Serialize)]
pub struct ComparedModel {
    pub model_name: String,
    pub rank: u32,
    pub source_path: String,
    pub artifact_sha256: String,
    pub request_sha256: String,
    pub elpd_loo: f64,
    pub standard_error: f64,
    pub p_loo: f64,
    pub maximum_pareto_k: f64,
    pub reliability: String,
}

#[derive(Debug, Serialize)]
pub struct PairwiseElpdDifference {
    pub model_a: String,
    pub model_b: String,
    pub elpd_difference: f64,
    pub standard_error: f64,
}

#[derive(Debug, Serialize)]
pub struct BayesianModelComparisonResult {
    pub format: &'static str,
    pub version: u32,
    pub heldout_unit: String,
    pub likelihood_target: String,
    pub data_identity_sha256: String,
    pub preprocessing_identity_sha256: String,
    pub unit_count: usize,
    pub reliability: &'static str,
    pub best_predictive_model: String,
    pub models: Vec<ComparedModel>,
    pub pairwise: Vec<PairwiseElpdDifference>,
    pub models_requiring_refit_or_kfold: Vec<String>,
    pub claim_status: &'static str,
}

pub fn compare_psis_loo_models(
    mut inputs: Vec<PsisLooComparisonInput>,
) -> Result<BayesianModelComparisonResult, BayesError> {
    if !(2..=16).contains(&inputs.len()) {
        return Err(BayesError::InvalidSpec(
            "Bayesian comparison requires 2-16 PSIS-LOO artifacts".into(),
        ));
    }
    for input in &inputs {
        if input.path.is_empty()
            || !sha256(&input.artifact_sha256)
            || input.result.validate_published().is_err()
        {
            return Err(BayesError::InvalidSpec(
                "Bayesian comparison input artifact is invalid".into(),
            ));
        }
    }
    inputs.sort_by(|left, right| left.result.model_name.cmp(&right.result.model_name));
    if inputs
        .windows(2)
        .any(|pair| pair[0].result.model_name == pair[1].result.model_name)
    {
        return Err(BayesError::InvalidSpec(
            "Bayesian comparison model names must be unique".into(),
        ));
    }
    let baseline = &inputs[0].result;
    let baseline_units = baseline
        .pointwise
        .iter()
        .map(|point| point.unit_id.as_str())
        .collect::<Vec<_>>();
    for input in inputs.iter().skip(1) {
        let result = &input.result;
        let units = result
            .pointwise
            .iter()
            .map(|point| point.unit_id.as_str())
            .collect::<Vec<_>>();
        if result.heldout_unit != baseline.heldout_unit
            || result.likelihood_target != baseline.likelihood_target
            || result.data_identity_sha256 != baseline.data_identity_sha256
            || result.preprocessing_identity_sha256 != baseline.preprocessing_identity_sha256
            || units != baseline_units
        {
            return Err(BayesError::InvalidSpec(
                "Bayesian comparison requires exact held-out units, target, data, and preprocessing compatibility"
                    .into(),
            ));
        }
    }

    let mut pairwise = Vec::new();
    for left in 0..inputs.len() {
        for right in left + 1..inputs.len() {
            let differences = inputs[left]
                .result
                .pointwise
                .iter()
                .zip(&inputs[right].result.pointwise)
                .map(|(a, b)| a.elpd_loo - b.elpd_loo)
                .collect::<Vec<_>>();
            let difference = differences.iter().sum::<f64>();
            let mean = difference / differences.len() as f64;
            let variance = differences
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / (differences.len() - 1) as f64;
            pairwise.push(PairwiseElpdDifference {
                model_a: inputs[left].result.model_name.clone(),
                model_b: inputs[right].result.model_name.clone(),
                elpd_difference: difference,
                standard_error: (differences.len() as f64 * variance).sqrt(),
            });
        }
    }

    let mut ranked = (0..inputs.len()).collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        inputs[*right]
            .result
            .elpd_loo
            .total_cmp(&inputs[*left].result.elpd_loo)
            .then_with(|| {
                inputs[*left]
                    .result
                    .model_name
                    .cmp(&inputs[*right].result.model_name)
            })
    });
    let models = ranked
        .iter()
        .enumerate()
        .map(|(rank, index)| {
            let input = &inputs[*index];
            ComparedModel {
                model_name: input.result.model_name.clone(),
                rank: rank as u32 + 1,
                source_path: input.path.clone(),
                artifact_sha256: input.artifact_sha256.clone(),
                request_sha256: input.result.request_sha256.clone(),
                elpd_loo: input.result.elpd_loo,
                standard_error: input.result.standard_error,
                p_loo: input.result.p_loo,
                maximum_pareto_k: input.result.maximum_pareto_k,
                reliability: input.result.reliability.clone(),
            }
        })
        .collect::<Vec<_>>();
    let models_requiring_refit_or_kfold = inputs
        .iter()
        .filter(|input| input.result.warning)
        .map(|input| input.result.model_name.clone())
        .collect::<Vec<_>>();
    Ok(BayesianModelComparisonResult {
        format: "marklab.bayesian_model_comparison",
        version: 1,
        heldout_unit: baseline.heldout_unit.clone(),
        likelihood_target: baseline.likelihood_target.clone(),
        data_identity_sha256: baseline.data_identity_sha256.clone(),
        preprocessing_identity_sha256: baseline.preprocessing_identity_sha256.clone(),
        unit_count: baseline.pointwise.len(),
        reliability: if models_requiring_refit_or_kfold.is_empty() {
            "reliable"
        } else {
            "requires_refit_or_kfold"
        },
        best_predictive_model: models[0].model_name.clone(),
        models,
        pairwise,
        models_requiring_refit_or_kfold,
        claim_status: "experimental_predictive_comparison",
    })
}
