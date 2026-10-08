use std::path::PathBuf;

use marklab_cohort::{
    BlockedEnergyDistanceResult, BlockedFunctionalPermutationResult, BlockedMmdPermutationResult,
    EnergyDistanceResult, FunctionalPermutationResult, InferenceNullFamily,
    InferencePermutationUnit, MaxTPermutationResult, MmdKernel, MmdPermutationResult,
    PairedMaxTPermutationResult, PairedPatientPermutationResult, PatientPermutationResult,
};
use serde::Serialize;

use super::{CliAlternative, CliEnergyMetric, CliFunctionalStatistic, CliMmdEstimator};

#[derive(Debug, Serialize)]
pub(super) struct PermutationOutput {
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
    pub(super) fn from_result(
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
pub(super) struct PairedPermutationOutput {
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
    pub(super) fn from_result(
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
pub(super) struct PairedMaxTOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    design: PairedMaxTDesignSummary,
    pairs: PairSummary,
    conditions: PairedMaxTConditionLabels,
    endpoints: Vec<PairedMaxTEndpointOutput>,
    alpha: f64,
    critical_value: f64,
    permutations: PermutationSummary,
    seed: u64,
}

impl PairedMaxTOutput {
    pub(super) fn from_result(input: PathBuf, result: PairedMaxTPermutationResult) -> Self {
        match result.inference_design.null_family() {
            InferenceNullFamily::PairedSignFlip => {}
            _ => unreachable!("paired Max-T returned another null family"),
        }
        match result.inference_design.permutation_unit() {
            InferencePermutationUnit::CompletePatientPairDifferenceVector => {}
            _ => unreachable!("paired Max-T returned another permutation unit"),
        }
        Self {
            format: "marklab.cohort_paired_max_t",
            version: 1,
            input,
            design: PairedMaxTDesignSummary {
                randomization_unit: "patient_pair",
                null_family: "paired_sign_flip",
                permutation_unit: "complete_patient_pair_difference_vector",
                multiplicity: "complete_endpoint_family_max_t",
                correction: result.correction.as_str(),
            },
            pairs: PairSummary {
                completed: result.pair_count,
            },
            conditions: PairedMaxTConditionLabels {
                condition_a: result.condition_a,
                condition_b: result.condition_b,
            },
            endpoints: result
                .endpoints
                .into_iter()
                .map(|endpoint| PairedMaxTEndpointOutput {
                    endpoint: endpoint.endpoint,
                    effect_condition_b_minus_condition_a: endpoint
                        .effect_condition_b_minus_condition_a,
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
struct PairedMaxTDesignSummary {
    randomization_unit: &'static str,
    null_family: &'static str,
    permutation_unit: &'static str,
    multiplicity: &'static str,
    correction: &'static str,
}

#[derive(Debug, Serialize)]
struct PairedMaxTConditionLabels {
    condition_a: String,
    condition_b: String,
}

#[derive(Debug, Serialize)]
struct PairedMaxTEndpointOutput {
    endpoint: String,
    effect_condition_b_minus_condition_a: f64,
    studentized_statistic: f64,
    adjusted_p_value: f64,
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
pub(super) struct FunctionalPermutationOutput {
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
    pub(super) fn from_result(
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

    pub(super) fn from_blocked_result(
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
pub(super) struct MaxTOutput {
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
    pub(super) fn from_result(
        input: PathBuf,
        result: MaxTPermutationResult,
        blocked: bool,
    ) -> Self {
        Self {
            format: "marklab.cohort_max_t",
            version: 1,
            input,
            design: MaxTDesignSummary {
                randomization_unit: "patient",
                correction: result.correction.as_str(),
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
pub(super) struct MmdOutput {
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
    pub(super) fn from_result(
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

    pub(super) fn from_blocked_result(
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
pub(super) struct EnergyOutput {
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
    pub(super) fn from_result(
        input: PathBuf,
        metric: CliEnergyMetric,
        result: EnergyDistanceResult,
    ) -> Self {
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

    pub(super) fn from_blocked_result(
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
