use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand, ValueEnum};
use marklab_cohort::{
    functional_two_sample_blocked_permutation, functional_two_sample_permutation,
    max_t_multiple_endpoint_blocked_permutation, max_t_multiple_endpoint_permutation,
    paired_patient_permutation_test, patient_level_blocked_energy_distance,
    patient_level_blocked_mmd, patient_level_energy_distance, patient_level_mmd,
    patient_level_permutation_test, BlockedEnergyDistanceResult,
    BlockedFunctionalPermutationResult, BlockedMmdPermutationResult, CohortInferenceError,
    EnergyDistanceResult, EnergyDistanceSpec, EnergyMetric, Fingerprint, FunctionalCurve,
    FunctionalPermutationResult, FunctionalPermutationSpec, FunctionalTestStatistic,
    InferenceNullFamily, InferencePermutationUnit, MaxTPermutationResult, MaxTPermutationSpec,
    MmdEstimator, MmdKernel, MmdPermutationResult, MmdPermutationSpec, PairedPatientEndpoint,
    PairedPatientPermutationResult, PairedPatientPermutationSpec, PatientEndpoint,
    PatientEndpointVector, PatientExchangeabilityBlock, PatientPermutationResult,
    PatientPermutationSpec, PermutationAlternative,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[path = "cohort/cluster.rs"]
mod cluster;
#[path = "cohort/effects.rs"]
mod effects;
#[path = "cohort/equivalence.rs"]
mod equivalence;
#[path = "cohort/fingerprint.rs"]
mod fingerprint;
#[path = "cohort/functional_equivalence.rs"]
mod functional_equivalence;
#[path = "cohort/hierarchical_bootstrap.rs"]
mod hierarchical_bootstrap;
#[path = "cohort/multisite.rs"]
mod multisite;
#[path = "cohort/noninferiority.rs"]
mod noninferiority;
#[path = "cohort/publication.rs"]
mod publication;
#[path = "cohort/repeated.rs"]
mod repeated;

use publication::publish_json;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct CohortCli {
    #[command(subcommand)]
    command: CohortTopLevel,
}

#[derive(Debug, Subcommand)]
enum CohortTopLevel {
    Cohort {
        #[command(subcommand)]
        command: CohortCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CohortCommand {
    Permutation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long, value_enum)]
        alternative: CliAlternative,
        #[arg(long)]
        out: PathBuf,
    },
    PairedPermutation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        condition_a: String,
        #[arg(long)]
        condition_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long, value_enum)]
        alternative: CliAlternative,
        #[arg(long)]
        out: PathBuf,
    },
    ClusterPermutation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long, value_enum)]
        alternative: CliAlternative,
        #[arg(long)]
        out: PathBuf,
    },
    RepeatedFreedmanLane {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MultisiteInference {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum)]
        model: CliMultisiteModel,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        out: PathBuf,
    },
    MultisitePatientContrast {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long, value_enum)]
        model: CliMultisiteModel,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        out: PathBuf,
    },
    FunctionalPermutation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long, value_enum)]
        statistic: CliFunctionalStatistic,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FunctionalEquivalence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        replicates: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
    MaxT {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        out: PathBuf,
    },
    Mmd {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long, value_enum)]
        kernel: CliMmdKernel,
        #[arg(long)]
        bandwidth: Option<f64>,
        #[arg(long, value_enum)]
        estimator: CliMmdEstimator,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    Energy {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long, value_enum)]
        metric: CliEnergyMetric,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FingerprintDistance {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        left_sample: String,
        #[arg(long)]
        right_sample: String,
        #[arg(long)]
        spec_version: String,
        #[arg(long)]
        out: PathBuf,
    },
    RegionCompatibility {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        left_sample: String,
        #[arg(long)]
        right_sample: String,
        #[arg(long)]
        spec_version: String,
        #[arg(long)]
        out: PathBuf,
    },
    Equivalence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        lower_margin: f64,
        #[arg(long, allow_hyphen_values = true)]
        upper_margin: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
    Noninferiority {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum)]
        direction: noninferiority::CliDirection,
        #[arg(long)]
        margin: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalBootstrap {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        replicates: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        out: PathBuf,
    },
    BootstrapEquivalence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        lower_margin: f64,
        #[arg(long, allow_hyphen_values = true)]
        upper_margin: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        replicates: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum CliAlternative {
    Less,
    Greater,
    TwoSided,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum CliFunctionalStatistic {
    L2,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliMmdKernel {
    Linear,
    Rbf,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum CliMmdEstimator {
    Unbiased,
    Biased,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum CliEnergyMetric {
    Euclidean,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliMultisiteModel {
    FixedEffect,
    RandomEffectsReml,
}

impl From<CliMultisiteModel> for marklab_cohort::MultisiteEffectModel {
    fn from(value: CliMultisiteModel) -> Self {
        match value {
            CliMultisiteModel::FixedEffect => Self::FixedEffect,
            CliMultisiteModel::RandomEffectsReml => Self::RandomEffectsReml,
        }
    }
}

impl From<CliEnergyMetric> for EnergyMetric {
    fn from(value: CliEnergyMetric) -> Self {
        match value {
            CliEnergyMetric::Euclidean => Self::Euclidean,
        }
    }
}

impl From<CliMmdEstimator> for MmdEstimator {
    fn from(value: CliMmdEstimator) -> Self {
        match value {
            CliMmdEstimator::Unbiased => Self::Unbiased,
            CliMmdEstimator::Biased => Self::Biased,
        }
    }
}

impl From<CliFunctionalStatistic> for FunctionalTestStatistic {
    fn from(value: CliFunctionalStatistic) -> Self {
        match value {
            CliFunctionalStatistic::L2 => Self::L2,
        }
    }
}

impl From<CliAlternative> for PermutationAlternative {
    fn from(value: CliAlternative) -> Self {
        match value {
            CliAlternative::Less => Self::Less,
            CliAlternative::Greater => Self::Greater,
            CliAlternative::TwoSided => Self::TwoSided,
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum CohortError {
    #[error("cohort input error: {0}")]
    Input(String),
    #[error("cohort inference failed: {0}")]
    Inference(#[from] CohortInferenceError),
    #[error("cohort output error at {path}: {source}")]
    Output {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cohort JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub(super) fn into_marklab_error(error: CohortError) -> marklab::MarklabError {
    match error {
        CohortError::Input(message) => marklab::MarklabError::Validation(message),
        CohortError::Inference(CohortInferenceError::InvalidInput(message)) => {
            marklab::MarklabError::Validation(message)
        }
        CohortError::Inference(CohortInferenceError::NumericalFailure(message)) => {
            marklab::MarklabError::Compute(message)
        }
        CohortError::Output { path, source } => marklab::MarklabError::io(path, source),
        CohortError::Json(error) => marklab::MarklabError::Json(error),
    }
}

pub(super) fn run_cli() -> Result<(), CohortError> {
    match CohortCli::parse_from(std::env::args_os()).command {
        CohortTopLevel::Cohort {
            command:
                CohortCommand::Permutation {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative,
                    out,
                },
        } => {
            let records = read_records(&input)?;
            let result = patient_level_permutation_test(
                &records,
                &PatientPermutationSpec {
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative: alternative.into(),
                },
            )?;
            publish_json(
                &out,
                &PermutationOutput::from_result(input, alternative, result),
            )
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::MaxT {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    out,
                },
        } => {
            let max_t_input = read_max_t_patients(&input)?;
            let spec = MaxTPermutationSpec {
                group_a,
                group_b,
                permutations,
                seed,
                alpha,
            };
            let (result, blocked) = match max_t_input.blocks {
                Some(blocks) => (
                    max_t_multiple_endpoint_blocked_permutation(
                        &max_t_input.patients,
                        &blocks,
                        &spec,
                    )?,
                    true,
                ),
                None => (
                    max_t_multiple_endpoint_permutation(&max_t_input.patients, &spec)?,
                    false,
                ),
            };
            publish_json(&out, &MaxTOutput::from_result(input, result, blocked))
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::Mmd {
                    input,
                    group_a,
                    group_b,
                    kernel,
                    bandwidth,
                    estimator,
                    permutations,
                    seed,
                    out,
                },
        } => {
            let kernel = match (kernel, bandwidth) {
                (CliMmdKernel::Linear, None) => MmdKernel::Linear,
                (CliMmdKernel::Linear, Some(_)) => {
                    return Err(CohortError::Input("linear MMD forbids --bandwidth".into()))
                }
                (CliMmdKernel::Rbf, Some(bandwidth)) => MmdKernel::Rbf { bandwidth },
                (CliMmdKernel::Rbf, None) => {
                    return Err(CohortError::Input("RBF MMD requires --bandwidth".into()))
                }
            };
            let fingerprint_input = read_fingerprints(&input)?;
            let spec = MmdPermutationSpec {
                group_a,
                group_b,
                kernel,
                estimator: estimator.into(),
                permutations,
                seed,
            };
            let output = match fingerprint_input.blocks {
                Some(blocks) => MmdOutput::from_blocked_result(
                    input,
                    estimator,
                    patient_level_blocked_mmd(&fingerprint_input.fingerprints, &blocks, &spec)?,
                ),
                None => MmdOutput::from_result(
                    input,
                    estimator,
                    patient_level_mmd(&fingerprint_input.fingerprints, &spec)?,
                ),
            };
            publish_json(&out, &output)
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::Energy {
                    input,
                    group_a,
                    group_b,
                    metric,
                    permutations,
                    seed,
                    out,
                },
        } => {
            let fingerprint_input = read_fingerprints(&input)?;
            let spec = EnergyDistanceSpec {
                group_a,
                group_b,
                metric: metric.into(),
                permutations,
                seed,
            };
            let output = match fingerprint_input.blocks {
                Some(blocks) => EnergyOutput::from_blocked_result(
                    input,
                    metric,
                    patient_level_blocked_energy_distance(
                        &fingerprint_input.fingerprints,
                        &blocks,
                        &spec,
                    )?,
                ),
                None => EnergyOutput::from_result(
                    input,
                    metric,
                    patient_level_energy_distance(&fingerprint_input.fingerprints, &spec)?,
                ),
            };
            publish_json(&out, &output)
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::FingerprintDistance {
                    input,
                    left_sample,
                    right_sample,
                    spec_version,
                    out,
                },
        } => fingerprint::run(input, left_sample, right_sample, spec_version, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::RegionCompatibility {
                    input,
                    left_sample,
                    right_sample,
                    spec_version,
                    out,
                },
        } => fingerprint::run_compatibility(input, left_sample, right_sample, spec_version, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::Equivalence {
                    input,
                    lower_margin,
                    upper_margin,
                    alpha,
                    margin_rationale,
                    out,
                },
        } => equivalence::run(
            input,
            lower_margin,
            upper_margin,
            alpha,
            margin_rationale,
            out,
        ),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::Noninferiority {
                    input,
                    direction,
                    margin,
                    alpha,
                    margin_rationale,
                    out,
                },
        } => noninferiority::run(input, direction, margin, alpha, margin_rationale, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::HierarchicalBootstrap {
                    input,
                    replicates,
                    seed,
                    alpha,
                    out,
                },
        } => hierarchical_bootstrap::run(input, replicates, seed, alpha, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::BootstrapEquivalence {
                    input,
                    lower_margin,
                    upper_margin,
                    alpha,
                    replicates,
                    seed,
                    margin_rationale,
                    out,
                },
        } => hierarchical_bootstrap::run_equivalence(
            input,
            lower_margin,
            upper_margin,
            alpha,
            replicates,
            seed,
            margin_rationale,
            out,
        ),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::FunctionalPermutation {
                    input,
                    group_a,
                    group_b,
                    statistic,
                    permutations,
                    seed,
                    out,
                },
        } => {
            let functional_input = read_functional_curves(&input)?;
            let spec = FunctionalPermutationSpec {
                group_a,
                group_b,
                statistic: statistic.into(),
                permutations,
                seed,
            };
            let output = match functional_input.blocks {
                Some(blocks) => FunctionalPermutationOutput::from_blocked_result(
                    input,
                    statistic,
                    functional_two_sample_blocked_permutation(
                        &functional_input.curves,
                        &blocks,
                        &spec,
                    )?,
                ),
                None => FunctionalPermutationOutput::from_result(
                    input,
                    statistic,
                    functional_two_sample_permutation(&functional_input.curves, &spec)?,
                ),
            };
            publish_json(&out, &output)
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::FunctionalEquivalence {
                    input,
                    alpha,
                    replicates,
                    seed,
                    margin_rationale,
                    out,
                },
        } => functional_equivalence::run(input, alpha, replicates, seed, margin_rationale, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::PairedPermutation {
                    input,
                    condition_a,
                    condition_b,
                    permutations,
                    seed,
                    alternative,
                    out,
                },
        } => {
            let records = read_paired_records(&input)?;
            let result = paired_patient_permutation_test(
                &records,
                &PairedPatientPermutationSpec {
                    condition_a,
                    condition_b,
                    permutations,
                    seed,
                    alternative: alternative.into(),
                },
            )?;
            publish_json(
                &out,
                &PairedPermutationOutput::from_result(input, alternative, result),
            )
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::ClusterPermutation {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative,
                    out,
                },
        } => cluster::run(
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alternative,
            out,
        ),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::RepeatedFreedmanLane {
                    input,
                    permutations,
                    seed,
                    out,
                },
        } => repeated::run(input, permutations, seed, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::MultisiteInference {
                    input,
                    model,
                    alpha,
                    out,
                },
        } => multisite::run(input, model, alpha, out),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::MultisitePatientContrast {
                    input,
                    group_a,
                    group_b,
                    model,
                    alpha,
                    out,
                },
        } => multisite::run_patient_contrast(input, group_a, group_b, model, alpha, out),
    }
}

#[derive(Debug, Deserialize)]
struct CsvRecord {
    patient_id: String,
    group: String,
    endpoint: f64,
    #[serde(default)]
    block: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PairedCsvRecord {
    patient_id: String,
    condition: String,
    endpoint: f64,
}

#[derive(Debug, Deserialize)]
struct FunctionalCsvRecord {
    patient_id: String,
    group: String,
    axis: f64,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

struct FunctionalInput {
    curves: Vec<FunctionalCurve>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

#[derive(Debug, Deserialize)]
struct MaxTCsvRecord {
    patient_id: String,
    group: String,
    endpoint: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

struct MaxTInput {
    patients: Vec<PatientEndpointVector>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

#[derive(Debug, Deserialize)]
struct FingerprintCsvRecord {
    patient_id: String,
    group: String,
    feature: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

struct FingerprintInput {
    fingerprints: Vec<Fingerprint>,
    blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

fn read_records(path: &Path) -> Result<Vec<PatientEndpoint>, CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(CohortError::Input(format!(
            "input must be a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(format!(
            "input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"
        )));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let valid_headers = headers.iter().eq(["patient_id", "group", "endpoint"])
        || headers
            .iter()
            .eq(["patient_id", "group", "endpoint", "block"]);
    if !valid_headers {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,endpoint with optional trailing block"
                .into(),
        ));
    }
    reader
        .deserialize::<CsvRecord>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(PatientEndpoint {
                patient_id: row.patient_id,
                group: row.group,
                endpoint: row.endpoint,
                block: row.block.filter(|block| !block.is_empty()),
            })
        })
        .collect()
}

fn read_paired_records(path: &Path) -> Result<Vec<PairedPatientEndpoint>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if !headers.iter().eq(["patient_id", "condition", "endpoint"]) {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,condition,endpoint".into(),
        ));
    }
    reader
        .deserialize::<PairedCsvRecord>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(PairedPatientEndpoint {
                patient_id: row.patient_id,
                condition: row.condition,
                endpoint: row.endpoint,
            })
        })
        .collect()
}

fn read_functional_curves(path: &Path) -> Result<FunctionalInput, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "axis", "value", "block"]);
    if !blocked && !headers.iter().eq(["patient_id", "group", "axis", "value"]) {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,axis,value or patient_id,group,axis,value,block".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, Vec<(f64, f64)>)>::new();
    for decoded in reader.deserialize::<FunctionalCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if !row.axis.is_finite() || !row.value.is_finite() {
            return Err(CohortError::Input(
                "functional CSV axis and value fields must be finite".into(),
            ));
        }
        if blocked && row.block.is_none() {
            return Err(CohortError::Input(format!(
                "patient {} is missing its functional block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), Vec::new()));
        if entry.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting functional blocks",
                row.patient_id
            )));
        }
        entry.2.push((row.axis, row.value));
    }
    let mut curves = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, mut points)) in grouped {
        points.sort_by(|left, right| left.0.total_cmp(&right.0));
        if points.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(CohortError::Input(format!(
                "patient {patient_id} has duplicate or non-increasing axis rows"
            )));
        }
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        let (axis, values): (Vec<_>, Vec<_>) = points.into_iter().unzip();
        curves.push(FunctionalCurve {
            patient_id,
            group,
            axis,
            values,
        });
    }
    Ok(FunctionalInput { curves, blocks })
}

fn read_max_t_patients(path: &Path) -> Result<MaxTInput, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "endpoint", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "endpoint", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,endpoint,value or patient_id,group,endpoint,value,block".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<MaxTCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if blocked && row.block.is_none() {
            return Err(CohortError::Input(format!(
                "patient {} is missing its Max-T block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting Max-T blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.endpoint.clone(), row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has duplicate endpoint {:?}",
                row.patient_id, row.endpoint
            )));
        }
    }
    let mut patients = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, endpoints)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        patients.push(PatientEndpointVector {
            patient_id,
            group,
            values: endpoints.values().copied().collect(),
            endpoints: endpoints.into_keys().collect(),
        });
    }
    Ok(MaxTInput { patients, blocks })
}

fn read_fingerprints(path: &Path) -> Result<FingerprintInput, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "feature", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "feature", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,feature,value or patient_id,group,feature,value,block".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<FingerprintCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if blocked && row.block.is_none() {
            return Err(CohortError::Input(format!(
                "patient {} is missing its fingerprint block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting fingerprint blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.feature.clone(), row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has duplicate feature {:?}",
                row.patient_id, row.feature
            )));
        }
    }
    let mut fingerprints = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, features)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        fingerprints.push(Fingerprint {
            patient_id,
            group,
            values: features.values().copied().collect(),
            features: features.into_keys().collect(),
        });
    }
    Ok(FingerprintInput {
        fingerprints,
        blocks,
    })
}

fn validate_input_file(path: &Path) -> Result<(), CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(CohortError::Input(format!(
            "input must be a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(format!(
            "input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"
        )));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct PermutationOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: DesignSummary,
    patients: PatientSummary,
    groups: GroupSummaries,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
    alternative: CliAlternative,
}

impl PermutationOutput {
    fn from_result(
        input: PathBuf,
        alternative: CliAlternative,
        result: PatientPermutationResult,
    ) -> Self {
        Self {
            format: "marklab.cohort_permutation",
            version: 1,
            input,
            design: DesignSummary {
                randomization_unit: "patient",
                blocked: result.blocked,
                block_count: result.block_count,
            },
            patients: PatientSummary {
                total: result.patient_count,
            },
            groups: GroupSummaries {
                group_a: GroupSummary {
                    label: result.group_a.label,
                    patient_count: result.group_a.patient_count,
                    mean: result.group_a.mean,
                },
                group_b: GroupSummary {
                    label: result.group_b.label,
                    patient_count: result.group_b.patient_count,
                    mean: result.group_b.mean,
                },
            },
            effect_group_a_minus_group_b: result.effect_group_a_minus_group_b,
            studentized_statistic: result.studentized_statistic,
            p_value: result.p_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
            alternative,
        }
    }
}

#[derive(Debug, Serialize)]
struct DesignSummary {
    randomization_unit: &'static str,
    blocked: bool,
    block_count: usize,
}

#[derive(Debug, Serialize)]
struct PatientSummary {
    total: usize,
}

#[derive(Debug, Serialize)]
struct GroupSummaries {
    group_a: GroupSummary,
    group_b: GroupSummary,
}

#[derive(Debug, Serialize)]
struct GroupSummary {
    label: String,
    patient_count: usize,
    mean: f64,
}

#[derive(Debug, Serialize)]
struct PermutationSummary {
    requested: usize,
    attempted: usize,
    completed: usize,
}

#[derive(Debug, Serialize)]
struct PairedPermutationOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: PairedDesignSummary,
    pairs: PairSummary,
    conditions: ConditionSummaries,
    effect_condition_b_minus_condition_a: f64,
    studentized_statistic: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
    alternative: CliAlternative,
}

impl PairedPermutationOutput {
    fn from_result(
        input: PathBuf,
        alternative: CliAlternative,
        result: PairedPatientPermutationResult,
    ) -> Self {
        let null_family = match result.inference_design.null_family() {
            InferenceNullFamily::PairedSignFlip => "paired_sign_flip",
            _ => unreachable!("paired permutation returned another null family"),
        };
        let permutation_unit = match result.inference_design.permutation_unit() {
            InferencePermutationUnit::CompletePatientPairDifference => {
                "complete_patient_pair_difference"
            }
            _ => unreachable!("paired permutation returned another permutation unit"),
        };
        Self {
            format: "marklab.cohort_paired_permutation",
            version: 1,
            input,
            design: PairedDesignSummary {
                randomization_unit: "patient_pair",
                operation: "sign_flip",
                null_family,
                permutation_unit,
            },
            pairs: PairSummary {
                completed: result.pair_count,
            },
            conditions: ConditionSummaries {
                condition_a: ConditionSummary {
                    label: result.condition_a.label,
                    mean: result.condition_a.mean,
                },
                condition_b: ConditionSummary {
                    label: result.condition_b.label,
                    mean: result.condition_b.mean,
                },
            },
            effect_condition_b_minus_condition_a: result.effect_condition_b_minus_condition_a,
            studentized_statistic: result.studentized_statistic,
            p_value: result.p_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
            alternative,
        }
    }
}

#[derive(Debug, Serialize)]
struct PairedDesignSummary {
    randomization_unit: &'static str,
    operation: &'static str,
    null_family: &'static str,
    permutation_unit: &'static str,
}

#[derive(Debug, Serialize)]
struct PairSummary {
    completed: usize,
}

#[derive(Debug, Serialize)]
struct ConditionSummaries {
    condition_a: ConditionSummary,
    condition_b: ConditionSummary,
}

#[derive(Debug, Serialize)]
struct ConditionSummary {
    label: String,
    mean: f64,
}

#[derive(Debug, Serialize)]
struct FunctionalPermutationOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: PopulationDesignSummary,
    groups: FunctionalGroupSummary,
    axis: Vec<f64>,
    group_a_mean: Vec<f64>,
    group_b_mean: Vec<f64>,
    observed_difference: Vec<f64>,
    statistic: CliFunctionalStatistic,
    observed_statistic: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl FunctionalPermutationOutput {
    fn from_result(
        input: PathBuf,
        statistic: CliFunctionalStatistic,
        result: FunctionalPermutationResult,
    ) -> Self {
        Self {
            format: "marklab.cohort_functional_permutation",
            version: 1,
            input,
            design: PopulationDesignSummary {
                randomization_unit: "patient",
                blocked: false,
                null_family: None,
                block_count: None,
            },
            groups: FunctionalGroupSummary {
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
            },
            axis: result.axis,
            group_a_mean: result.group_a_mean,
            group_b_mean: result.group_b_mean,
            observed_difference: result.observed_difference,
            statistic,
            observed_statistic: result.observed_statistic,
            p_value: result.p_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
        }
    }

    fn from_blocked_result(
        input: PathBuf,
        statistic: CliFunctionalStatistic,
        blocked: BlockedFunctionalPermutationResult,
    ) -> Self {
        let (result, design) = blocked.into_parts();
        let mut output = Self::from_result(input, statistic, result);
        output.design = PopulationDesignSummary {
            randomization_unit: "patient",
            blocked: true,
            null_family: Some("population_independence"),
            block_count: Some(design.block_count()),
        };
        output
    }
}

#[derive(Debug, Serialize)]
struct PopulationDesignSummary {
    randomization_unit: &'static str,
    blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    null_family: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FunctionalGroupSummary {
    group_a_patients: usize,
    group_b_patients: usize,
}

#[derive(Debug, Serialize)]
struct MaxTOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: MaxTDesignSummary,
    groups: FunctionalGroupSummary,
    endpoints: Vec<MaxTEndpointOutput>,
    alpha: f64,
    critical_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl MaxTOutput {
    fn from_result(input: PathBuf, result: MaxTPermutationResult, blocked: bool) -> Self {
        Self {
            format: "marklab.cohort_max_t",
            version: 1,
            input,
            design: MaxTDesignSummary {
                randomization_unit: "patient",
                correction: "single_step_max_t",
                blocked: blocked.then_some(true),
                null_family: blocked.then_some("population_independence"),
                block_count: blocked.then_some(result.inference_design.block_count()),
            },
            groups: FunctionalGroupSummary {
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
            },
            endpoints: result
                .endpoints
                .into_iter()
                .map(|endpoint| MaxTEndpointOutput {
                    endpoint: endpoint.endpoint,
                    effect_group_a_minus_group_b: endpoint.effect_group_a_minus_group_b,
                    studentized_statistic: endpoint.studentized_statistic,
                    adjusted_p_value: endpoint.adjusted_p_value,
                })
                .collect(),
            alpha: result.alpha,
            critical_value: result.critical_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
        }
    }
}

#[derive(Debug, Serialize)]
struct MaxTDesignSummary {
    randomization_unit: &'static str,
    correction: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    null_family: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct MaxTEndpointOutput {
    endpoint: String,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    adjusted_p_value: f64,
}

#[derive(Debug, Serialize)]
struct MmdOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: PopulationDesignSummary,
    groups: FunctionalGroupSummary,
    feature_count: usize,
    kernel: MmdKernelOutput,
    estimator: CliMmdEstimator,
    mmd_squared: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl MmdOutput {
    fn from_result(
        input: PathBuf,
        estimator: CliMmdEstimator,
        result: MmdPermutationResult,
    ) -> Self {
        Self {
            format: "marklab.cohort_mmd",
            version: 1,
            input,
            design: PopulationDesignSummary {
                randomization_unit: "patient",
                blocked: false,
                null_family: None,
                block_count: None,
            },
            groups: FunctionalGroupSummary {
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
            },
            feature_count: result.feature_count,
            kernel: match result.kernel {
                MmdKernel::Linear => MmdKernelOutput {
                    kind: "linear",
                    bandwidth: None,
                },
                MmdKernel::Rbf { bandwidth } => MmdKernelOutput {
                    kind: "rbf",
                    bandwidth: Some(bandwidth),
                },
            },
            estimator,
            mmd_squared: result.mmd_squared,
            p_value: result.p_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
        }
    }

    fn from_blocked_result(
        input: PathBuf,
        estimator: CliMmdEstimator,
        blocked: BlockedMmdPermutationResult,
    ) -> Self {
        let (result, design) = blocked.into_parts();
        let mut output = Self::from_result(input, estimator, result);
        output.design = PopulationDesignSummary {
            randomization_unit: "patient",
            blocked: true,
            null_family: Some("population_independence"),
            block_count: Some(design.block_count()),
        };
        output
    }
}

#[derive(Debug, Serialize)]
struct MmdKernelOutput {
    kind: &'static str,
    bandwidth: Option<f64>,
}

#[derive(Debug, Serialize)]
struct EnergyOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: PopulationDesignSummary,
    groups: FunctionalGroupSummary,
    feature_count: usize,
    metric: CliEnergyMetric,
    energy_distance: f64,
    p_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl EnergyOutput {
    fn from_result(input: PathBuf, metric: CliEnergyMetric, result: EnergyDistanceResult) -> Self {
        Self {
            format: "marklab.cohort_energy",
            version: 1,
            input,
            design: PopulationDesignSummary {
                randomization_unit: "patient",
                blocked: false,
                null_family: None,
                block_count: None,
            },
            groups: FunctionalGroupSummary {
                group_a_patients: result.group_a_count,
                group_b_patients: result.group_b_count,
            },
            feature_count: result.feature_count,
            metric,
            energy_distance: result.energy_distance,
            p_value: result.p_value,
            permutations: PermutationSummary {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
            },
            seed: result.seed,
        }
    }

    fn from_blocked_result(
        input: PathBuf,
        metric: CliEnergyMetric,
        blocked: BlockedEnergyDistanceResult,
    ) -> Self {
        let (result, design) = blocked.into_parts();
        let mut output = Self::from_result(input, metric, result);
        output.design = PopulationDesignSummary {
            randomization_unit: "patient",
            blocked: true,
            null_family: Some("population_independence"),
            block_count: Some(design.block_count()),
        };
        output
    }
}
