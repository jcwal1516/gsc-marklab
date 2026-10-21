use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
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

mod cluster;
pub(crate) mod cluster_covariate;
mod covariate;
mod covariate_matrix;
mod effects;
pub(crate) mod equivalence;
mod fingerprint;
mod functional_equivalence;
pub(crate) mod hierarchical_bootstrap;
mod hierarchical_max_t;
mod input;
pub(crate) mod max_t_calibration;
pub(crate) mod multisite;
pub(crate) mod noninferiority;
mod output;
pub(crate) mod patient_nested_fields;
mod publication;
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

#[derive(Debug, Subcommand)]
pub(crate) enum CohortCommand {
    /// Test a patient-level group contrast by label permutation.
    Permutation(PermutationArgs),
    /// Test within-patient condition differences by permutation.
    PairedPermutation(PairedPermutationArgs),
    /// Test paired endpoints with max-T multiplicity correction.
    PairedMaxT(PairedMaxTArgs),
    /// Test a group effect after adjusting for a covariate.
    CovariatePermutation(covariate::RunArgs),
    /// Test a group effect after adjustment for multiple covariates.
    CovariateMatrixPermutation(covariate_matrix::RunArgs),
    /// Permute whole clusters to preserve within-cluster dependence.
    ClusterPermutation(ClusterPermutationArgs),
    /// Test covariate-adjusted group effects with cluster-level permutations.
    ClusterCovariatePermutation(cluster_covariate::RunArgs),
    /// Test repeated-measure effects with restricted residual permutations.
    RepeatedFreedmanLane(RepeatedFreedmanLaneArgs),
    /// Estimate group effects while accounting for study sites.
    MultisiteInference(MultisiteInferenceArgs),
    /// Estimate patient-level group contrasts across study sites.
    MultisitePatientContrast(MultisitePatientContrastArgs),
    /// Estimate site-aware group contrasts with covariate adjustment.
    MultisiteCovariateContrast(MultisiteCovariateContrastArgs),
    /// Compare patient-level curves by permutation.
    FunctionalPermutation(FunctionalPermutationArgs),
    /// Test whether patient-level curves lie within declared equivalence margins.
    FunctionalEquivalence(FunctionalEquivalenceArgs),
    /// Test multiple patient-level endpoints with max-T correction.
    MaxT(MaxTArgs),
    /// Test nested patient endpoints with hierarchical max-T correction.
    HierarchicalMaxT(hierarchical_max_t::RunArgs),
    /// Compare multivariate patient distributions with a kernel MMD test.
    Mmd(MmdArgs),
    /// Compare multivariate patient distributions with an energy-distance test.
    Energy(EnergyArgs),
    /// Measure distance between declared cohort fingerprints.
    FingerprintDistance(FingerprintDistanceArgs),
    /// Compare two regions using their declared summaries and support.
    RegionCompatibility(RegionCompatibilityArgs),
    /// Test whether a group contrast lies within prespecified equivalence margins.
    Equivalence(EquivalenceArgs),
    /// Test a group contrast against a prespecified noninferiority margin.
    Noninferiority(NoninferiorityArgs),
    /// Bootstrap patients and their nested specimens.
    HierarchicalBootstrap(HierarchicalBootstrapArgs),
    /// Assess equivalence using a bootstrap interval for a group contrast.
    BootstrapEquivalence(BootstrapEquivalenceArgs),
    /// Reduce nested spatial fields to patient-level endpoint inference.
    PatientNestedFields(patient_nested_fields::RunArgs),
    /// Evaluate max-T calibration on declared synthetic endpoint families.
    MaxTCalibration(max_t_calibration::RunArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct PermutationArgs {
    /// CSV with patient_id, group and endpoint columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Direction of the alternative hypothesis.
    #[arg(long, value_enum)]
    pub(super) alternative: CliAlternative,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PairedMaxTArgs {
    /// CSV with patient_id, condition, endpoint and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// First within-patient condition label.
    #[arg(long)]
    pub(super) condition_a: String,
    /// Second within-patient condition label.
    #[arg(long)]
    pub(super) condition_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Apply step-down rather than single-step Max-T adjustment.
    #[arg(long)]
    pub(super) step_down: bool,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MaxTArgs {
    /// CSV with patient_id, group, endpoint and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Apply step-down rather than single-step Max-T adjustment.
    #[arg(long)]
    pub(super) step_down: bool,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MmdArgs {
    /// CSV with patient_id, group, feature and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Kernel used to compare observations.
    #[arg(long, value_enum)]
    pub(super) kernel: CliMmdKernel,
    /// Bandwidth of the kernel used to compare observations.
    #[arg(long)]
    pub(super) bandwidth: Option<f64>,
    /// Estimator used for the kernel discrepancy between groups.
    #[arg(long, value_enum)]
    pub(super) estimator: CliMmdEstimator,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct EnergyArgs {
    /// CSV with patient_id, group, feature and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Distance metric used to compare observations.
    #[arg(long, value_enum)]
    pub(super) metric: CliEnergyMetric,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct FingerprintDistanceArgs {
    /// CSV with sample_id, component, axis, value, uncertainty and weight columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Identifier of the first sample in the comparison.
    #[arg(long)]
    pub(super) left_sample: String,
    /// Identifier of the second sample in the comparison.
    #[arg(long)]
    pub(super) right_sample: String,
    /// Version of the declared analysis specification.
    #[arg(long)]
    pub(super) spec_version: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct RegionCompatibilityArgs {
    /// CSV with sample_id, component, axis, value, uncertainty and weight columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Identifier of the first sample in the comparison.
    #[arg(long)]
    pub(super) left_sample: String,
    /// Identifier of the second sample in the comparison.
    #[arg(long)]
    pub(super) right_sample: String,
    /// Version of the declared analysis specification.
    #[arg(long)]
    pub(super) spec_version: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct EquivalenceArgs {
    /// CSV with patient_id and effect columns, one effect per patient.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Lower prespecified equivalence margin.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) lower_margin: f64,
    /// Upper prespecified equivalence margin.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) upper_margin: f64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Scientific rationale for the declared equivalence or noninferiority margin.
    #[arg(long)]
    pub(super) margin_rationale: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct NoninferiorityArgs {
    /// CSV with patient_id and effect columns, one effect per patient.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Direction in which the noninferiority margin is interpreted.
    #[arg(long, value_enum)]
    pub(super) direction: noninferiority::CliDirection,
    /// Prespecified noninferiority margin.
    #[arg(long)]
    pub(super) margin: f64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Scientific rationale for the declared equivalence or noninferiority margin.
    #[arg(long)]
    pub(super) margin_rationale: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct HierarchicalBootstrapArgs {
    /// CSV with patient_id, specimen_id and endpoint columns, one row per specimen.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Number of bootstrap resamples used for the interval estimate.
    #[arg(long)]
    pub(super) replicates: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct BootstrapEquivalenceArgs {
    /// CSV with patient_id, specimen_id and endpoint columns, one row per specimen.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Lower prespecified equivalence margin.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) lower_margin: f64,
    /// Upper prespecified equivalence margin.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) upper_margin: f64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Number of bootstrap resamples used for the interval estimate.
    #[arg(long)]
    pub(super) replicates: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Scientific rationale for the declared equivalence or noninferiority margin.
    #[arg(long)]
    pub(super) margin_rationale: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct FunctionalPermutationArgs {
    /// CSV with patient_id, group, axis and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Functional contrast statistic used for the permutation test.
    #[arg(long, value_enum)]
    pub(super) statistic: CliFunctionalStatistic,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct FunctionalEquivalenceArgs {
    /// CSV with patient_id, axis, difference and margin columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Number of bootstrap resamples used for the interval estimate.
    #[arg(long)]
    pub(super) replicates: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Scientific rationale for the declared equivalence or noninferiority margin.
    #[arg(long)]
    pub(super) margin_rationale: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PairedPermutationArgs {
    /// CSV with patient_id, condition and endpoint columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// First within-patient condition label.
    #[arg(long)]
    pub(super) condition_a: String,
    /// Second within-patient condition label.
    #[arg(long)]
    pub(super) condition_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Direction of the alternative hypothesis.
    #[arg(long, value_enum)]
    pub(super) alternative: CliAlternative,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ClusterPermutationArgs {
    /// CSV with patient_id, cluster_id, group and endpoint columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Direction of the alternative hypothesis.
    #[arg(long, value_enum)]
    pub(super) alternative: CliAlternative,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct RepeatedFreedmanLaneArgs {
    /// CSV with subject_id, visit_id, outcome and target columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MultisiteInferenceArgs {
    /// CSV with site_id, effect, standard_error and patient_count columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Model variant used for the declared spatial or multisite analysis.
    #[arg(long, value_enum)]
    pub(super) model: CliMultisiteModel,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MultisitePatientContrastArgs {
    /// CSV with patient_id, site_id, group and endpoint columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Model variant used for the declared spatial or multisite analysis.
    #[arg(long, value_enum)]
    pub(super) model: CliMultisiteModel,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MultisiteCovariateContrastArgs {
    /// CSV with patient_id, site_id, group, endpoint, covariate and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Model variant used for the declared spatial or multisite analysis.
    #[arg(long, value_enum)]
    pub(super) model: CliMultisiteModel,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
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
pub(crate) enum CliFunctionalStatistic {
    L2,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum CliMmdKernel {
    Linear,
    Rbf,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CliMmdEstimator {
    Unbiased,
    Biased,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CliEnergyMetric {
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

pub(crate) fn run_cli(command: CohortCommand) -> Result<(), CohortError> {
    match command {
        CohortCommand::MaxTCalibration(args) => max_t_calibration::run(args),
        CohortCommand::PatientNestedFields(args) => patient_nested_fields::run(args),
        CohortCommand::Permutation(args) => run_permutation(args),
        CohortCommand::PairedMaxT(args) => run_paired_max_t(args),
        CohortCommand::CovariatePermutation(args) => covariate::run(args),
        CohortCommand::CovariateMatrixPermutation(args) => covariate_matrix::run(args),
        CohortCommand::MaxT(args) => run_max_t(args),
        CohortCommand::HierarchicalMaxT(args) => hierarchical_max_t::run(args),
        CohortCommand::Mmd(args) => run_mmd(args),
        CohortCommand::Energy(args) => run_energy(args),
        CohortCommand::FingerprintDistance(args) => fingerprint::run(args),
        CohortCommand::RegionCompatibility(args) => fingerprint::run_compatibility(args),
        CohortCommand::Equivalence(args) => equivalence::run(args),
        CohortCommand::Noninferiority(args) => noninferiority::run(args),
        CohortCommand::HierarchicalBootstrap(args) => hierarchical_bootstrap::run(args),
        CohortCommand::BootstrapEquivalence(args) => hierarchical_bootstrap::run_equivalence(args),
        CohortCommand::FunctionalPermutation(args) => run_functional_permutation(args),
        CohortCommand::FunctionalEquivalence(args) => functional_equivalence::run(args),
        CohortCommand::PairedPermutation(args) => run_paired_permutation(args),
        CohortCommand::ClusterPermutation(args) => cluster::run(args),
        CohortCommand::ClusterCovariatePermutation(args) => cluster_covariate::run(args),
        CohortCommand::RepeatedFreedmanLane(args) => repeated::run(args),
        CohortCommand::MultisiteInference(args) => multisite::run(args),
        CohortCommand::MultisitePatientContrast(args) => multisite::run_patient_contrast(args),
        CohortCommand::MultisiteCovariateContrast(args) => multisite::run_covariate_contrast(args),
    }
}

pub(crate) fn execute(command: CohortCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}

fn run_permutation(args: PermutationArgs) -> Result<(), CohortError> {
    let PermutationArgs {
        input,
        group_a,
        group_b,
        permutations,
        seed,
        alternative,
        out,
    } = args;
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

fn run_paired_max_t(args: PairedMaxTArgs) -> Result<(), CohortError> {
    let PairedMaxTArgs {
        input,
        condition_a,
        condition_b,
        permutations,
        seed,
        alpha,
        step_down,
        out,
    } = args;
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

fn run_max_t(args: MaxTArgs) -> Result<(), CohortError> {
    let MaxTArgs {
        input,
        group_a,
        group_b,
        permutations,
        seed,
        alpha,
        step_down,
        out,
    } = args;
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
            max_t_multiple_endpoint_blocked_permutation(&max_t_input.patients, &blocks, &spec)?,
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

fn run_mmd(args: MmdArgs) -> Result<(), CohortError> {
    let MmdArgs {
        input,
        group_a,
        group_b,
        kernel,
        bandwidth,
        estimator,
        permutations,
        seed,
        out,
    } = args;
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

fn run_energy(args: EnergyArgs) -> Result<(), CohortError> {
    let EnergyArgs {
        input,
        group_a,
        group_b,
        metric,
        permutations,
        seed,
        out,
    } = args;
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
            patient_level_blocked_energy_distance(&fingerprint_input.fingerprints, &blocks, &spec)?,
        ),
        None => EnergyOutput::from_result(
            input,
            metric,
            patient_level_energy_distance(&fingerprint_input.fingerprints, &spec)?,
        ),
    };
    publish_json(&out, &output)
}

fn run_functional_permutation(args: FunctionalPermutationArgs) -> Result<(), CohortError> {
    let FunctionalPermutationArgs {
        input,
        group_a,
        group_b,
        statistic,
        permutations,
        seed,
        out,
    } = args;
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
            functional_two_sample_blocked_permutation(&functional_input.curves, &blocks, &spec)?,
        ),
        None => FunctionalPermutationOutput::from_result(
            input,
            statistic,
            functional_two_sample_permutation(&functional_input.curves, &spec)?,
        ),
    };
    publish_json(&out, &output)
}

fn run_paired_permutation(args: PairedPermutationArgs) -> Result<(), CohortError> {
    let PairedPermutationArgs {
        input,
        condition_a,
        condition_b,
        permutations,
        seed,
        alternative,
        out,
    } = args;
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
