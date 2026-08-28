use std::path::PathBuf;

use marklab_cohort::{
    bootstrap_equivalence, hierarchical_bootstrap, BootstrapEquivalenceResult,
    BootstrapEquivalenceSpec, HierarchicalBootstrapResult, HierarchicalBootstrapSpec,
    HierarchicalScalarRecord, InferenceDesign, InferenceNullFamily, InferencePermutationUnit,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, validate_input_file, CohortError};

#[derive(Debug, Deserialize)]
struct CsvRow {
    patient_id: String,
    specimen_id: String,
    endpoint: f64,
}

pub(super) fn run(
    input: PathBuf,
    replicates: usize,
    seed: u64,
    alpha: f64,
    out: PathBuf,
) -> Result<(), CohortError> {
    let records = read_records(&input)?;
    let result = hierarchical_bootstrap(
        &records,
        &HierarchicalBootstrapSpec {
            replicates,
            seed,
            alpha,
        },
    )?;
    publish_json(
        &out,
        &HierarchicalBootstrapOutput::from_result(input, result),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_equivalence(
    input: PathBuf,
    lower_margin: f64,
    upper_margin: f64,
    alpha: f64,
    replicates: usize,
    seed: u64,
    margin_rationale: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let records = read_records(&input)?;
    let result = bootstrap_equivalence(
        &records,
        &BootstrapEquivalenceSpec {
            lower_margin,
            upper_margin,
            margin_rationale,
            bootstrap: HierarchicalBootstrapSpec {
                replicates,
                seed,
                alpha,
            },
        },
    )?;
    publish_json(
        &out,
        &BootstrapEquivalenceOutput::from_result(input, result),
    )
}

fn read_records(path: &PathBuf) -> Result<Vec<HierarchicalScalarRecord>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if !headers.iter().eq(["patient_id", "specimen_id", "endpoint"]) {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,specimen_id,endpoint".into(),
        ));
    }
    reader
        .deserialize::<CsvRow>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(HierarchicalScalarRecord {
                patient_id: row.patient_id,
                specimen_id: row.specimen_id,
                endpoint: row.endpoint,
            })
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct HierarchicalBootstrapOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: HierarchicalDesignOutput,
    patients: usize,
    specimens: usize,
    observed_mean: f64,
    interval: IntervalOutput,
    replicates: ReplicateOutput,
    seed: u64,
    alpha: f64,
}

impl HierarchicalBootstrapOutput {
    fn from_result(input: PathBuf, result: HierarchicalBootstrapResult) -> Self {
        let design = hierarchical_design(&result.inference_design);
        Self {
            format: "marklab.cohort_hierarchical_bootstrap",
            version: 1,
            input,
            design,
            patients: result.patient_count,
            specimens: result.specimen_count,
            observed_mean: result.observed_mean,
            interval: IntervalOutput {
                lower: result.interval.lower,
                upper: result.interval.upper,
                level: result.interval.level,
            },
            replicates: ReplicateOutput {
                requested: result.replicates_requested,
                attempted: result.replicates_attempted,
                completed: result.replicates_completed,
            },
            seed: result.seed,
            alpha: result.alpha,
        }
    }
}

#[derive(Debug, Serialize)]
struct HierarchicalDesignOutput {
    levels: [&'static str; 2],
    null_family: &'static str,
    permutation_unit: &'static str,
    statistic: &'static str,
    interval_method: &'static str,
}

#[derive(Debug, Serialize)]
struct IntervalOutput {
    lower: f64,
    upper: f64,
    level: f64,
}

#[derive(Debug, Serialize)]
struct ReplicateOutput {
    requested: usize,
    attempted: usize,
    completed: usize,
}

#[derive(Debug, Serialize)]
struct BootstrapEquivalenceOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: HierarchicalDesignOutput,
    observed_mean: f64,
    interval: BootstrapEquivalenceIntervalOutput,
    margins: [f64; 2],
    margin_rationale: String,
    equivalent: bool,
    replicates: ReplicateOutput,
    seed: u64,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct BootstrapEquivalenceIntervalOutput {
    lower: f64,
    upper: f64,
    level: f64,
    method: &'static str,
}

impl BootstrapEquivalenceOutput {
    fn from_result(input: PathBuf, result: BootstrapEquivalenceResult) -> Self {
        let design = hierarchical_design(&result.bootstrap.inference_design);
        Self {
            format: "marklab.cohort_bootstrap_equivalence",
            version: 1,
            input,
            design,
            observed_mean: result.bootstrap.observed_mean,
            interval: BootstrapEquivalenceIntervalOutput {
                lower: result.bootstrap.interval.lower,
                upper: result.bootstrap.interval.upper,
                level: result.bootstrap.interval.level,
                method: "nearest_rank_percentile",
            },
            margins: [result.lower_margin, result.upper_margin],
            margin_rationale: result.margin_rationale,
            equivalent: result.equivalent,
            replicates: ReplicateOutput {
                requested: result.bootstrap.replicates_requested,
                attempted: result.bootstrap.replicates_attempted,
                completed: result.bootstrap.replicates_completed,
            },
            seed: result.bootstrap.seed,
            claim_status: "experimental_percentile_interval",
        }
    }
}

fn hierarchical_design(design: &InferenceDesign) -> HierarchicalDesignOutput {
    let null_family = match design.null_family() {
        InferenceNullFamily::HierarchicalBootstrap => "hierarchical_bootstrap",
        _ => unreachable!("hierarchical bootstrap returned another null family"),
    };
    let permutation_unit = match design.permutation_unit() {
        InferencePermutationUnit::PatientThenNestedSpecimen => "patient_then_nested_specimen",
        _ => unreachable!("hierarchical bootstrap returned another permutation unit"),
    };
    HierarchicalDesignOutput {
        levels: ["patient", "specimen"],
        null_family,
        permutation_unit,
        statistic: "specimen_row_mean",
        interval_method: "nearest_rank_percentile",
    }
}
