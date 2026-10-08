use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use marklab_cohort::{
    functional_two_sample_blocked_permutation, functional_two_sample_permutation,
    max_t_multiple_endpoint_blocked_permutation,
    max_t_multiple_endpoint_blocked_step_down_permutation, max_t_multiple_endpoint_permutation,
    max_t_multiple_endpoint_step_down_permutation, paired_max_t_permutation,
    paired_patient_permutation_test, patient_level_blocked_energy_distance,
    patient_level_blocked_mmd, patient_level_energy_distance, patient_level_mmd,
    patient_level_permutation_test, CohortInferenceError, EnergyDistanceSpec, EnergyMetric,
    FunctionalPermutationSpec, FunctionalTestStatistic, MaxTPermutationSpec, MmdEstimator,
    MmdKernel, MmdPermutationSpec, PairedMaxTPermutationSpec, PairedPatientPermutationSpec,
    PatientPermutationSpec, PermutationAlternative,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[path = "cohort/cluster.rs"]
mod cluster;
#[path = "cohort/cluster_covariate.rs"]
pub(crate) mod cluster_covariate;
#[path = "cohort/covariate.rs"]
mod covariate;
#[path = "cohort/covariate_matrix.rs"]
mod covariate_matrix;
#[path = "cohort/effects.rs"]
mod effects;
#[path = "cohort/equivalence.rs"]
pub(crate) mod equivalence;
#[path = "cohort/fingerprint.rs"]
mod fingerprint;
#[path = "cohort/functional_equivalence.rs"]
mod functional_equivalence;
#[path = "cohort/hierarchical_bootstrap.rs"]
pub(crate) mod hierarchical_bootstrap;
#[path = "cohort/hierarchical_max_t.rs"]
mod hierarchical_max_t;
#[path = "cohort/input.rs"]
mod input;
#[path = "cohort/max_t_calibration.rs"]
pub(crate) mod max_t_calibration;
#[path = "cohort/multisite.rs"]
pub(crate) mod multisite;
#[path = "cohort/noninferiority.rs"]
pub(crate) mod noninferiority;
#[path = "cohort/output.rs"]
mod output;
#[path = "cohort/patient_nested_fields.rs"]
pub(crate) mod patient_nested_fields;
#[path = "cohort/publication.rs"]
mod publication;
#[path = "cohort/repeated.rs"]
mod repeated;

use input::{
    read_fingerprints, read_functional_curves, read_max_t_patients, read_paired_max_t_records,
    read_paired_records, read_records, validate_input_file,
};
use output::{
    EnergyOutput, FunctionalPermutationOutput, MaxTOutput, MmdOutput, PairedMaxTOutput,
    PairedPermutationOutput, PermutationOutput,
};
use publication::publish_json;

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
    PairedMaxT {
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
        #[arg(long)]
        alpha: f64,
        /// Apply step-down rather than single-step Max-T adjustment.
        #[arg(long)]
        step_down: bool,
        #[arg(long)]
        out: PathBuf,
    },
    CovariatePermutation {
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
    CovariateMatrixPermutation {
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
    ClusterCovariatePermutation {
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
    MultisiteCovariateContrast {
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
        /// Apply step-down rather than single-step Max-T adjustment.
        #[arg(long)]
        step_down: bool,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalMaxT {
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
        /// Apply step-down rather than single-step Max-T within each opened family.
        #[arg(long)]
        step_down: bool,
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
    PatientNestedFields {
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
        maximum_patients: usize,
        #[arg(long)]
        maximum_specimens: usize,
        #[arg(long)]
        maximum_endpoints: usize,
        #[arg(long)]
        maximum_permutation_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    MaxTCalibration {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a_count: usize,
        #[arg(long, value_delimiter = ',')]
        family_sizes: Vec<usize>,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_assignments: usize,
        #[arg(long)]
        maximum_assignment_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CliAlternative {
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliMultisiteModel {
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
                CohortCommand::MaxTCalibration {
                    input,
                    group_a_count,
                    family_sizes,
                    alpha,
                    maximum_assignments,
                    maximum_assignment_endpoint_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => max_t_calibration::run(max_t_calibration::RunArgs {
            input,
            group_a_count,
            family_sizes,
            alpha,
            maximum_assignments,
            maximum_assignment_endpoint_evaluations,
            memory_budget_mib,
            out,
        }),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::PatientNestedFields {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    maximum_patients,
                    maximum_specimens,
                    maximum_endpoints,
                    maximum_permutation_endpoint_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => patient_nested_fields::run(patient_nested_fields::RunArgs {
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
            maximum_patients,
            maximum_specimens,
            maximum_endpoints,
            maximum_permutation_endpoint_evaluations,
            memory_budget_mib,
            out,
        }),
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
                CohortCommand::PairedMaxT {
                    input,
                    condition_a,
                    condition_b,
                    permutations,
                    seed,
                    alpha,
                    step_down,
                    out,
                },
        } => {
            let records = read_paired_max_t_records(&input)?;
            let result = paired_max_t_permutation(
                &records,
                &PairedMaxTPermutationSpec {
                    condition_a,
                    condition_b,
                    permutations,
                    seed,
                    alpha,
                    correction: if step_down {
                        marklab_cohort::MaxTCorrection::StepDown
                    } else {
                        marklab_cohort::MaxTCorrection::SingleStep
                    },
                },
            )?;
            publish_json(&out, &PairedMaxTOutput::from_result(input, result))
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::CovariatePermutation {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative,
                    out,
                },
        } => covariate::run(covariate::RunArgs {
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alternative,
            out,
        }),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::CovariateMatrixPermutation {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative,
                    out,
                },
        } => covariate_matrix::run(covariate_matrix::RunArgs {
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alternative,
            out,
        }),
        CohortTopLevel::Cohort {
            command:
                CohortCommand::MaxT {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    step_down,
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
            let (result, blocked) = match (max_t_input.blocks, step_down) {
                (Some(blocks), true) => (
                    max_t_multiple_endpoint_blocked_step_down_permutation(
                        &max_t_input.patients,
                        &blocks,
                        &spec,
                    )?,
                    true,
                ),
                (Some(blocks), false) => (
                    max_t_multiple_endpoint_blocked_permutation(
                        &max_t_input.patients,
                        &blocks,
                        &spec,
                    )?,
                    true,
                ),
                (None, true) => (
                    max_t_multiple_endpoint_step_down_permutation(&max_t_input.patients, &spec)?,
                    false,
                ),
                (None, false) => (
                    max_t_multiple_endpoint_permutation(&max_t_input.patients, &spec)?,
                    false,
                ),
            };
            publish_json(&out, &MaxTOutput::from_result(input, result, blocked))
        }
        CohortTopLevel::Cohort {
            command:
                CohortCommand::HierarchicalMaxT {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    step_down,
                    out,
                },
        } => hierarchical_max_t::run(hierarchical_max_t::RunArgs {
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
            step_down,
            out,
        }),
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
                CohortCommand::ClusterCovariatePermutation {
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative,
                    out,
                },
        } => cluster_covariate::run(cluster_covariate::RunArgs {
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alternative,
            out,
        }),
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
        CohortTopLevel::Cohort {
            command:
                CohortCommand::MultisiteCovariateContrast {
                    input,
                    group_a,
                    group_b,
                    model,
                    alpha,
                    out,
                },
        } => multisite::run_covariate_contrast(input, group_a, group_b, model, alpha, out),
    }
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<CohortCli>(|| run_cli().map_err(into_marklab_error))
}
