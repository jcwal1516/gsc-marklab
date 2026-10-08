use std::{collections::BTreeMap, path::PathBuf};

use marklab_cohort::{
    functional_equivalence_band, FunctionalDifferenceCurve, FunctionalEquivalenceResult,
    FunctionalEquivalenceSpec,
};
use serde::{Deserialize, Serialize};

use super::{input::validate_input_file_with_message, publication::publish_json, CohortError};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    patient_id: String,
    axis: f64,
    difference: f64,
    margin: f64,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    patient_count: usize,
    axis: Vec<f64>,
    mean_difference: Vec<f64>,
    lower_simultaneous_band: Vec<f64>,
    upper_simultaneous_band: Vec<f64>,
    margin_curve: Vec<f64>,
    simultaneous_level: f64,
    max_deviation_critical: f64,
    equivalent_at_all_scales: bool,
    failing_scales: Vec<f64>,
    margin_rationale: String,
    replicates: Replicates,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Replicates {
    requested: usize,
    completed: usize,
    seed: u64,
    unit: &'static str,
}

pub(super) fn run(
    input: PathBuf,
    alpha: f64,
    replicates: usize,
    seed: u64,
    margin_rationale: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let (curves, margin_curve) = read_curves(&input)?;
    let result = functional_equivalence_band(
        &curves,
        &FunctionalEquivalenceSpec {
            margin_curve,
            alpha,
            replicates,
            seed,
            margin_rationale,
        },
    )?;
    publish_json(&out, &Output::from(result))
}

impl From<FunctionalEquivalenceResult> for Output {
    fn from(result: FunctionalEquivalenceResult) -> Self {
        Self {
            format: "marklab.cohort_functional_equivalence",
            version: 1,
            patient_count: result.patient_count,
            axis: result.points.iter().map(|point| point.axis).collect(),
            mean_difference: result
                .points
                .iter()
                .map(|point| point.mean_difference)
                .collect(),
            lower_simultaneous_band: result.points.iter().map(|point| point.lower_band).collect(),
            upper_simultaneous_band: result.points.iter().map(|point| point.upper_band).collect(),
            margin_curve: result.points.iter().map(|point| point.margin).collect(),
            simultaneous_level: result.simultaneous_level,
            max_deviation_critical: result.max_deviation_critical,
            equivalent_at_all_scales: result.equivalent_at_all_scales,
            failing_scales: result.failing_scales,
            margin_rationale: result.margin_rationale,
            replicates: Replicates {
                requested: result.replicates_requested,
                completed: result.replicates_completed,
                seed: result.seed,
                unit: "patient_curve",
            },
            claim_status: "experimental_simultaneous_bootstrap_equivalence",
        }
    }
}

fn read_curves(
    path: &std::path::Path,
) -> Result<(Vec<FunctionalDifferenceCurve>, Vec<f64>), CohortError> {
    validate_input_file_with_message(
        path,
        "functional equivalence input must be a regular file within 16 MiB",
    )?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq(["patient_id", "axis", "difference", "margin"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,axis,difference,margin".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, Vec<(f64, f64, f64)>>::new();
    for row in reader.deserialize::<CsvRow>() {
        let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
        grouped
            .entry(row.patient_id)
            .or_default()
            .push((row.axis, row.difference, row.margin));
    }
    let mut curves = Vec::with_capacity(grouped.len());
    let mut declared_axis = None::<Vec<f64>>;
    let mut margins = None::<Vec<f64>>;
    for (patient_id, mut points) in grouped {
        points.sort_by(|left, right| left.0.total_cmp(&right.0));
        let axis = points.iter().map(|point| point.0).collect::<Vec<_>>();
        let patient_margins = points.iter().map(|point| point.2).collect::<Vec<_>>();
        if declared_axis.as_ref().is_some_and(|value| *value != axis)
            || margins
                .as_ref()
                .is_some_and(|value| *value != patient_margins)
        {
            return Err(CohortError::Input(
                "every patient must declare the identical axis and margin curve".into(),
            ));
        }
        declared_axis.get_or_insert_with(|| axis.clone());
        margins.get_or_insert(patient_margins);
        curves.push(FunctionalDifferenceCurve {
            patient_id,
            axis,
            differences: points.into_iter().map(|point| point.1).collect(),
        });
    }
    Ok((curves, margins.unwrap_or_default()))
}
