use std::collections::{BTreeMap, HashSet};

use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};

use super::{numeric::welch_contrast, CohortInferenceError, MAXIMUM_PATIENTS};

#[derive(Clone, Debug)]
pub struct MultisitePatientEndpoint {
    pub patient_id: String,
    pub site_id: String,
    pub group: String,
    pub endpoint: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SitePatientContrast {
    pub site_id: String,
    pub group_a_count: usize,
    pub group_b_count: usize,
    pub group_a_mean: f64,
    pub group_b_mean: f64,
    pub effect: f64,
    pub standard_error: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultisitePatientContrastResult {
    pub group_a: String,
    pub group_b: String,
    pub sites: Vec<SitePatientContrast>,
    pub pooled: MultisiteInferenceResult,
}

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
    let total_patient_count = sites.iter().try_fold(0usize, |total, site| {
        total.checked_add(site.patient_count).ok_or_else(|| {
            CohortInferenceError::InvalidInput(
                "multisite total patient count overflows usize".into(),
            )
        })
    })?;
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
        total_patient_count,
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

pub fn multisite_patient_contrast(
    records: &[MultisitePatientEndpoint],
    group_a: &str,
    group_b: &str,
    spec: &MultisiteInferenceSpec,
) -> Result<MultisitePatientContrastResult, CohortInferenceError> {
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(CohortInferenceError::InvalidInput(
            "multisite patient groups must be distinct exact non-empty labels".into(),
        ));
    }
    if records.is_empty() || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "multisite patient contrast requires 1 to {MAXIMUM_PATIENTS} rows"
        )));
    }
    let mut patients = HashSet::with_capacity(records.len());
    let mut grouped = BTreeMap::<&str, Vec<(f64, bool)>>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.site_id.is_empty()
            || record.site_id.trim() != record.site_id
            || !record.endpoint.is_finite()
        {
            return Err(CohortInferenceError::InvalidInput(
                "multisite patient rows require exact non-empty IDs and finite endpoints".into(),
            ));
        }
        if !patients.insert(record.patient_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate multisite patient_id: {}",
                record.patient_id
            )));
        }
        let is_group_a = if record.group == group_a {
            true
        } else if record.group == group_b {
            false
        } else {
            return Err(CohortInferenceError::InvalidInput(format!(
                "patient {} has undeclared group {:?}",
                record.patient_id, record.group
            )));
        };
        grouped
            .entry(record.site_id.as_str())
            .or_default()
            .push((record.endpoint, is_group_a));
    }
    let mut sites = Vec::with_capacity(grouped.len());
    let mut effects = Vec::with_capacity(grouped.len());
    for (site_id, rows) in grouped {
        let values = rows.iter().map(|row| row.0).collect::<Vec<_>>();
        let labels = rows.iter().map(|row| row.1).collect::<Vec<_>>();
        let contrast = welch_contrast(&values, &labels).map_err(|error| match error {
            CohortInferenceError::InvalidInput(message) => {
                CohortInferenceError::InvalidInput(format!("site {site_id}: {message}"))
            }
            CohortInferenceError::NumericalFailure(message) => {
                CohortInferenceError::NumericalFailure(format!("site {site_id}: {message}"))
            }
        })?;
        sites.push(SitePatientContrast {
            site_id: site_id.to_owned(),
            group_a_count: contrast.group_a_count,
            group_b_count: contrast.group_b_count,
            group_a_mean: contrast.group_a_mean,
            group_b_mean: contrast.group_b_mean,
            effect: contrast.effect,
            standard_error: contrast.standard_error,
        });
        effects.push(SiteEffect {
            site_id: site_id.to_owned(),
            effect: contrast.effect,
            standard_error: contrast.standard_error,
            patient_count: rows.len(),
        });
    }
    let pooled = multisite_spatial_inference(&effects, spec)?;
    Ok(MultisitePatientContrastResult {
        group_a: group_a.to_owned(),
        group_b: group_b.to_owned(),
        sites,
        pooled,
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

    #[test]
    fn total_patient_count_overflow_is_rejected() {
        let sites = [usize::MAX, 1, 1]
            .into_iter()
            .enumerate()
            .map(|(index, patient_count)| SiteEffect {
                site_id: format!("s-{index}"),
                effect: index as f64,
                standard_error: 1.0,
                patient_count,
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            multisite_spatial_inference(
                &sites,
                &MultisiteInferenceSpec {
                    model: MultisiteEffectModel::FixedEffect,
                    alpha: 0.05,
                }
            ),
            Err(CohortInferenceError::InvalidInput(message))
                if message.contains("total patient count")
        ));
    }

    #[test]
    fn patient_rows_produce_exact_site_contrasts_before_pooling() {
        let records = [
            ("a-1", "site-a", "A", 3.0),
            ("a-2", "site-a", "A", 5.0),
            ("a-3", "site-a", "B", 1.0),
            ("a-4", "site-a", "B", 1.0),
            ("b-1", "site-b", "A", 4.0),
            ("b-2", "site-b", "A", 6.0),
            ("b-3", "site-b", "B", 2.0),
            ("b-4", "site-b", "B", 2.0),
            ("c-1", "site-c", "A", 5.0),
            ("c-2", "site-c", "A", 7.0),
            ("c-3", "site-c", "B", 3.0),
            ("c-4", "site-c", "B", 3.0),
        ]
        .into_iter()
        .map(
            |(patient_id, site_id, group, endpoint)| MultisitePatientEndpoint {
                patient_id: patient_id.into(),
                site_id: site_id.into(),
                group: group.into(),
                endpoint,
            },
        )
        .collect::<Vec<_>>();
        let spec = MultisiteInferenceSpec {
            model: MultisiteEffectModel::FixedEffect,
            alpha: 0.05,
        };
        let result = multisite_patient_contrast(&records, "A", "B", &spec).expect("contrast");
        assert_eq!(result.sites.len(), 3);
        assert!(result
            .sites
            .iter()
            .all(|site| site.effect == 3.0 && site.standard_error == 1.0));
        assert_eq!(result.pooled.pooled_effect, 3.0);
        assert_eq!(result.pooled.total_patient_count, 12);

        let mut duplicate = records.clone();
        duplicate[4].patient_id = "a-1".into();
        assert!(matches!(
            multisite_patient_contrast(&duplicate, "A", "B", &spec),
            Err(CohortInferenceError::InvalidInput(message)) if message.contains("duplicate")
        ));

        let confounded = records
            .iter()
            .filter(|record| !(record.site_id == "site-c" && record.group == "B"))
            .cloned()
            .collect::<Vec<_>>();
        assert!(matches!(
            multisite_patient_contrast(&confounded, "A", "B", &spec),
            Err(CohortInferenceError::InvalidInput(message))
                if message.contains("site site-c") && message.contains("two patients")
        ));
    }
}
