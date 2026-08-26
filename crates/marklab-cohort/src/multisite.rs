use std::collections::HashSet;

use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};

use super::CohortInferenceError;

#[derive(Clone, Debug)]
pub struct SiteEffect {
    pub site_id: String,
    pub effect: f64,
    pub standard_error: f64,
    pub patient_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultisiteEffectModel {
    FixedEffect,
    RandomEffectsReml,
}

#[derive(Clone, Debug)]
pub struct MultisiteInferenceSpec {
    pub model: MultisiteEffectModel,
    pub alpha: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SiteSensitivityResult {
    pub omitted_site_id: String,
    pub pooled_effect: f64,
    pub pooled_standard_error: f64,
    pub tau_squared: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultisiteInferenceResult {
    pub model: MultisiteEffectModel,
    pub site_count: usize,
    pub total_patient_count: usize,
    pub pooled_effect: f64,
    pub pooled_standard_error: f64,
    pub confidence_interval: [f64; 2],
    pub prediction_interval: Option<[f64; 2]>,
    pub tau_squared: f64,
    pub heterogeneity_q: f64,
    pub heterogeneity_degrees_of_freedom: usize,
    pub heterogeneity_p_value: f64,
    pub leave_one_site_out: Vec<SiteSensitivityResult>,
    pub alpha: f64,
}

pub fn multisite_spatial_inference(
    sites: &[SiteEffect],
    spec: &MultisiteInferenceSpec,
) -> Result<MultisiteInferenceResult, CohortInferenceError> {
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 0.5) {
        return Err(CohortInferenceError::InvalidInput(
            "multisite alpha must be in (0, 0.5)".into(),
        ));
    }
    let sites = validate_sites(sites)?;
    let fixed = pool(&sites, 0.0)?;
    let q = sites
        .iter()
        .map(|site| (site.effect - fixed.effect).powi(2) / site.standard_error.powi(2))
        .sum::<f64>();
    let degrees = sites.len() - 1;
    let chi = ChiSquared::new(degrees as f64).map_err(|error| {
        CohortInferenceError::NumericalFailure(format!(
            "failed to construct heterogeneity distribution: {error}"
        ))
    })?;
    let heterogeneity_p_value = 1.0 - chi.cdf(q);
    let tau_squared = match spec.model {
        MultisiteEffectModel::FixedEffect => 0.0,
        MultisiteEffectModel::RandomEffectsReml => estimate_reml_tau_squared(&sites)?,
    };
    let pooled = pool(&sites, tau_squared)?;
    let normal = Normal::new(0.0, 1.0).map_err(|error| {
        CohortInferenceError::NumericalFailure(format!(
            "failed to construct normal distribution: {error}"
        ))
    })?;
    let critical = normal.inverse_cdf(1.0 - spec.alpha / 2.0);
    let confidence_interval = [
        pooled.effect - critical * pooled.standard_error,
        pooled.effect + critical * pooled.standard_error,
    ];
    let prediction_interval = (spec.model == MultisiteEffectModel::RandomEffectsReml).then(|| {
        let prediction_sd = (tau_squared + pooled.standard_error.powi(2)).sqrt();
        [
            pooled.effect - critical * prediction_sd,
            pooled.effect + critical * prediction_sd,
        ]
    });
    let leave_one_site_out = sites
        .iter()
        .enumerate()
        .map(|(omitted, site)| {
            let retained = sites
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (index != omitted).then_some(value.clone()))
                .collect::<Vec<_>>();
            let tau = match spec.model {
                MultisiteEffectModel::FixedEffect => 0.0,
                MultisiteEffectModel::RandomEffectsReml => estimate_reml_tau_squared(&retained)?,
            };
            let summary = pool(&retained, tau)?;
            Ok(SiteSensitivityResult {
                omitted_site_id: site.site_id.clone(),
                pooled_effect: summary.effect,
                pooled_standard_error: summary.standard_error,
                tau_squared: tau,
            })
        })
        .collect::<Result<Vec<_>, CohortInferenceError>>()?;
    Ok(MultisiteInferenceResult {
        model: spec.model,
        site_count: sites.len(),
        total_patient_count: sites.iter().map(|site| site.patient_count).sum(),
        pooled_effect: pooled.effect,
        pooled_standard_error: pooled.standard_error,
        confidence_interval,
        prediction_interval,
        tau_squared,
        heterogeneity_q: q,
        heterogeneity_degrees_of_freedom: degrees,
        heterogeneity_p_value,
        leave_one_site_out,
        alpha: spec.alpha,
    })
}

fn validate_sites(sites: &[SiteEffect]) -> Result<Vec<SiteEffect>, CohortInferenceError> {
    if !(3..=10_000).contains(&sites.len()) {
        return Err(CohortInferenceError::InvalidInput(
            "multisite inference requires 3-10000 sites".into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut canonical = sites.to_vec();
    for site in &canonical {
        if site.site_id.is_empty()
            || site.site_id.trim() != site.site_id
            || !ids.insert(site.site_id.as_str())
            || !site.effect.is_finite()
            || !site.standard_error.is_finite()
            || site.standard_error <= 0.0
            || site.patient_count == 0
        {
            return Err(CohortInferenceError::InvalidInput(
                "sites require unique exact IDs, finite effects, positive SEs, and patients".into(),
            ));
        }
    }
    canonical.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    Ok(canonical)
}

#[derive(Clone, Copy)]
struct Pooled {
    effect: f64,
    standard_error: f64,
}

fn pool(sites: &[SiteEffect], tau_squared: f64) -> Result<Pooled, CohortInferenceError> {
    let weights = sites
        .iter()
        .map(|site| 1.0 / (site.standard_error.powi(2) + tau_squared))
        .collect::<Vec<_>>();
    let total_weight = weights.iter().sum::<f64>();
    let effect = sites
        .iter()
        .zip(&weights)
        .map(|(site, weight)| weight * site.effect)
        .sum::<f64>()
        / total_weight;
    let standard_error = (1.0 / total_weight).sqrt();
    if !effect.is_finite() || !standard_error.is_finite() || standard_error <= 0.0 {
        return Err(CohortInferenceError::NumericalFailure(
            "multisite pooling produced a non-finite result".into(),
        ));
    }
    Ok(Pooled {
        effect,
        standard_error,
    })
}

fn estimate_reml_tau_squared(sites: &[SiteEffect]) -> Result<f64, CohortInferenceError> {
    let minimum = sites
        .iter()
        .map(|site| site.effect)
        .fold(f64::INFINITY, f64::min);
    let maximum = sites
        .iter()
        .map(|site| site.effect)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut left = 0.0;
    let mut right = ((maximum - minimum).powi(2) * 100.0).max(1.0);
    let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut inner_left = right - ratio * (right - left);
    let mut inner_right = left + ratio * (right - left);
    let mut left_value = restricted_log_likelihood(sites, inner_left)?;
    let mut right_value = restricted_log_likelihood(sites, inner_right)?;
    for _ in 0..160 {
        if left_value < right_value {
            left = inner_left;
            inner_left = inner_right;
            left_value = right_value;
            inner_right = left + ratio * (right - left);
            right_value = restricted_log_likelihood(sites, inner_right)?;
        } else {
            right = inner_right;
            inner_right = inner_left;
            right_value = left_value;
            inner_left = right - ratio * (right - left);
            left_value = restricted_log_likelihood(sites, inner_left)?;
        }
    }
    let candidate = (left + right) / 2.0;
    let zero_value = restricted_log_likelihood(sites, 0.0)?;
    let candidate_value = restricted_log_likelihood(sites, candidate)?;
    Ok(if zero_value >= candidate_value {
        0.0
    } else {
        candidate
    })
}

fn restricted_log_likelihood(
    sites: &[SiteEffect],
    tau_squared: f64,
) -> Result<f64, CohortInferenceError> {
    let pooled = pool(sites, tau_squared)?;
    let variances = sites
        .iter()
        .map(|site| site.standard_error.powi(2) + tau_squared)
        .collect::<Vec<_>>();
    let sum_weights = variances.iter().map(|variance| 1.0 / variance).sum::<f64>();
    let value = -0.5
        * (variances.iter().map(|variance| variance.ln()).sum::<f64>()
            + sum_weights.ln()
            + sites
                .iter()
                .zip(variances)
                .map(|(site, variance)| (site.effect - pooled.effect).powi(2) / variance)
                .sum::<f64>());
    if !value.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "multisite REML objective is non-finite".into(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_effect_reml_detects_clear_between_site_variation() {
        let sites = [0.0, 3.0, 6.0]
            .into_iter()
            .enumerate()
            .map(|(index, effect)| SiteEffect {
                site_id: format!("s-{index}"),
                effect,
                standard_error: 0.5,
                patient_count: 10,
            })
            .collect::<Vec<_>>();
        let result = multisite_spatial_inference(
            &sites,
            &MultisiteInferenceSpec {
                model: MultisiteEffectModel::RandomEffectsReml,
                alpha: 0.05,
            },
        )
        .expect("random effects");
        assert!(result.tau_squared > 0.0);
        assert_eq!(result.pooled_effect, 3.0);
        assert!(result.prediction_interval.is_some());
    }
}
