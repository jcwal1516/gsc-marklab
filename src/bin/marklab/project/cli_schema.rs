//! Project command arguments; execution remains in the existing workflow adapters.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use super::{
    correlated_multitype_lgcp, gaussian_crossed_nested_hierarchy, joint_location_embedding,
    joint_location_mark, multitype_lgcp_sbc,
};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
pub(super) struct ProjectCli {
    #[command(subcommand)]
    pub(super) command: ProjectTopLevel,
}

#[derive(Debug, Subcommand)]
pub(super) enum ProjectTopLevel {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Debug, clap::Args)]
pub(super) struct ArbitraryWindowIppProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) events: PathBuf,
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    #[arg(long)]
    pub(super) window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept: f64,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient: f64,
    #[arg(long)]
    pub(super) maximum_events: usize,
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    #[arg(long)]
    pub(super) maximum_work: usize,
    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    pub(super) maximum_retained_bytes: usize,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ArbitraryWindowIppFitProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) events: PathBuf,
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    #[arg(long)]
    pub(super) window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient_prior_mean: f64,
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_events: usize,
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ArbitraryWindowIppSpatialPpcProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) events: PathBuf,
    #[arg(long)]
    pub(super) event_membership: PathBuf,
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    #[arg(long)]
    pub(super) window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient_prior_mean: f64,
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) prediction_seed: u64,
    #[arg(long)]
    pub(super) neighbor_radius_um: f64,
    #[arg(long)]
    pub(super) maximum_events: usize,
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    #[arg(long)]
    pub(super) maximum_neighbor_pairs: usize,
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ArbitraryWindowLgcpFitProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) events: PathBuf,
    #[arg(long)]
    pub(super) event_membership: PathBuf,
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    #[arg(long)]
    pub(super) window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient_prior_mean: f64,
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    #[arg(long)]
    pub(super) field_amplitude: f64,
    #[arg(long)]
    pub(super) field_length_scale_um: f64,
    #[arg(long)]
    pub(super) jitter: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_events: usize,
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    #[arg(long)]
    pub(super) prediction_replicates: u32,
    #[arg(long)]
    pub(super) prediction_seed: u64,
    #[arg(long)]
    pub(super) maximum_predictive_points: u64,
    #[arg(long)]
    pub(super) neighbor_radius_um: f64,
    #[arg(long)]
    pub(super) maximum_neighbor_pairs: usize,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ReplicatedArbitraryWindowLgcpFitProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) reference_group: String,
    #[arg(long)]
    pub(super) comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) covariate_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) field_amplitude: f64,
    #[arg(long)]
    pub(super) field_length_scale_um: f64,
    #[arg(long)]
    pub(super) jitter: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_patients: usize,
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    #[arg(long)]
    pub(super) maximum_nodes_per_pattern: usize,
    #[arg(long)]
    pub(super) maximum_total_nodes: usize,
    #[arg(long)]
    pub(super) maximum_total_events: u64,
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) reference_group: String,
    #[arg(long)]
    pub(super) comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) covariate_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) field_amplitude_prior_scale: f64,
    #[arg(long)]
    pub(super) field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    pub(super) jitter: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_patients: usize,
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    #[arg(long)]
    pub(super) maximum_nodes_per_pattern: usize,
    #[arg(long)]
    pub(super) maximum_total_nodes: usize,
    #[arg(long)]
    pub(super) maximum_total_events: u64,
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    #[arg(long)]
    pub(super) maximum_kernel_cube_work: u64,
    #[arg(long)]
    pub(super) maximum_tree_depth: u32,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ReplicatedArbitraryWindowMultitypeLgcpProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) reference_group: String,
    #[arg(long)]
    pub(super) comparison_group: String,
    #[arg(long)]
    pub(super) reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) covariate_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) field_amplitude: f64,
    #[arg(long)]
    pub(super) field_length_scale_um: f64,
    #[arg(long)]
    pub(super) jitter: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_patients: usize,
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    #[arg(long)]
    pub(super) maximum_types: usize,
    #[arg(long)]
    pub(super) maximum_nodes_per_pattern: usize,
    #[arg(long)]
    pub(super) maximum_total_nodes: usize,
    #[arg(long)]
    pub(super) maximum_total_node_type_rows: usize,
    #[arg(long)]
    pub(super) maximum_total_events: u64,
    #[arg(long)]
    pub(super) maximum_draw_node_type_work: u64,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) reference_group: String,
    #[arg(long)]
    pub(super) comparison_group: String,
    #[arg(long)]
    pub(super) reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) covariate_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) field_amplitude_prior_scale: f64,
    #[arg(long)]
    pub(super) field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    pub(super) jitter: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_patients: usize,
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    #[arg(long)]
    pub(super) maximum_types: usize,
    #[arg(long)]
    pub(super) maximum_nodes_per_pattern: usize,
    #[arg(long)]
    pub(super) maximum_total_nodes: usize,
    #[arg(long)]
    pub(super) maximum_total_node_type_rows: usize,
    #[arg(long)]
    pub(super) maximum_total_events: u64,
    #[arg(long)]
    pub(super) maximum_draw_node_type_work: u64,
    #[arg(long)]
    pub(super) maximum_kernel_cube_work: u64,
    #[arg(long)]
    pub(super) maximum_tree_depth: u32,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ConditionalMultitypeMarkProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) reference_type: String,
    #[arg(long)]
    pub(super) radius_um: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long)]
    pub(super) interaction_prior_sd: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_points: usize,
    #[arg(long)]
    pub(super) maximum_types: usize,
    #[arg(long)]
    pub(super) maximum_neighbor_visits: u64,
    #[arg(long)]
    pub(super) maximum_edges: usize,
    #[arg(long)]
    pub(super) maximum_draw_parameter_work: u64,
    #[arg(long)]
    pub(super) maximum_working_bytes: usize,
    #[arg(long)]
    pub(super) maximum_tree_depth: u32,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct SmcAbcGrowthFrontProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) diffusion_um2_per_time: f64,
    #[arg(long)]
    pub(super) carrying_capacity: f64,
    #[arg(long)]
    pub(super) final_time: f64,
    #[arg(long)]
    pub(super) time_step: f64,
    #[arg(long)]
    pub(super) front_threshold_fraction: f64,
    #[arg(long)]
    pub(super) observed_final_mass: f64,
    #[arg(long)]
    pub(super) mass_scale: f64,
    #[arg(long)]
    pub(super) growth_rate_prior_min: f64,
    #[arg(long)]
    pub(super) growth_rate_prior_max: f64,
    #[arg(long, value_delimiter = ',')]
    pub(super) epsilon_schedule: Vec<f64>,
    #[arg(long)]
    pub(super) particles: u32,
    #[arg(long)]
    pub(super) maximum_proposals_per_stage: u32,
    #[arg(long)]
    pub(super) maximum_cell_steps_per_proposal: u64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(super) struct ReplicatedConditionalMultitypeMarkProjectArgs {
    #[arg(long)]
    pub(super) project: PathBuf,
    #[arg(long)]
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) reference_group: String,
    #[arg(long)]
    pub(super) reference_type: String,
    #[arg(long)]
    pub(super) radius_um: f64,
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    #[arg(long)]
    pub(super) interaction_prior_sd: f64,
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(super) chains: u32,
    #[arg(long)]
    pub(super) tune: u32,
    #[arg(long)]
    pub(super) draws: u32,
    #[arg(long)]
    pub(super) target_accept: f64,
    #[arg(long)]
    pub(super) seed: u64,
    #[arg(long)]
    pub(super) maximum_patients: usize,
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    #[arg(long)]
    pub(super) maximum_points: usize,
    #[arg(long)]
    pub(super) maximum_types: usize,
    #[arg(long)]
    pub(super) maximum_neighbor_visits: u64,
    #[arg(long)]
    pub(super) maximum_edges: usize,
    #[arg(long)]
    pub(super) maximum_draw_parameter_work: u64,
    #[arg(long)]
    pub(super) maximum_working_bytes: usize,
    #[arg(long)]
    pub(super) maximum_tree_depth: u32,
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum ProjectNoninferiorityDirection {
    HigherIsBetter,
    LowerIsBetter,
}

#[derive(Debug, Subcommand)]
pub(super) enum ProjectCommand {
    TestCellPatchComplementarity {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        outer_folds: u32,
        #[arg(long)]
        inner_folds: u32,
        #[arg(long)]
        ridge_alphas: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortEnergy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        metric: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_features: usize,
        #[arg(long)]
        maximum_distance_elements: u64,
        #[arg(long)]
        maximum_energy_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CausalRandomizedInterference {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    GaussianEig {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CohortClusterCovariatePermutation {
        #[arg(long)]
        project: PathBuf,
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
        alternative: crate::cohort::CliAlternative,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_clusters: usize,
        #[arg(long)]
        maximum_covariates: usize,
        #[arg(long)]
        maximum_patient_covariate_cells: u64,
        #[arg(long)]
        maximum_ols_work: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortHierarchicalMaxT {
        #[arg(long)]
        project: PathBuf,
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
        step_down: bool,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_families: usize,
        #[arg(long)]
        maximum_endpoints: usize,
        #[arg(long)]
        maximum_cells: u64,
        #[arg(long)]
        maximum_permutation_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortHierarchicalBootstrap {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        replicates: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_specimens: usize,
        #[arg(long)]
        maximum_bootstrap_draws: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortEquivalence {
        #[arg(long)]
        project: PathBuf,
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
    CohortNoninferiority {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum)]
        direction: ProjectNoninferiorityDirection,
        #[arg(long)]
        margin: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
    CohortMaxT {
        #[arg(long)]
        project: PathBuf,
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
        step_down: bool,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_endpoints: usize,
        #[arg(long)]
        maximum_patient_endpoint_cells: u64,
        #[arg(long)]
        maximum_permutation_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    PatientNestedFields {
        #[arg(long)]
        project: PathBuf,
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
        project: PathBuf,
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
    CohortMmd {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        kernel: String,
        #[arg(long)]
        bandwidth: Option<f64>,
        #[arg(long)]
        estimator: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_features: usize,
        #[arg(long)]
        maximum_kernel_elements: u64,
        #[arg(long)]
        maximum_mmd_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortMultisiteCovariateContrast {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long, value_enum)]
        model: crate::cohort::CliMultisiteModel,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_sites: usize,
        #[arg(long)]
        maximum_covariates: usize,
        #[arg(long)]
        maximum_patient_covariate_cells: u64,
        #[arg(long)]
        maximum_ols_work: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    ProjectedEmbeddingVariograms {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        components: u32,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    VectorSemivariogram {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        weights: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
    },
    LocalMultivariateMoran {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        window: PathBuf,
        #[arg(long)]
        radius_um: f64,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_directed_edges: usize,
        #[arg(long)]
        maximum_permutation_edge_evaluations: usize,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    EmbeddingCrossCovarianceByDistance {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        maximum_matrix_elements: u64,
        #[arg(long)]
        memory_budget_mib: usize,
    },
    KernelMarkCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        kernel: String,
        #[arg(long)]
        global_reference_tolerance: f64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    EmbeddingSpatialDependenceEnvelope {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        curve: String,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    GraphDirichletEnergy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        laplacian: String,
        #[arg(long)]
        normalization: String,
        #[arg(long)]
        maximum_nodes: usize,
        #[arg(long)]
        maximum_edges: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    GraphSmoothnessPermutationTest {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        laplacian: String,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_nodes: usize,
        #[arg(long)]
        maximum_edges: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    LocalEmbeddingRoughness {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        maximum_nodes: usize,
        #[arg(long)]
        maximum_edges: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    LongitudinalKalmanSmooth {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MultiscaleEmbeddingKernel {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        weights: PathBuf,
        #[arg(long)]
        sample_a: String,
        #[arg(long)]
        sample_b: String,
        #[arg(long)]
        base_kernel: String,
        #[arg(long)]
        kernel_scale: Option<f64>,
        #[arg(long)]
        maximum_summaries: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_scale_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    RegionRetrieval {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        training: PathBuf,
        #[arg(long)]
        query: PathBuf,
        #[arg(long)]
        k: u32,
        #[arg(long)]
        leakage_policy: String,
        #[arg(long)]
        maximum_component_candidate_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MarkedPrepost {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        pre: PathBuf,
        #[arg(long)]
        post: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusHeat {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusBasis {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    GraphMotifTriangleSummary {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SmcAbcGrowthFront(Box<SmcAbcGrowthFrontProjectArgs>),
    SpatialVaryingCoefficient {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        global_predictor_name: String,
        #[arg(long)]
        spatial_predictor_name: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        amplitude_prior_sd: f64,
        #[arg(long)]
        length_scale_prior_sd_um: f64,
        #[arg(long)]
        known_noise_sd: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dKFunction {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dVoxelKFunction {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dRegisteredSerialVoxelK {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dRegisteredLongitudinalVoxelKChange {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusFourierEnergy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusHeatStability {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusDiffusionWavelet {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusScattering {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseCarFit {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    NegativeBinomialHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistence {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceStability {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceBottleneckStability {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdaptiveWindowSpde {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    NonstationaryAdaptiveWindowSpde {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ArbitraryWindowIppLikelihood(Box<ArbitraryWindowIppProjectArgs>),
    FitArbitraryWindowIpp(Box<ArbitraryWindowIppFitProjectArgs>),
    ArbitraryWindowIppSpatialPpc(Box<ArbitraryWindowIppSpatialPpcProjectArgs>),
    ArbitraryWindowLgcp(Box<ArbitraryWindowLgcpFitProjectArgs>),
    ReplicatedArbitraryWindowLgcp(Box<ReplicatedArbitraryWindowLgcpFitProjectArgs>),
    ReplicatedArbitraryWindowLgcpInferredKernel(
        Box<ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs>,
    ),
    ReplicatedArbitraryWindowMultitypeLgcp(Box<ReplicatedArbitraryWindowMultitypeLgcpProjectArgs>),
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernel(
        Box<ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectArgs>,
    ),
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(Box<multitype_lgcp_sbc::ProjectArgs>),
    CorrelatedReplicatedArbitraryWindowMultitypeLgcp(Box<correlated_multitype_lgcp::ProjectArgs>),
    JointReplicatedLocationEmbedding(Box<joint_location_embedding::ProjectArgs>),
    JointReplicatedLocationMark(Box<joint_location_mark::ProjectArgs>),
    GaussianCrossedNestedHierarchy(Box<gaussian_crossed_nested_hierarchy::ProjectArgs>),
    ConditionalMultitypeMark(Box<ConditionalMultitypeMarkProjectArgs>),
    ReplicatedConditionalMultitypeMark(Box<ReplicatedConditionalMultitypeMarkProjectArgs>),
    NormalMean {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalNormal {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    StudentTHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        observation_sd_prior: f64,
        #[arg(long)]
        degrees_of_freedom_excess_rate: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        population_alpha: f64,
        #[arg(long)]
        population_beta: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupRegression {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    DirichletMultinomialGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        logit_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    OrdinalGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        cutpoint_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    OrdinalGroupSiteHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        cutpoint_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        site_intercept_sd_prior_sd: f64,
        #[arg(long)]
        site_group_slope_sd_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    NonproportionalOrdinalGroupSite {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        cutpoint_prior_sd: f64,
        #[arg(long)]
        site_intercept_sd_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    OrdinalSiteHeldoutComparison {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        smoothing: f64,
        #[arg(long)]
        optimizer_tolerance: f64,
        #[arg(long)]
        maximum_optimizer_evaluations: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HurdleBetaBinomialGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long, allow_hyphen_values = true)]
        presence_intercept_prior_mean: f64,
        #[arg(long)]
        presence_intercept_prior_sd: f64,
        #[arg(long)]
        presence_group_effect_prior_sd: f64,
        #[arg(long, allow_hyphen_values = true)]
        abundance_intercept_prior_mean: f64,
        #[arg(long)]
        abundance_intercept_prior_sd: f64,
        #[arg(long)]
        abundance_group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupGenderRegression {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        reference_gender: String,
        #[arg(long)]
        comparison_gender: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        gender_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupGenderSlideHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        reference_gender: String,
        #[arg(long)]
        comparison_gender: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        gender_effect_prior_sd: f64,
        #[arg(long)]
        patient_log_odds_sd_prior_sd: f64,
        #[arg(long)]
        slide_concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GriddedLgcp {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        grid: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        grid_x: u32,
        #[arg(long)]
        grid_y: u32,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long, allow_hyphen_values = true)]
        coefficient_prior_mean: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        field_amplitude: f64,
        #[arg(long)]
        field_length_scale_um: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FusedGromovWasserstein {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        feature_scale: f64,
        #[arg(long)]
        structure_scale: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}
