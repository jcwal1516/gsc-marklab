use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_cohort::{
    multisite_covariate_patient_contrast, multisite_patient_contrast, multisite_spatial_inference,
    MultisiteCovariateContrastResult, MultisiteCovariatePatientRecord, MultisiteEffectModel,
    MultisiteInferenceResult, MultisiteInferenceSpec, MultisitePatientContrastResult,
    MultisitePatientEndpoint, SiteEffect,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, CliMultisiteModel, CohortError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    site_id: String,
    effect: f64,
    standard_error: f64,
    patient_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatientCsvRow {
    patient_id: String,
    site_id: String,
    group: String,
    endpoint: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CovariateCsvRow {
    patient_id: String,
    site_id: String,
    group: String,
    endpoint: f64,
    covariate: String,
    value: f64,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    model: &'static str,
    site_count: usize,
    total_patient_count: usize,
    pooled_effect: f64,
    pooled_standard_error: f64,
    confidence_interval: [f64; 2],
    prediction_interval: Option<[f64; 2]>,
    heterogeneity: Heterogeneity,
    leave_one_site_out: Vec<Sensitivity>,
    alpha: f64,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Heterogeneity {
    tau_squared: f64,
    q: f64,
    degrees_of_freedom: usize,
    p_value: f64,
}

#[derive(Debug, Serialize)]
struct Sensitivity {
    omitted_site_id: String,
    pooled_effect: f64,
    pooled_standard_error: f64,
    tau_squared: f64,
}

pub(super) fn run(
    input: PathBuf,
    model: CliMultisiteModel,
    alpha: f64,
    out: PathBuf,
) -> Result<(), CohortError> {
    let sites = read_sites(&input)?;
    let result = multisite_spatial_inference(
        &sites,
        &MultisiteInferenceSpec {
            model: model.into(),
            alpha,
        },
    )?;
    publish_json(&out, &Output::from(result))
}

pub(super) fn run_patient_contrast(
    input: PathBuf,
    group_a: String,
    group_b: String,
    model: CliMultisiteModel,
    alpha: f64,
    out: PathBuf,
) -> Result<(), CohortError> {
    let records = read_patient_records(&input)?;
    let result = multisite_patient_contrast(
        &records,
        &group_a,
        &group_b,
        &MultisiteInferenceSpec {
            model: model.into(),
            alpha,
        },
    )?;
    publish_json(&out, &PatientContrastOutput::from_result(input, result))
}

pub(super) fn run_covariate_contrast(
    input: PathBuf,
    group_a: String,
    group_b: String,
    model: CliMultisiteModel,
    alpha: f64,
    out: PathBuf,
) -> Result<(), CohortError> {
    let records = read_covariate_records(&input)?;
    let result = multisite_covariate_patient_contrast(
        &records,
        &group_a,
        &group_b,
        &MultisiteInferenceSpec {
            model: model.into(),
            alpha,
        },
    )?;
    publish_json(&out, &CovariateContrastOutput::from_result(input, result))
}

impl From<MultisiteInferenceResult> for Output {
    fn from(result: MultisiteInferenceResult) -> Self {
        Self {
            format: "marklab.cohort_multisite_inference",
            version: 1,
            model: match result.model {
                MultisiteEffectModel::FixedEffect => "fixed_effect",
                MultisiteEffectModel::RandomEffectsReml => "random_effects_reml",
            },
            site_count: result.site_count,
            total_patient_count: result.total_patient_count,
            pooled_effect: result.pooled_effect,
            pooled_standard_error: result.pooled_standard_error,
            confidence_interval: result.confidence_interval,
            prediction_interval: result.prediction_interval,
            heterogeneity: Heterogeneity {
                tau_squared: result.tau_squared,
                q: result.heterogeneity_q,
                degrees_of_freedom: result.heterogeneity_degrees_of_freedom,
                p_value: result.heterogeneity_p_value,
            },
            leave_one_site_out: result
                .leave_one_site_out
                .into_iter()
                .map(|row| Sensitivity {
                    omitted_site_id: row.omitted_site_id,
                    pooled_effect: row.pooled_effect,
                    pooled_standard_error: row.pooled_standard_error,
                    tau_squared: row.tau_squared,
                })
                .collect(),
            alpha: result.alpha,
            claim_status: "experimental_site_summary_meta_analysis",
        }
    }
}

fn read_sites(path: &std::path::Path) -> Result<Vec<SiteEffect>, CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(
            "multisite input must be a regular file within 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq(["site_id", "effect", "standard_error", "patient_count"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly site_id,effect,standard_error,patient_count".into(),
        ));
    }
    reader
        .deserialize::<CsvRow>()
        .map(|row| {
            let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(SiteEffect {
                site_id: row.site_id,
                effect: row.effect,
                standard_error: row.standard_error,
                patient_count: row.patient_count,
            })
        })
        .collect()
}

fn read_patient_records(
    path: &std::path::Path,
) -> Result<Vec<MultisitePatientEndpoint>, CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(
            "multisite patient input must be a regular file within 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq(["patient_id", "site_id", "group", "endpoint"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,site_id,group,endpoint".into(),
        ));
    }
    reader
        .deserialize::<PatientCsvRow>()
        .map(|row| {
            let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(MultisitePatientEndpoint {
                patient_id: row.patient_id,
                site_id: row.site_id,
                group: row.group,
                endpoint: row.endpoint,
            })
        })
        .collect()
}

fn read_covariate_records(
    path: &std::path::Path,
) -> Result<Vec<MultisiteCovariatePatientRecord>, CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(
            "multisite covariate input must be a regular file within 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq([
            "patient_id",
            "site_id",
            "group",
            "endpoint",
            "covariate",
            "value",
        ])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,site_id,group,endpoint,covariate,value".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, String, f64, BTreeMap<String, f64>)>::new();
    for row in reader.deserialize::<CovariateCsvRow>() {
        let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
        let entry = grouped.entry(row.patient_id.clone()).or_insert_with(|| {
            (
                row.site_id.clone(),
                row.group.clone(),
                row.endpoint,
                BTreeMap::new(),
            )
        });
        if entry.0 != row.site_id
            || entry.1 != row.group
            || entry.2.to_bits() != row.endpoint.to_bits()
        {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting site, group, or endpoint values",
                row.patient_id
            )));
        }
        if entry.3.insert(row.covariate.clone(), row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has duplicate covariate {:?}",
                row.patient_id, row.covariate
            )));
        }
    }
    Ok(grouped
        .into_iter()
        .map(|(patient_id, (site_id, group, endpoint, covariates))| {
            MultisiteCovariatePatientRecord {
                patient_id,
                site_id,
                group,
                endpoint,
                covariates: covariates.values().copied().collect(),
                covariate_names: covariates.into_keys().collect(),
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
struct PatientContrastOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: PatientContrastDesign,
    groups: PatientGroupLabels,
    sites: Vec<PatientSiteOutput>,
    pooled: Output,
}

#[derive(Debug, Serialize)]
struct PatientContrastDesign {
    randomization_unit: &'static str,
    site_effect: &'static str,
    standard_error: &'static str,
}

#[derive(Debug, Serialize)]
struct PatientGroupLabels {
    group_a: String,
    group_b: String,
}

#[derive(Debug, Serialize)]
struct PatientSiteOutput {
    site_id: String,
    group_a_patients: usize,
    group_b_patients: usize,
    group_a_mean: f64,
    group_b_mean: f64,
    effect: f64,
    standard_error: f64,
}

impl PatientContrastOutput {
    fn from_result(input: PathBuf, result: MultisitePatientContrastResult) -> Self {
        Self {
            format: "marklab.cohort_multisite_patient_contrast",
            version: 1,
            input,
            design: PatientContrastDesign {
                randomization_unit: "patient",
                site_effect: "group_a_minus_group_b",
                standard_error: "welch_independent_groups",
            },
            groups: PatientGroupLabels {
                group_a: result.group_a,
                group_b: result.group_b,
            },
            sites: result
                .sites
                .into_iter()
                .map(|site| PatientSiteOutput {
                    site_id: site.site_id,
                    group_a_patients: site.group_a_count,
                    group_b_patients: site.group_b_count,
                    group_a_mean: site.group_a_mean,
                    group_b_mean: site.group_b_mean,
                    effect: site.effect,
                    standard_error: site.standard_error,
                })
                .collect(),
            pooled: Output::from(result.pooled),
        }
    }
}

#[derive(Debug, Serialize)]
struct CovariateContrastOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: CovariateContrastDesign,
    groups: PatientGroupLabels,
    covariates: CovariateNames,
    sites: Vec<CovariateSiteOutput>,
    pooled: Output,
}

#[derive(Debug, Serialize)]
struct CovariateContrastDesign {
    population_unit: &'static str,
    site_effect: &'static str,
    standard_error: &'static str,
    nuisance_transform: &'static str,
}

#[derive(Debug, Serialize)]
struct CovariateNames {
    names: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CovariateSiteOutput {
    site_id: String,
    group_a_patients: usize,
    group_b_patients: usize,
    effect: f64,
    standard_error: f64,
    residual_degrees_of_freedom: usize,
    covariate_centers: Vec<f64>,
    covariate_scales: Vec<f64>,
}

impl CovariateContrastOutput {
    fn from_result(input: PathBuf, result: MultisiteCovariateContrastResult) -> Self {
        Self {
            format: "marklab.cohort_multisite_covariate_contrast",
            version: 1,
            input,
            design: CovariateContrastDesign {
                population_unit: "patient",
                site_effect: "adjusted_group_a_indicator",
                standard_error: "within_site_ols",
                nuisance_transform: "within_site_center_and_max_absolute_deviation_scale",
            },
            groups: PatientGroupLabels {
                group_a: result.group_a,
                group_b: result.group_b,
            },
            covariates: CovariateNames {
                names: result.covariate_names,
            },
            sites: result
                .sites
                .into_iter()
                .map(|site| CovariateSiteOutput {
                    site_id: site.site_id,
                    group_a_patients: site.group_a_count,
                    group_b_patients: site.group_b_count,
                    effect: site.effect,
                    standard_error: site.standard_error,
                    residual_degrees_of_freedom: site.residual_degrees_of_freedom,
                    covariate_centers: site.covariate_centers,
                    covariate_scales: site.covariate_scales,
                })
                .collect(),
            pooled: Output::from(result.pooled),
        }
    }
}
