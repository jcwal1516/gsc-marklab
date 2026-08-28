use std::collections::{BTreeMap, HashSet};

use super::{
    covariate::fit_ols, multisite_spatial_inference, numeric::stable_mean, CohortInferenceError,
    MultisiteInferenceResult, MultisiteInferenceSpec, SiteEffect, MAXIMUM_PATIENTS,
};

const MAXIMUM_NUISANCE_COVARIATES: usize = 32;
const MAXIMUM_MULTISITE_COVARIATE_WORK: usize = 100_000_000;

/// One independent patient in one site with an exact nuisance-covariate vector.
#[derive(Clone, Debug)]
pub struct MultisiteCovariatePatientRecord {
    /// Globally unique stable patient identifier.
    pub patient_id: String,
    /// Exact site identifier.
    pub site_id: String,
    /// Exact declared group label.
    pub group: String,
    /// Finite scalar endpoint.
    pub endpoint: f64,
    /// Exact ordered nuisance-column names shared by every patient and site.
    pub covariate_names: Vec<String>,
    /// Finite nuisance values aligned to `covariate_names`.
    pub covariates: Vec<f64>,
}

/// One site's adjusted patient-level group contrast.
#[derive(Clone, Debug, PartialEq)]
pub struct AdjustedSitePatientContrast {
    /// Exact site identifier.
    pub site_id: String,
    /// Group A patient count.
    pub group_a_count: usize,
    /// Group B patient count.
    pub group_b_count: usize,
    /// Adjusted group-A-minus-group-B coefficient.
    pub effect: f64,
    /// OLS standard error of the adjusted group coefficient.
    pub standard_error: f64,
    /// Full-model residual degrees of freedom.
    pub residual_degrees_of_freedom: usize,
    /// Site-specific deterministic nuisance-column centers.
    pub covariate_centers: Vec<f64>,
    /// Site-specific positive max-absolute-deviation scales.
    pub covariate_scales: Vec<f64>,
}

/// Adjusted site effects plus canonical fixed/REML pooling and sensitivity.
#[derive(Clone, Debug, PartialEq)]
pub struct MultisiteCovariateContrastResult {
    /// Exact first group label.
    pub group_a: String,
    /// Exact second group label.
    pub group_b: String,
    /// Exact ordered nuisance-column names.
    pub covariate_names: Vec<String>,
    /// Canonical site order and adjusted site effects.
    pub sites: Vec<AdjustedSitePatientContrast>,
    /// Existing fixed-effect or REML pooled result.
    pub pooled: MultisiteInferenceResult,
}

/// Fit the same fixed nuisance matrix within every site, then pool adjusted group effects.
pub fn multisite_covariate_patient_contrast(
    records: &[MultisiteCovariatePatientRecord],
    group_a: &str,
    group_b: &str,
    spec: &MultisiteInferenceSpec,
) -> Result<MultisiteCovariateContrastResult, CohortInferenceError> {
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(CohortInferenceError::InvalidInput(
            "multisite covariate groups must be distinct exact non-empty labels".into(),
        ));
    }
    if records.is_empty() || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "multisite covariate contrast requires 1 to {MAXIMUM_PATIENTS} patient rows"
        )));
    }
    let names = &records[0].covariate_names;
    if names.is_empty()
        || names.len() > MAXIMUM_NUISANCE_COVARIATES
        || names.iter().collect::<HashSet<_>>().len() != names.len()
        || names.iter().any(|name| {
            name.is_empty()
                || name.len() > 128
                || name.trim() != name
                || name.chars().any(char::is_control)
        })
    {
        return Err(CohortInferenceError::InvalidInput(
            "multisite nuisance names must be 1-32 exact unique bounded names".into(),
        ));
    }
    let mut patient_ids = HashSet::with_capacity(records.len());
    let mut grouped = BTreeMap::<&str, BTreeMap<&str, Row>>::new();
    for record in records {
        if record.patient_id.is_empty()
            || record.patient_id.trim() != record.patient_id
            || record.site_id.is_empty()
            || record.site_id.trim() != record.site_id
            || !record.endpoint.is_finite()
            || record.covariate_names != *names
            || record.covariates.len() != names.len()
            || record.covariates.iter().any(|value| !value.is_finite())
        {
            return Err(CohortInferenceError::InvalidInput(
                "multisite covariate rows require exact IDs and one complete finite nuisance vector"
                    .into(),
            ));
        }
        if !patient_ids.insert(record.patient_id.as_str()) {
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
        grouped.entry(&record.site_id).or_default().insert(
            &record.patient_id,
            Row {
                endpoint: record.endpoint,
                group_a: is_group_a,
                covariates: record.covariates.clone(),
            },
        );
    }
    let full_columns = names.len() + 2;
    let solver_work = grouped
        .len()
        .checked_mul(full_columns.pow(3))
        .ok_or_else(work_limit_error)?;
    let work = records
        .len()
        .checked_mul(full_columns)
        .and_then(|value| value.checked_mul(full_columns))
        .and_then(|value| value.checked_add(solver_work))
        .ok_or_else(work_limit_error)?;
    if work > MAXIMUM_MULTISITE_COVARIATE_WORK {
        return Err(work_limit_error());
    }

    let mut sites = Vec::with_capacity(grouped.len());
    let mut effects = Vec::with_capacity(grouped.len());
    for (site_id, rows) in grouped {
        let rows = rows.into_values().collect::<Vec<_>>();
        let group_a_count = rows.iter().filter(|row| row.group_a).count();
        let group_b_count = rows.len() - group_a_count;
        if group_a_count < 2 || group_b_count < 2 {
            return Err(CohortInferenceError::InvalidInput(format!(
                "site {site_id}: each group must contain at least two patients"
            )));
        }
        let mut centers = Vec::with_capacity(names.len());
        let mut scales = Vec::with_capacity(names.len());
        for (column, name) in names.iter().enumerate() {
            let values = rows
                .iter()
                .map(|row| row.covariates[column])
                .collect::<Vec<_>>();
            let center = stable_mean(&values)?;
            let scale = values
                .iter()
                .map(|value| (value - center).abs())
                .fold(0.0_f64, f64::max);
            if !scale.is_finite() || scale == 0.0 {
                return Err(CohortInferenceError::InvalidInput(format!(
                    "site {site_id}: nuisance covariate {:?} must vary",
                    name
                )));
            }
            centers.push(center);
            scales.push(scale);
        }
        let design = rows
            .iter()
            .map(|row| {
                let mut design = Vec::with_capacity(full_columns);
                design.push(1.0);
                design.extend(
                    row.covariates
                        .iter()
                        .enumerate()
                        .map(|(column, value)| (value - centers[column]) / scales[column]),
                );
                design.push(f64::from(row.group_a));
                design
            })
            .collect::<Vec<_>>();
        let outcomes = rows.iter().map(|row| row.endpoint).collect::<Vec<_>>();
        let fit =
            fit_ols(&design, &outcomes, Some(full_columns - 1)).map_err(|error| match error {
                CohortInferenceError::InvalidInput(message) => {
                    CohortInferenceError::InvalidInput(format!("site {site_id}: {message}"))
                }
                CohortInferenceError::NumericalFailure(message) => {
                    CohortInferenceError::NumericalFailure(format!("site {site_id}: {message}"))
                }
            })?;
        sites.push(AdjustedSitePatientContrast {
            site_id: site_id.to_owned(),
            group_a_count,
            group_b_count,
            effect: fit.target_coefficient,
            standard_error: fit.target_standard_error,
            residual_degrees_of_freedom: fit.residual_degrees_of_freedom,
            covariate_centers: centers,
            covariate_scales: scales,
        });
        effects.push(SiteEffect {
            site_id: site_id.to_owned(),
            effect: fit.target_coefficient,
            standard_error: fit.target_standard_error,
            patient_count: rows.len(),
        });
    }
    let pooled = multisite_spatial_inference(&effects, spec)?;
    Ok(MultisiteCovariateContrastResult {
        group_a: group_a.to_owned(),
        group_b: group_b.to_owned(),
        covariate_names: names.clone(),
        sites,
        pooled,
    })
}

#[derive(Clone)]
struct Row {
    endpoint: f64,
    group_a: bool,
    covariates: Vec<f64>,
}

fn work_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "multisite covariate OLS work exceeds the {MAXIMUM_MULTISITE_COVARIATE_WORK}-unit limit"
    ))
}
