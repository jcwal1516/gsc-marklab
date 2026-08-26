use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::CausalError;

const MAXIMUM_SCENARIOS: usize = 1_000_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiasSensitivityScenario {
    pub scenario_id: String,
    pub prevalence_treated: f64,
    pub prevalence_control: f64,
    pub outcome_effect: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiasSensitivitySpec {
    pub observed_effect: f64,
    pub scenarios: Vec<BiasSensitivityScenario>,
    pub maximum_scenarios: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BiasSensitivityScenarioResult {
    pub scenario_id: String,
    pub prevalence_treated: f64,
    pub prevalence_control: f64,
    pub outcome_effect: f64,
    pub prevalence_difference: f64,
    pub bias: f64,
    pub adjusted_effect: f64,
    pub sign_reversal: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BiasSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub sensitivity_model: &'static str,
    pub observed_effect: f64,
    pub scenarios: Vec<BiasSensitivityScenarioResult>,
    pub region_lower: f64,
    pub region_upper: f64,
    pub region_includes_zero: bool,
    pub scenario_evaluations: usize,
    pub claim_status: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundedOutcomeObservation {
    pub unit_id: String,
    pub treatment: bool,
    pub outcome: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManskiBoundedOutcomeSpec {
    pub observations: Vec<BoundedOutcomeObservation>,
    pub outcome_lower: f64,
    pub outcome_upper: f64,
    pub maximum_observations: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManskiBoundedOutcomeResult {
    pub format: &'static str,
    pub version: u32,
    pub assumptions: Vec<&'static str>,
    pub observations: usize,
    pub treated_observations: usize,
    pub control_observations: usize,
    pub treatment_fraction: f64,
    pub treated_observed_mean: f64,
    pub control_observed_mean: f64,
    pub observed_difference: f64,
    pub outcome_lower: f64,
    pub outcome_upper: f64,
    pub treated_potential_mean_lower: f64,
    pub treated_potential_mean_upper: f64,
    pub control_potential_mean_lower: f64,
    pub control_potential_mean_upper: f64,
    pub ate_lower: f64,
    pub ate_upper: f64,
    pub ate_includes_zero: bool,
    pub sampling_uncertainty: &'static str,
    pub claim_status: &'static str,
}

pub fn binary_confounder_bias_sensitivity(
    mut spec: BiasSensitivitySpec,
) -> Result<BiasSensitivityResult, CausalError> {
    validate(&spec)?;
    spec.scenarios
        .sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    let scenarios = spec
        .scenarios
        .into_iter()
        .map(|scenario| {
            let prevalence_difference = scenario.prevalence_treated - scenario.prevalence_control;
            let bias = prevalence_difference * scenario.outcome_effect;
            let adjusted_effect = spec.observed_effect - bias;
            BiasSensitivityScenarioResult {
                scenario_id: scenario.scenario_id,
                prevalence_treated: scenario.prevalence_treated,
                prevalence_control: scenario.prevalence_control,
                outcome_effect: scenario.outcome_effect,
                prevalence_difference,
                bias,
                adjusted_effect,
                sign_reversal: spec.observed_effect != 0.0
                    && adjusted_effect != 0.0
                    && spec.observed_effect.is_sign_positive()
                        != adjusted_effect.is_sign_positive(),
            }
        })
        .collect::<Vec<_>>();
    let region_lower = scenarios
        .iter()
        .map(|scenario| scenario.adjusted_effect)
        .fold(f64::INFINITY, f64::min);
    let region_upper = scenarios
        .iter()
        .map(|scenario| scenario.adjusted_effect)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(BiasSensitivityResult {
        format: "marklab.binary_confounder_bias_sensitivity",
        version: 1,
        sensitivity_model: "prevalence_difference_times_additive_outcome_effect",
        observed_effect: spec.observed_effect,
        scenario_evaluations: scenarios.len(),
        region_lower,
        region_upper,
        region_includes_zero: region_lower <= 0.0 && region_upper >= 0.0,
        scenarios,
        claim_status: "supplied_assumption_sensitivity_only",
    })
}

pub fn manski_bounded_outcome_ate(
    spec: ManskiBoundedOutcomeSpec,
) -> Result<ManskiBoundedOutcomeResult, CausalError> {
    if !spec.outcome_lower.is_finite()
        || !spec.outcome_upper.is_finite()
        || spec.outcome_lower >= spec.outcome_upper
        || spec.observations.is_empty()
        || spec.observations.len() > spec.maximum_observations
        || spec.observations.len() > MAXIMUM_SCENARIOS
    {
        return Err(CausalError::Invalid(
            "outcome support and observation count violate finite ordered bounded requirements"
                .into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut treated_sum = 0.0;
    let mut control_sum = 0.0;
    let mut treated_observations = 0;
    for observation in &spec.observations {
        if observation.unit_id.trim().is_empty()
            || !ids.insert(observation.unit_id.as_str())
            || !observation.outcome.is_finite()
            || observation.outcome < spec.outcome_lower
            || observation.outcome > spec.outcome_upper
        {
            return Err(CausalError::Invalid(
                "observation IDs must be unique and outcomes finite within declared support".into(),
            ));
        }
        if observation.treatment {
            treated_sum += observation.outcome;
            treated_observations += 1;
        } else {
            control_sum += observation.outcome;
        }
    }
    let control_observations = spec.observations.len() - treated_observations;
    if treated_observations == 0 || control_observations == 0 {
        return Err(CausalError::Invalid(
            "both treatment groups must be observed".into(),
        ));
    }
    let treated_mean = treated_sum / treated_observations as f64;
    let control_mean = control_sum / control_observations as f64;
    let fraction = treated_observations as f64 / spec.observations.len() as f64;
    let treated_potential_mean_lower =
        fraction * treated_mean + (1.0 - fraction) * spec.outcome_lower;
    let treated_potential_mean_upper =
        fraction * treated_mean + (1.0 - fraction) * spec.outcome_upper;
    let control_potential_mean_lower =
        (1.0 - fraction) * control_mean + fraction * spec.outcome_lower;
    let control_potential_mean_upper =
        (1.0 - fraction) * control_mean + fraction * spec.outcome_upper;
    let ate_lower = treated_potential_mean_lower - control_potential_mean_upper;
    let ate_upper = treated_potential_mean_upper - control_potential_mean_lower;
    Ok(ManskiBoundedOutcomeResult {
        format: "marklab.manski_bounded_outcome_ate",
        version: 1,
        assumptions: vec![
            "consistency_for_observed_potential_outcomes",
            "known_bounded_outcome_support",
            "no_ignorability_or_monotonicity_assumed",
        ],
        observations: spec.observations.len(),
        treated_observations,
        control_observations,
        treatment_fraction: fraction,
        treated_observed_mean: treated_mean,
        control_observed_mean: control_mean,
        observed_difference: treated_mean - control_mean,
        outcome_lower: spec.outcome_lower,
        outcome_upper: spec.outcome_upper,
        treated_potential_mean_lower,
        treated_potential_mean_upper,
        control_potential_mean_lower,
        control_potential_mean_upper,
        ate_lower,
        ate_upper,
        ate_includes_zero: ate_lower <= 0.0 && ate_upper >= 0.0,
        sampling_uncertainty: "not_requested_point_identification_bounds_only",
        claim_status: "bounded_outcome_partial_identification_only",
    })
}

fn validate(spec: &BiasSensitivitySpec) -> Result<(), CausalError> {
    if !spec.observed_effect.is_finite()
        || spec.scenarios.is_empty()
        || spec.scenarios.len() > spec.maximum_scenarios
        || spec.scenarios.len() > MAXIMUM_SCENARIOS
    {
        return Err(CausalError::Invalid(
            "observed effect/scenario count violates finite nonempty bounded requirements".into(),
        ));
    }
    let mut ids = HashSet::new();
    for scenario in &spec.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !ids.insert(scenario.scenario_id.as_str())
            || !scenario.prevalence_treated.is_finite()
            || !(0.0..=1.0).contains(&scenario.prevalence_treated)
            || !scenario.prevalence_control.is_finite()
            || !(0.0..=1.0).contains(&scenario.prevalence_control)
            || !scenario.outcome_effect.is_finite()
        {
            return Err(CausalError::Invalid(
                "scenario IDs must be unique and prevalence/effect values valid".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchedPairObservation {
    pub unit_id: String,
    pub treatment: bool,
    pub outcome: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchedPairSet {
    pub set_id: String,
    pub observations: [MatchedPairObservation; 2],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosenbaumSignSensitivitySpec {
    pub matched_sets: Vec<MatchedPairSet>,
    pub gamma_grid: Vec<f64>,
    pub alpha: f64,
    pub maximum_set_gamma_evaluations: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MatchedPairDifference {
    pub set_id: String,
    pub treated_unit_id: String,
    pub control_unit_id: String,
    pub treated_minus_control: f64,
    pub sign: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RosenbaumSensitivityPoint {
    pub gamma: f64,
    pub success_probability_lower: f64,
    pub success_probability_upper: f64,
    pub lower_p_value: f64,
    pub upper_p_value: f64,
    pub conclusion_retained: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RosenbaumSignSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub statistic: &'static str,
    pub alternative: &'static str,
    pub matched_sets: usize,
    pub non_tied_pairs: usize,
    pub positive_differences: usize,
    pub negative_differences: usize,
    pub ties: usize,
    pub pair_differences: Vec<MatchedPairDifference>,
    pub alpha: f64,
    pub curve: Vec<RosenbaumSensitivityPoint>,
    pub critical_gamma: Option<f64>,
    pub set_gamma_evaluations: u64,
    pub claim_status: &'static str,
}

pub fn rosenbaum_sign_sensitivity(
    mut spec: RosenbaumSignSensitivitySpec,
) -> Result<RosenbaumSignSensitivityResult, CausalError> {
    validate_rosenbaum(&spec)?;
    spec.matched_sets
        .sort_by(|left, right| left.set_id.cmp(&right.set_id));
    let mut differences = Vec::with_capacity(spec.matched_sets.len());
    let mut positive = 0;
    let mut negative = 0;
    let mut ties = 0;
    for set in &spec.matched_sets {
        let treated = set
            .observations
            .iter()
            .find(|observation| observation.treatment)
            .expect("validated one treated observation");
        let control = set
            .observations
            .iter()
            .find(|observation| !observation.treatment)
            .expect("validated one control observation");
        let difference = treated.outcome - control.outcome;
        let sign = if difference > 0.0 {
            positive += 1;
            "positive"
        } else if difference < 0.0 {
            negative += 1;
            "negative"
        } else {
            ties += 1;
            "tie_excluded"
        };
        differences.push(MatchedPairDifference {
            set_id: set.set_id.clone(),
            treated_unit_id: treated.unit_id.clone(),
            control_unit_id: control.unit_id.clone(),
            treated_minus_control: difference,
            sign,
        });
    }
    let non_tied = positive + negative;
    if non_tied == 0 {
        return Err(CausalError::Invalid(
            "Rosenbaum sign sensitivity requires at least one non-tied pair".into(),
        ));
    }
    let work = u64::try_from(spec.matched_sets.len())
        .ok()
        .and_then(|sets| {
            u64::try_from(spec.gamma_grid.len())
                .ok()
                .and_then(|grid| sets.checked_mul(grid))
        })
        .ok_or_else(|| CausalError::Resource("set-gamma work overflowed".into()))?;
    if work > spec.maximum_set_gamma_evaluations || work > 250_000_000 {
        return Err(CausalError::Resource(format!(
            "set-gamma evaluations {work} exceed caller or built-in maximum"
        )));
    }
    let mut critical_gamma = None;
    let curve = spec
        .gamma_grid
        .iter()
        .map(|&gamma| {
            let lower_probability = 1.0 / (1.0 + gamma);
            let upper_probability = gamma / (1.0 + gamma);
            let lower_p_value = binomial_upper_tail(non_tied, positive, lower_probability);
            let upper_p_value = binomial_upper_tail(non_tied, positive, upper_probability);
            let conclusion_retained = upper_p_value <= spec.alpha;
            if conclusion_retained {
                critical_gamma = Some(gamma);
            }
            RosenbaumSensitivityPoint {
                gamma,
                success_probability_lower: lower_probability,
                success_probability_upper: upper_probability,
                lower_p_value,
                upper_p_value,
                conclusion_retained,
            }
        })
        .collect();
    Ok(RosenbaumSignSensitivityResult {
        format: "marklab.rosenbaum_matched_pair_sign_sensitivity",
        version: 1,
        statistic: "positive_treated_minus_control_sign_count",
        alternative: "greater_one_sided",
        matched_sets: spec.matched_sets.len(),
        non_tied_pairs: non_tied,
        positive_differences: positive,
        negative_differences: negative,
        ties,
        pair_differences: differences,
        alpha: spec.alpha,
        curve,
        critical_gamma,
        set_gamma_evaluations: work,
        claim_status: "matched_pair_hidden_bias_sensitivity_only",
    })
}

fn validate_rosenbaum(spec: &RosenbaumSignSensitivitySpec) -> Result<(), CausalError> {
    if spec.matched_sets.is_empty()
        || spec.gamma_grid.is_empty()
        || !spec.alpha.is_finite()
        || !(0.0..1.0).contains(&spec.alpha)
        || spec
            .gamma_grid
            .iter()
            .any(|gamma| !gamma.is_finite() || *gamma < 1.0)
        || spec.gamma_grid.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(CausalError::Invalid(
            "matched sets, increasing gamma>=1 grid, and alpha in (0,1) are required".into(),
        ));
    }
    let mut set_ids = HashSet::new();
    let mut unit_ids = HashSet::new();
    for set in &spec.matched_sets {
        if set.set_id.trim().is_empty()
            || !set_ids.insert(set.set_id.as_str())
            || set.observations[0].treatment == set.observations[1].treatment
        {
            return Err(CausalError::Invalid(
                "matched sets must be uniquely named with exactly one treated unit".into(),
            ));
        }
        for observation in &set.observations {
            if observation.unit_id.trim().is_empty()
                || !unit_ids.insert(observation.unit_id.as_str())
                || !observation.outcome.is_finite()
            {
                return Err(CausalError::Invalid(
                    "matched unit IDs must be globally unique and outcomes finite".into(),
                ));
            }
        }
    }
    Ok(())
}

fn binomial_upper_tail(trials: usize, successes: usize, probability: f64) -> f64 {
    if successes == 0 {
        return 1.0;
    }
    let log_odds = probability.ln() - (1.0 - probability).ln();
    let mut log_probability = trials as f64 * (1.0 - probability).ln();
    let mut tail_logs = Vec::with_capacity(trials - successes + 1);
    for count in 0..=trials {
        if count >= successes {
            tail_logs.push(log_probability);
        }
        if count < trials {
            log_probability +=
                ((trials - count) as f64).ln() - ((count + 1) as f64).ln() + log_odds;
        }
    }
    let maximum = tail_logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (maximum
        + tail_logs
            .iter()
            .map(|value| (value - maximum).exp())
            .sum::<f64>()
            .ln())
    .exp()
    .min(1.0)
}
