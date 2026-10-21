use std::path::PathBuf;

use clap::{Args, Subcommand};
use marklab_bayes::NutsSamplingSpec;

mod anisotropic_gp3d;
mod arbitrary_window_ipp;
mod arbitrary_window_ipp_agreement;
mod arbitrary_window_ipp_fit;
mod arbitrary_window_ipp_membership;
mod arbitrary_window_ipp_quadrature_sensitivity;
mod arbitrary_window_ipp_sbc;
mod arbitrary_window_ipp_sensitivity;
mod arbitrary_window_ipp_spatial_ppc;
mod arbitrary_window_lgcp_agreement;
mod arbitrary_window_lgcp_fit;
mod arbitrary_window_lgcp_quadrature_sensitivity;
mod arbitrary_window_lgcp_sbc;
mod arbitrary_window_lgcp_sensitivity;
pub(super) use arbitrary_window_ipp::{
    execute as execute_arbitrary_window_ipp, prepare as prepare_arbitrary_window_ipp,
};
pub(super) use arbitrary_window_ipp_fit::{
    execute as execute_arbitrary_window_ipp_fit, prepare as prepare_arbitrary_window_ipp_fit,
    ArbitraryWindowIppFitResult, Parameters as ArbitraryWindowIppFitParameters,
};
pub(super) use arbitrary_window_lgcp_fit::{
    execute as execute_arbitrary_window_lgcp_fit, prepare as prepare_arbitrary_window_lgcp_fit,
    Arguments as ArbitraryWindowLgcpFitArgs, FitResult as ArbitraryWindowLgcpFitResult,
};

pub(crate) use arbitrary_window_ipp_spatial_ppc::{
    execute as execute_arbitrary_window_ipp_spatial_ppc,
    prepare as prepare_arbitrary_window_ipp_spatial_ppc,
    validate as validate_arbitrary_window_ipp_spatial_ppc, SpatialPpcParameters, SpatialPpcResult,
};
mod berman_turner;
mod conditional_multitype_mark;
mod conditional_multitype_mark_agreement;
mod conditional_multitype_mark_sbc;
mod conditional_multitype_mark_sensitivity;
mod replicated_conditional_multitype_mark;
mod replicated_conditional_multitype_mark_agreement;
mod replicated_conditional_multitype_mark_numpyro;
mod replicated_conditional_multitype_mark_sbc;
mod replicated_conditional_multitype_mark_sensitivity;
pub(super) use conditional_multitype_mark::{
    backend_contract as conditional_multitype_mark_backend_contract,
    execute as execute_conditional_multitype_mark, Args as ConditionalMultitypeMarkArgs,
    Output as ConditionalMultitypeMarkResult,
};
pub(super) use replicated_conditional_multitype_mark::{
    backend_contract as replicated_conditional_multitype_mark_backend_contract,
    execute as execute_replicated_conditional_multitype_mark,
    Args as ReplicatedConditionalMultitypeMarkArgs,
    Output as ReplicatedConditionalMultitypeMarkResult,
};
mod beta_binomial_hierarchy;
pub(super) use beta_binomial_hierarchy::{
    execute as execute_beta_binomial_hierarchy, prepare as prepare_beta_binomial_hierarchy,
};
mod beta_binomial_group_gender_regression;
pub(super) use beta_binomial_group_gender_regression::{
    execute as execute_beta_binomial_group_gender_regression,
    prepare as prepare_beta_binomial_group_gender_regression,
};
mod beta_binomial_group_gender_agreement;
mod beta_binomial_group_gender_sbc;
mod beta_binomial_group_gender_sensitivity;
mod beta_binomial_group_gender_slide_agreement;
mod beta_binomial_group_gender_slide_hierarchy;
mod beta_binomial_group_gender_slide_sbc;
mod beta_binomial_group_gender_slide_sensitivity;
pub(super) use beta_binomial_group_gender_slide_hierarchy::{
    execute as execute_beta_binomial_group_gender_slide_hierarchy,
    prepare as prepare_beta_binomial_group_gender_slide_hierarchy,
};
mod beta_binomial_group_regression;
pub(super) use beta_binomial_group_regression::{
    execute as execute_beta_binomial_group_regression,
    prepare as prepare_beta_binomial_group_regression,
};
mod beta_binomial_group_regression_agreement;
mod beta_binomial_group_regression_sbc;
mod beta_binomial_group_regression_sensitivity;
mod dirichlet_multinomial_group;
mod dirichlet_multinomial_group_agreement;
mod dirichlet_multinomial_group_sbc;
mod dirichlet_multinomial_group_sensitivity;
pub(super) use dirichlet_multinomial_group::{
    execute as execute_dirichlet_multinomial_group, prepare as prepare_dirichlet_multinomial_group,
};
mod beta_binomial_hierarchy_agreement;
mod beta_binomial_hierarchy_sbc;
mod beta_binomial_hierarchy_sensitivity;
mod bym;
mod bym2;
mod car;
pub(crate) mod complementarity;
mod cross_backend;
mod cross_modal;
mod distance_to_resource;
mod embedding_envelope;
mod embedding_factor;
mod embedding_kernel;
pub(crate) mod embedding_spatial;
pub(super) mod fused_gromov;
mod geyer;
mod gmrf;
mod gp;
mod graph_signal;
mod gridded_lgcp;
mod gridded_lgcp_agreement;
mod gridded_lgcp_fit;
pub(super) use gridded_lgcp_fit::{
    execute as execute_gridded_lgcp, prepare as prepare_gridded_lgcp,
};
mod gridded_lgcp_sbc;
mod gridded_lgcp_sensitivity;
mod gridded_lgcp_spatial_ppc;
mod grouped_conformal;
mod hierarchical;
pub(super) use hierarchical::{execute as execute_hierarchical, prepare as prepare_hierarchical};
mod hierarchical_agreement;
mod hierarchical_sbc;
mod hierarchical_sensitivity;
mod hurdle_beta_binomial_group;
mod inhomogeneous_poisson;
mod inhomogeneous_poisson_fit;
mod inla;
pub(crate) use crate::input_file::read_regular_file;
mod joint_mark;
mod laplace;
mod late_fusion;
mod matern_cluster;
mod meta_analysis;
mod mixture_of_experts;
mod model_comparison;
mod multi_output_gp;
mod multiscale_kernel;
mod multitype;
mod nngp;
mod partial_fused_gromov;
mod point_process_ppc;
mod posterior_predictive_lab;
mod prediction_calibration;
mod prediction_safety;
mod predictive_process;
mod predictive_stacking;
mod prior_sensitivity;
mod psis_loo;
mod rejection_abc;
mod replicated;
mod replicated_arbitrary_window_lgcp_agreement;
mod replicated_arbitrary_window_lgcp_fit;
mod replicated_arbitrary_window_lgcp_inferred_kernel;
mod replicated_arbitrary_window_lgcp_inferred_kernel_agreement;
mod replicated_arbitrary_window_lgcp_inferred_kernel_sbc;
mod unbalanced_fused_gromov;
pub(super) use replicated_arbitrary_window_lgcp_inferred_kernel::{
    execute as execute_replicated_arbitrary_window_lgcp_inferred_kernel,
    prepare as prepare_replicated_arbitrary_window_lgcp_inferred_kernel,
    Arguments as ReplicatedArbitraryWindowLgcpInferredKernelArgs,
    ResultDocument as ReplicatedArbitraryWindowLgcpInferredKernelResult,
};
pub(crate) mod correlated_replicated_arbitrary_window_multitype_lgcp;
pub(crate) mod joint_replicated_location_embedding;
pub(crate) mod joint_replicated_location_mark;
mod replicated_arbitrary_window_lgcp_sbc;
mod replicated_arbitrary_window_lgcp_sensitivity;
mod replicated_arbitrary_window_multitype_lgcp;
mod replicated_arbitrary_window_multitype_lgcp_agreement;
mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel;
mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement;
mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration;
pub(crate) mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc;
pub(super) use replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{
    execute as execute_replicated_arbitrary_window_multitype_lgcp_inferred_kernel,
    prepare as prepare_replicated_arbitrary_window_multitype_lgcp_inferred_kernel,
    Args as ReplicatedArbitraryWindowMultitypeLgcpInferredKernelArgs,
    Output as ReplicatedArbitraryWindowMultitypeLgcpInferredKernelOutput,
};
mod replicated_arbitrary_window_multitype_lgcp_numpyro;
mod replicated_arbitrary_window_multitype_lgcp_prior_calibration;
mod replicated_arbitrary_window_multitype_lgcp_sensitivity;
pub(super) use replicated_arbitrary_window_lgcp_fit::{
    execute as execute_replicated_arbitrary_window_lgcp_fit,
    prepare as prepare_replicated_arbitrary_window_lgcp_fit,
    Arguments as ReplicatedArbitraryWindowLgcpFitArgs,
    ResultDocument as ReplicatedArbitraryWindowLgcpFitResult,
};
pub(super) use replicated_arbitrary_window_multitype_lgcp::{
    execute_prepared as execute_replicated_arbitrary_window_multitype_lgcp,
    prepare as prepare_replicated_arbitrary_window_multitype_lgcp,
    Args as ReplicatedArbitraryWindowMultitypeLgcpArgs,
    Output as ReplicatedArbitraryWindowMultitypeLgcpResult,
};
pub(crate) mod negative_binomial_hierarchy;
pub(crate) mod retrieval;
mod sar;
mod sar_fit;
mod sbc;
mod sbi_sbc;
mod simulation_ood;
mod smc;
mod smc_abc;
pub(crate) mod sparse_car;
pub(crate) mod spatial_varying_coefficient;
mod strauss;
mod strauss_gibbs;
mod strauss_pseudolikelihood;
mod student_t_hierarchy;
pub(super) use student_t_hierarchy::{
    execute as execute_student_t_hierarchy, prepare as prepare_student_t_hierarchy,
};
mod student_t_hierarchy_agreement;
mod student_t_hierarchy_sbc;
mod student_t_hierarchy_sensitivity;
mod synthetic_likelihood;
mod thomas;
mod thomas_minimum_contrast;
mod transport;
mod variational_gp;
mod weights;

mod cli_schema;

/// No-U-Turn sampler settings shared by the MCMC commands.
#[derive(Clone, Debug, Args)]
pub(super) struct NutsArgs {
    /// Number of independent Markov chains.
    #[arg(long)]
    pub(super) chains: u32,
    /// Warm-up iterations per chain, used for step-size and mass-matrix adaptation and then
    /// discarded.
    #[arg(long)]
    pub(super) tune: u32,
    /// Posterior draws kept per chain.
    #[arg(long)]
    pub(super) draws: u32,
    /// Target acceptance probability for step-size adaptation, between 0 and 1.
    #[arg(long)]
    pub(super) target_accept: f64,
    /// Seed for the sampler's random number generator.
    #[arg(long)]
    pub(super) seed: u64,
}

impl NutsArgs {
    pub(super) fn spec(self) -> NutsSamplingSpec {
        NutsSamplingSpec {
            chains: self.chains,
            tune_per_chain: self.tune,
            draws_per_chain: self.draws,
            target_accept: self.target_accept,
            seed: self.seed,
        }
    }
}
pub(crate) mod gaussian_crossed_nested_hierarchy;
mod nonproportional_ordinal_group_site;
mod normal_mean;
mod ordinal_group;
mod ordinal_group_site_hierarchy;
mod ordinal_site_heldout;
mod worker_process;

pub(crate) use crate::cli_error::{into_marklab_error, CliError};

/// Keep migrated scientific validators in their original CLI error categories.
pub(crate) fn protocol_error(error: marklab_bayes::BayesError) -> CliError {
    match error {
        marklab_bayes::BayesError::InvalidSpec(message) => CliError::Input(message),
        marklab_bayes::BayesError::WorkerContract(message) => CliError::Backend(message),
        marklab_bayes::BayesError::Json(error) => CliError::Json(error),
        error => CliError::Model(error),
    }
}
pub(super) use hurdle_beta_binomial_group::{
    execute as execute_hurdle_beta_binomial_group, prepare as prepare_hurdle_beta_binomial_group,
};
pub(super) use nonproportional_ordinal_group_site::{
    execute as execute_nonproportional_ordinal_group_site,
    prepare as prepare_nonproportional_ordinal_group_site,
};
pub(super) use normal_mean::{execute_normal_mean, prepare_normal_mean};
pub(super) use ordinal_group::{
    execute as execute_ordinal_group, prepare as prepare_ordinal_group,
};
pub(super) use ordinal_group_site_hierarchy::{
    execute as execute_ordinal_group_site_hierarchy,
    prepare as prepare_ordinal_group_site_hierarchy,
};
pub(super) use ordinal_site_heldout::{
    execute as execute_ordinal_site_heldout, prepare as prepare_ordinal_site_heldout,
};
pub(super) use worker_process::{publish_json, run_worker, worker_identity};

use normal_mean::{observations_digest, read_observations};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) use cli_schema::{
    execute, BayesCommand, BetaBinomialGroupGenderRegressionModelArgs,
    BetaBinomialGroupGenderSlideHierarchyModelArgs, BetaBinomialGroupRegressionModelArgs,
    BetaBinomialHierarchyModelArgs, DirichletMultinomialGroupModelArgs, GriddedLgcpModelArgs,
    HierarchicalNormalModelArgs, HurdleBetaBinomialGroupModelArgs,
    NonproportionalOrdinalGroupSiteModelArgs, OrdinalGroupModelArgs,
    OrdinalGroupSiteHierarchyModelArgs, OrdinalSiteHeldoutComparisonModelArgs,
    StudentTHierarchyModelArgs, TestCellPatchComplementarityArgs,
};
