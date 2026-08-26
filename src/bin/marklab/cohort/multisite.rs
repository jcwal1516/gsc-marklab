use std::{fs, path::PathBuf};

use marklab_cohort::{
    multisite_spatial_inference, MultisiteEffectModel, MultisiteInferenceResult,
    MultisiteInferenceSpec, SiteEffect,
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
