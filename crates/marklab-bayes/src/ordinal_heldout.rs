use crate::{BayesError, OrdinalGroupSitePatientData};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
pub struct OrdinalSiteHeldoutSpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub ordered_levels: Vec<String>,
    pub smoothing: f64,
    pub optimizer_tolerance: f64,
    pub maximum_optimizer_evaluations: u64,
    pub patients: Vec<OrdinalGroupSitePatientData>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct OrdinalSiteFoldScore {
    pub site_id: String,
    pub heldout_patients: usize,
    pub proportional_log_score: f64,
    pub nonproportional_log_score: f64,
    pub nonproportional_minus_proportional: f64,
    pub optimizer_evaluations: u64,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct OrdinalSiteHeldoutComparison {
    pub format: String,
    pub version: u32,
    pub statistical_unit: String,
    pub heldout_unit: String,
    pub null: String,
    pub assumptions: Vec<String>,
    pub reference_group: String,
    pub comparison_group: String,
    pub ordered_levels: Vec<String>,
    pub patient_count: usize,
    pub site_count: usize,
    pub smoothing: f64,
    pub optimizer_tolerance: f64,
    pub maximum_optimizer_evaluations: u64,
    pub optimizer_evaluations: u64,
    pub folds: Vec<OrdinalSiteFoldScore>,
    pub proportional_mean_log_score: f64,
    pub nonproportional_mean_log_score: f64,
    pub nonproportional_minus_proportional_mean_log_score: f64,
    pub interpretation: String,
}

impl OrdinalSiteHeldoutComparison {
    pub fn validates(&self, spec: &OrdinalSiteHeldoutSpec) -> bool {
        let sites = spec
            .patients
            .iter()
            .map(|patient| patient.site_id.as_str())
            .collect::<BTreeSet<_>>();
        self.format == "marklab.ordinal_site_heldout_comparison"
            && self.version == 1
            && self.statistical_unit == "patient"
            && self.heldout_unit == "entire_site"
            && self.reference_group == spec.reference_group
            && self.comparison_group == spec.comparison_group
            && self.ordered_levels == spec.ordered_levels
            && self.patient_count == spec.patients.len()
            && self.site_count == sites.len()
            && self.smoothing.to_bits() == spec.smoothing.to_bits()
            && self.optimizer_tolerance.to_bits() == spec.optimizer_tolerance.to_bits()
            && self.maximum_optimizer_evaluations == spec.maximum_optimizer_evaluations
            && self.optimizer_evaluations <= self.maximum_optimizer_evaluations
            && self.folds.len() == sites.len()
            && self.folds.iter().all(|fold| {
                sites.contains(fold.site_id.as_str())
                    && fold.heldout_patients > 0
                    && fold.proportional_log_score.is_finite()
                    && fold.nonproportional_log_score.is_finite()
                    && (fold.nonproportional_log_score
                        - fold.proportional_log_score
                        - fold.nonproportional_minus_proportional)
                        .abs()
                        < 1e-10
            })
            && [
                self.proportional_mean_log_score,
                self.nonproportional_mean_log_score,
                self.nonproportional_minus_proportional_mean_log_score,
            ]
            .iter()
            .all(|value| value.is_finite())
    }
}

pub fn ordinal_site_heldout_comparison(
    mut spec: OrdinalSiteHeldoutSpec,
) -> Result<OrdinalSiteHeldoutComparison, BayesError> {
    if spec.reference_group.is_empty()
        || spec.comparison_group.is_empty()
        || spec.reference_group == spec.comparison_group
        || !(2..=8).contains(&spec.ordered_levels.len())
        || !spec.smoothing.is_finite()
        || spec.smoothing <= 0.0
        || !spec.optimizer_tolerance.is_finite()
        || !(1e-10..=1e-2).contains(&spec.optimizer_tolerance)
        || !(100..=200_000).contains(&spec.maximum_optimizer_evaluations)
        || !(16..=1024).contains(&spec.patients.len())
    {
        return Err(BayesError::InvalidSpec(
            "ordinal heldout controls are invalid".into(),
        ));
    }
    if spec.ordered_levels.iter().any(|x| x.is_empty())
        || spec.ordered_levels.iter().collect::<BTreeSet<_>>().len() != spec.ordered_levels.len()
    {
        return Err(BayesError::InvalidSpec(
            "ordinal heldout levels are invalid".into(),
        ));
    }
    spec.patients
        .sort_by(|a, b| a.patient_id.cmp(&b.patient_id));
    let mut ids = BTreeSet::new();
    let mut sites = BTreeMap::<String, [usize; 2]>::new();
    let mut level_counts = vec![0usize; spec.ordered_levels.len()];
    for p in &spec.patients {
        if p.patient_id.is_empty()
            || p.site_id.is_empty()
            || !ids.insert(p.patient_id.as_str())
            || (p.group != spec.reference_group && p.group != spec.comparison_group)
            || p.outcome_code as usize >= spec.ordered_levels.len()
        {
            return Err(BayesError::InvalidSpec(
                "ordinal heldout patient rows are invalid".into(),
            ));
        }
        sites.entry(p.site_id.clone()).or_default()
            [usize::from(p.group == spec.comparison_group)] += 1;
        level_counts[p.outcome_code as usize] += 1;
    }
    if !(4..=64).contains(&sites.len())
        || sites.values().any(|x| x[0] < 2 || x[1] < 2)
        || level_counts.contains(&0)
    {
        return Err(BayesError::InvalidSpec(
            "ordinal heldout requires four sites, both groups per site, and every level".into(),
        ));
    }
    if spec
        .maximum_optimizer_evaluations
        .checked_mul(spec.patients.len() as u64)
        .is_none_or(|x| x > 200_000_000)
    {
        return Err(BayesError::InvalidSpec(
            "ordinal heldout optimizer work exceeds 200000000 row evaluations".into(),
        ));
    }
    let mut folds = Vec::new();
    let mut total_prop = 0.0;
    let mut total_nonprop = 0.0;
    let mut total_n = 0usize;
    let mut total_evals = 0u64;
    for site_id in sites.keys() {
        let train = spec
            .patients
            .iter()
            .filter(|p| p.site_id != *site_id)
            .collect::<Vec<_>>();
        let test = spec
            .patients
            .iter()
            .filter(|p| p.site_id == *site_id)
            .collect::<Vec<_>>();
        let remaining = spec.maximum_optimizer_evaluations - total_evals;
        let fit = fit_proportional(
            &train,
            &spec.comparison_group,
            spec.ordered_levels.len(),
            spec.smoothing,
            spec.optimizer_tolerance,
            remaining,
        )?;
        total_evals += fit.evaluations;
        let nonprop = fit_group_probabilities(
            &train,
            &spec.reference_group,
            &spec.comparison_group,
            spec.ordered_levels.len(),
            spec.smoothing,
        );
        let mut prop_score = 0.0;
        let mut nonprop_score = 0.0;
        for p in &test {
            let x = if p.group == spec.comparison_group {
                1.0
            } else {
                0.0
            };
            let probabilities = ordinal_probabilities(&fit.cutpoints, fit.beta * x);
            prop_score += probabilities[p.outcome_code as usize].ln();
            let group = usize::from(p.group == spec.comparison_group);
            nonprop_score += nonprop[group][p.outcome_code as usize].ln();
        }
        let delta = nonprop_score - prop_score;
        folds.push(OrdinalSiteFoldScore {
            site_id: site_id.clone(),
            heldout_patients: test.len(),
            proportional_log_score: prop_score,
            nonproportional_log_score: nonprop_score,
            nonproportional_minus_proportional: delta,
            optimizer_evaluations: fit.evaluations,
        });
        total_prop += prop_score;
        total_nonprop += nonprop_score;
        total_n += test.len();
    }
    let prop = total_prop / total_n as f64;
    let nonprop = total_nonprop / total_n as f64;
    let delta = nonprop - prop;
    Ok(OrdinalSiteHeldoutComparison {
        format: "marklab.ordinal_site_heldout_comparison".into(),
        version: 1,
        statistical_unit: "patient".into(),
        heldout_unit: "entire_site".into(),
        null: "nonproportional ordinal probabilities do not improve heldout patient log score"
            .into(),
        assumptions: vec![
            "sites are independent transport units".into(),
            "patient outcomes are conditionally independent within training likelihood".into(),
            "fixed additive smoothing is applied inside each training fold".into(),
        ],
        reference_group: spec.reference_group,
        comparison_group: spec.comparison_group,
        ordered_levels: spec.ordered_levels,
        patient_count: spec.patients.len(),
        site_count: sites.len(),
        smoothing: spec.smoothing,
        optimizer_tolerance: spec.optimizer_tolerance,
        maximum_optimizer_evaluations: spec.maximum_optimizer_evaluations,
        optimizer_evaluations: total_evals,
        folds,
        proportional_mean_log_score: prop,
        nonproportional_mean_log_score: nonprop,
        nonproportional_minus_proportional_mean_log_score: delta,
        interpretation: if delta > 0.0 {
            "nonproportional_improves_site_heldout_log_score".into()
        } else {
            "no_nonproportional_site_heldout_improvement".into()
        },
    })
}

struct PropFit {
    cutpoints: Vec<f64>,
    beta: f64,
    evaluations: u64,
}
fn fit_proportional(
    rows: &[&OrdinalGroupSitePatientData],
    comparison: &str,
    levels: usize,
    smoothing: f64,
    tolerance: f64,
    max_eval: u64,
) -> Result<PropFit, BayesError> {
    let mut counts = vec![0.0; levels];
    for p in rows {
        counts[p.outcome_code as usize] += 1.0;
    }
    let total = rows.len() as f64;
    let mut cumulative = 0.0;
    let mut cutpoints = Vec::new();
    for count in counts.iter().take(levels - 1) {
        cumulative += count;
        let probability =
            ((cumulative + smoothing) / (total + 2.0 * smoothing)).clamp(1e-9, 1.0 - 1e-9);
        cutpoints.push((probability / (1.0 - probability)).ln());
    }
    let mut beta = 0.0;
    let mut evaluations = 1;
    let mut best = negative_log_likelihood(rows, comparison, &cutpoints, beta);
    let mut step = 1.0;
    while step >= tolerance {
        let mut improved = false;
        for dimension in 0..=cutpoints.len() {
            for direction in [-1.0, 1.0] {
                if evaluations >= max_eval {
                    return Err(BayesError::InvalidSpec(
                        "ordinal heldout optimizer evaluation limit exhausted".into(),
                    ));
                }
                let mut candidate = cutpoints.clone();
                let mut candidate_beta = beta;
                if dimension < cutpoints.len() {
                    candidate[dimension] += direction * step;
                    if (dimension > 0 && candidate[dimension] <= candidate[dimension - 1] + 1e-9)
                        || (dimension + 1 < candidate.len()
                            && candidate[dimension] >= candidate[dimension + 1] - 1e-9)
                    {
                        continue;
                    }
                } else {
                    candidate_beta += direction * step;
                }
                let objective =
                    negative_log_likelihood(rows, comparison, &candidate, candidate_beta);
                evaluations += 1;
                if objective + 1e-12 < best {
                    best = objective;
                    cutpoints = candidate;
                    beta = candidate_beta;
                    improved = true;
                }
            }
        }
        if !improved {
            step *= 0.5;
        }
    }
    Ok(PropFit {
        cutpoints,
        beta,
        evaluations,
    })
}
fn negative_log_likelihood(
    rows: &[&OrdinalGroupSitePatientData],
    comparison: &str,
    cutpoints: &[f64],
    beta: f64,
) -> f64 {
    rows.iter()
        .map(|p| {
            let probability =
                ordinal_probabilities(cutpoints, if p.group == comparison { beta } else { 0.0 })
                    [p.outcome_code as usize]
                    .max(1e-300);
            -probability.ln()
        })
        .sum()
}
fn ordinal_probabilities(cutpoints: &[f64], eta: f64) -> Vec<f64> {
    let cumulative = cutpoints
        .iter()
        .map(|c| 1.0 / (1.0 + (eta - c).exp()))
        .collect::<Vec<_>>();
    let mut result = Vec::with_capacity(cutpoints.len() + 1);
    let mut previous = 0.0;
    for value in cumulative {
        result.push((value - previous).max(1e-300));
        previous = value;
    }
    result.push((1.0 - previous).max(1e-300));
    result
}
fn fit_group_probabilities(
    rows: &[&OrdinalGroupSitePatientData],
    reference: &str,
    comparison: &str,
    levels: usize,
    smoothing: f64,
) -> [Vec<f64>; 2] {
    let mut counts = [vec![smoothing; levels], vec![smoothing; levels]];
    for p in rows {
        let group = if p.group == reference {
            0
        } else {
            debug_assert_eq!(p.group, comparison);
            1
        };
        counts[group][p.outcome_code as usize] += 1.0;
    }
    counts.map(|values| {
        let total = values.iter().sum::<f64>();
        values.into_iter().map(|x| x / total).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinal_probabilities_are_positive_and_sum_to_one() {
        let p = ordinal_probabilities(&[-1.0, 0.5, 2.0], 0.7);
        assert!(p.iter().all(|x| *x > 0.0));
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
