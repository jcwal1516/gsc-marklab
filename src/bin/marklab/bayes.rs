use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use marklab_bayes::{NutsSamplingSpec, SmcSamplingSpec};

#[path = "bayes/anisotropic_gp3d.rs"]
mod anisotropic_gp3d;
#[path = "bayes/arbitrary_window_ipp.rs"]
mod arbitrary_window_ipp;
#[path = "bayes/arbitrary_window_ipp_agreement.rs"]
mod arbitrary_window_ipp_agreement;
#[path = "bayes/arbitrary_window_ipp_fit.rs"]
mod arbitrary_window_ipp_fit;
#[path = "bayes/arbitrary_window_ipp_membership.rs"]
mod arbitrary_window_ipp_membership;
#[path = "bayes/arbitrary_window_ipp_quadrature_sensitivity.rs"]
mod arbitrary_window_ipp_quadrature_sensitivity;
#[path = "bayes/arbitrary_window_ipp_sbc.rs"]
mod arbitrary_window_ipp_sbc;
#[path = "bayes/arbitrary_window_ipp_sensitivity.rs"]
mod arbitrary_window_ipp_sensitivity;
#[path = "bayes/arbitrary_window_ipp_spatial_ppc.rs"]
mod arbitrary_window_ipp_spatial_ppc;
#[path = "bayes/arbitrary_window_lgcp_agreement.rs"]
mod arbitrary_window_lgcp_agreement;
#[path = "bayes/arbitrary_window_lgcp_fit.rs"]
mod arbitrary_window_lgcp_fit;
#[path = "bayes/arbitrary_window_lgcp_quadrature_sensitivity.rs"]
mod arbitrary_window_lgcp_quadrature_sensitivity;
#[path = "bayes/arbitrary_window_lgcp_sbc.rs"]
mod arbitrary_window_lgcp_sbc;
#[path = "bayes/arbitrary_window_lgcp_sensitivity.rs"]
mod arbitrary_window_lgcp_sensitivity;
pub(super) use arbitrary_window_ipp::{
    execute as execute_arbitrary_window_ipp, prepare as prepare_arbitrary_window_ipp,
    PreparedArbitraryWindowIpp,
};
pub(super) use arbitrary_window_ipp_fit::{
    execute as execute_arbitrary_window_ipp_fit, prepare as prepare_arbitrary_window_ipp_fit,
    ArbitraryWindowIppFitResult, PreparedArbitraryWindowIppFit,
};
pub(super) use arbitrary_window_lgcp_fit::{
    execute as execute_arbitrary_window_lgcp_fit, prepare as prepare_arbitrary_window_lgcp_fit,
    FitResult as ArbitraryWindowLgcpFitResult, PreparedArbitraryWindowLgcpFit,
};

pub(super) fn run_arbitrary_window_ipp_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp::run_cli()
}

pub(super) fn run_arbitrary_window_lgcp_fit_cli() -> Result<(), BayesCliError> {
    arbitrary_window_lgcp_fit::run_cli()
}

pub(super) fn run_arbitrary_window_lgcp_agreement_cli() -> Result<(), BayesCliError> {
    arbitrary_window_lgcp_agreement::run_cli()
}

pub(super) fn run_arbitrary_window_lgcp_quadrature_sensitivity_cli() -> Result<(), BayesCliError> {
    arbitrary_window_lgcp_quadrature_sensitivity::run_cli()
}

pub(super) fn run_arbitrary_window_lgcp_sbc_cli() -> Result<(), BayesCliError> {
    arbitrary_window_lgcp_sbc::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_fit_cli() -> Result<(), BayesCliError> {
    replicated_arbitrary_window_lgcp_fit::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_inferred_kernel_cli() -> Result<(), BayesCliError>
{
    replicated_arbitrary_window_lgcp_inferred_kernel::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_inferred_kernel_agreement_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_lgcp_inferred_kernel_agreement::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_lgcp_inferred_kernel_sbc::run_cli()
}

pub(super) fn run_conditional_multitype_mark_cli() -> Result<(), BayesCliError> {
    conditional_multitype_mark::run_cli()
}

pub(super) fn run_conditional_multitype_mark_agreement_cli() -> Result<(), BayesCliError> {
    conditional_multitype_mark_agreement::run_cli()
}

pub(super) fn run_conditional_multitype_mark_sensitivity_cli() -> Result<(), BayesCliError> {
    conditional_multitype_mark_sensitivity::run_cli()
}

pub(super) fn run_conditional_multitype_mark_sbc_cli() -> Result<(), BayesCliError> {
    conditional_multitype_mark_sbc::run_cli()
}

pub(super) fn run_replicated_conditional_multitype_mark_cli() -> Result<(), BayesCliError> {
    replicated_conditional_multitype_mark::run_cli()
}

pub(super) fn run_replicated_conditional_multitype_mark_agreement_cli() -> Result<(), BayesCliError>
{
    replicated_conditional_multitype_mark_agreement::run_cli()
}

pub(super) fn run_replicated_conditional_multitype_mark_sensitivity_cli(
) -> Result<(), BayesCliError> {
    replicated_conditional_multitype_mark_sensitivity::run_cli()
}

pub(super) fn run_replicated_conditional_multitype_mark_sbc_cli() -> Result<(), BayesCliError> {
    replicated_conditional_multitype_mark_sbc::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_agreement_cli() -> Result<(), BayesCliError> {
    replicated_arbitrary_window_lgcp_agreement::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_cli() -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_agreement_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_agreement::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::run_cli()
}

pub(super) fn run_correlated_replicated_arbitrary_window_multitype_lgcp_cli(
) -> Result<(), BayesCliError> {
    correlated_replicated_arbitrary_window_multitype_lgcp::run_cli()
}

pub(super) fn run_joint_replicated_location_mark_cli() -> Result<(), BayesCliError> {
    joint_replicated_location_mark::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_prior_calibration_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_prior_calibration::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_sensitivity_cli(
) -> Result<(), BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp_sensitivity::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_sensitivity_cli() -> Result<(), BayesCliError> {
    replicated_arbitrary_window_lgcp_sensitivity::run_cli()
}

pub(super) fn run_replicated_arbitrary_window_lgcp_sbc_cli() -> Result<(), BayesCliError> {
    replicated_arbitrary_window_lgcp_sbc::run_cli()
}

pub(super) fn run_arbitrary_window_lgcp_sensitivity_cli() -> Result<(), BayesCliError> {
    arbitrary_window_lgcp_sensitivity::run_cli()
}

pub(super) fn run_arbitrary_window_ipp_fit_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp_fit::run_cli()
}

pub(super) fn run_arbitrary_window_ipp_agreement_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp_agreement::run_cli()
}

pub(super) fn run_arbitrary_window_ipp_sensitivity_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp_sensitivity::run_cli()
}

pub(super) fn run_arbitrary_window_ipp_quadrature_sensitivity_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp_quadrature_sensitivity::run_cli()
}

pub(super) fn run_arbitrary_window_ipp_sbc_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp_sbc::run_cli()
}

pub(super) fn run_arbitrary_window_ipp_spatial_ppc_cli() -> Result<(), BayesCliError> {
    arbitrary_window_ipp_spatial_ppc::run_cli()
}
pub(crate) use arbitrary_window_ipp_spatial_ppc::{
    execute as execute_arbitrary_window_ipp_spatial_ppc,
    prepare as prepare_arbitrary_window_ipp_spatial_ppc,
    validate as validate_arbitrary_window_ipp_spatial_ppc, PreparedSpatialPpc,
    SpatialPpcParameters, SpatialPpcResult,
};
#[path = "bayes/berman_turner.rs"]
mod berman_turner;
#[path = "bayes/conditional_multitype_mark.rs"]
mod conditional_multitype_mark;
#[path = "bayes/conditional_multitype_mark_agreement.rs"]
mod conditional_multitype_mark_agreement;
#[path = "bayes/conditional_multitype_mark_sbc.rs"]
mod conditional_multitype_mark_sbc;
#[path = "bayes/conditional_multitype_mark_sensitivity.rs"]
mod conditional_multitype_mark_sensitivity;
#[path = "bayes/replicated_conditional_multitype_mark.rs"]
mod replicated_conditional_multitype_mark;
#[path = "bayes/replicated_conditional_multitype_mark_agreement.rs"]
mod replicated_conditional_multitype_mark_agreement;
#[path = "bayes/replicated_conditional_multitype_mark_numpyro.rs"]
mod replicated_conditional_multitype_mark_numpyro;
#[path = "bayes/replicated_conditional_multitype_mark_sbc.rs"]
mod replicated_conditional_multitype_mark_sbc;
#[path = "bayes/replicated_conditional_multitype_mark_sensitivity.rs"]
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
#[path = "bayes/beta_binomial_hierarchy.rs"]
mod beta_binomial_hierarchy;
pub(super) use beta_binomial_hierarchy::{
    execute as execute_beta_binomial_hierarchy, prepare as prepare_beta_binomial_hierarchy,
    PreparedBetaBinomialHierarchy,
};
#[path = "bayes/beta_binomial_group_gender_regression.rs"]
mod beta_binomial_group_gender_regression;
pub(super) use beta_binomial_group_gender_regression::{
    execute as execute_beta_binomial_group_gender_regression,
    prepare as prepare_beta_binomial_group_gender_regression,
    PreparedBetaBinomialGroupGenderRegression,
};
#[path = "bayes/beta_binomial_group_gender_agreement.rs"]
mod beta_binomial_group_gender_agreement;
#[path = "bayes/beta_binomial_group_gender_sbc.rs"]
mod beta_binomial_group_gender_sbc;
#[path = "bayes/beta_binomial_group_gender_sensitivity.rs"]
mod beta_binomial_group_gender_sensitivity;
#[path = "bayes/beta_binomial_group_gender_slide_agreement.rs"]
mod beta_binomial_group_gender_slide_agreement;
#[path = "bayes/beta_binomial_group_gender_slide_hierarchy.rs"]
mod beta_binomial_group_gender_slide_hierarchy;
#[path = "bayes/beta_binomial_group_gender_slide_sbc.rs"]
mod beta_binomial_group_gender_slide_sbc;
#[path = "bayes/beta_binomial_group_gender_slide_sensitivity.rs"]
mod beta_binomial_group_gender_slide_sensitivity;
pub(super) use beta_binomial_group_gender_slide_hierarchy::{
    execute as execute_beta_binomial_group_gender_slide_hierarchy,
    prepare as prepare_beta_binomial_group_gender_slide_hierarchy,
    PreparedBetaBinomialGroupGenderSlideHierarchy,
};
#[path = "bayes/beta_binomial_group_regression.rs"]
mod beta_binomial_group_regression;
pub(super) use beta_binomial_group_regression::{
    execute as execute_beta_binomial_group_regression,
    prepare as prepare_beta_binomial_group_regression, PreparedBetaBinomialGroupRegression,
};
#[path = "bayes/beta_binomial_group_regression_agreement.rs"]
mod beta_binomial_group_regression_agreement;
#[path = "bayes/beta_binomial_group_regression_sbc.rs"]
mod beta_binomial_group_regression_sbc;
#[path = "bayes/beta_binomial_group_regression_sensitivity.rs"]
mod beta_binomial_group_regression_sensitivity;
#[path = "bayes/dirichlet_multinomial_group.rs"]
mod dirichlet_multinomial_group;
#[path = "bayes/dirichlet_multinomial_group_agreement.rs"]
mod dirichlet_multinomial_group_agreement;
#[path = "bayes/dirichlet_multinomial_group_sbc.rs"]
mod dirichlet_multinomial_group_sbc;
#[path = "bayes/dirichlet_multinomial_group_sensitivity.rs"]
mod dirichlet_multinomial_group_sensitivity;
pub(super) use dirichlet_multinomial_group::{
    execute as execute_dirichlet_multinomial_group, prepare as prepare_dirichlet_multinomial_group,
    PreparedDirichletMultinomialGroup,
};
#[path = "bayes/beta_binomial_hierarchy_agreement.rs"]
mod beta_binomial_hierarchy_agreement;
#[path = "bayes/beta_binomial_hierarchy_sbc.rs"]
mod beta_binomial_hierarchy_sbc;
#[path = "bayes/beta_binomial_hierarchy_sensitivity.rs"]
mod beta_binomial_hierarchy_sensitivity;
#[path = "bayes/bym.rs"]
mod bym;
#[path = "bayes/bym2.rs"]
mod bym2;
#[path = "bayes/car.rs"]
mod car;
#[path = "bayes/complementarity.rs"]
pub(crate) mod complementarity;
#[path = "bayes/cross_modal.rs"]
mod cross_modal;
#[path = "bayes/distance_to_resource.rs"]
mod distance_to_resource;
#[path = "bayes/embedding_envelope.rs"]
mod embedding_envelope;
#[path = "bayes/embedding_factor.rs"]
mod embedding_factor;
#[path = "bayes/embedding_kernel.rs"]
mod embedding_kernel;
#[path = "bayes/embedding_spatial.rs"]
pub(crate) mod embedding_spatial;
#[path = "bayes/fused_gromov.rs"]
pub(super) mod fused_gromov;
#[path = "bayes/geyer.rs"]
mod geyer;
#[path = "bayes/gmrf.rs"]
mod gmrf;
#[path = "bayes/gp.rs"]
mod gp;
#[path = "bayes/graph_signal.rs"]
mod graph_signal;
#[path = "bayes/gridded_lgcp.rs"]
mod gridded_lgcp;
#[path = "bayes/gridded_lgcp_agreement.rs"]
mod gridded_lgcp_agreement;
#[path = "bayes/gridded_lgcp_fit.rs"]
mod gridded_lgcp_fit;
pub(super) use gridded_lgcp_fit::{
    execute as execute_gridded_lgcp, prepare as prepare_gridded_lgcp, PreparedGriddedLgcpFit,
};
#[path = "bayes/gridded_lgcp_sbc.rs"]
mod gridded_lgcp_sbc;
#[path = "bayes/gridded_lgcp_sensitivity.rs"]
mod gridded_lgcp_sensitivity;
#[path = "bayes/gridded_lgcp_spatial_ppc.rs"]
mod gridded_lgcp_spatial_ppc;
#[path = "bayes/grouped_conformal.rs"]
mod grouped_conformal;
#[path = "bayes/hierarchical.rs"]
mod hierarchical;
pub(super) use hierarchical::{
    execute as execute_hierarchical, prepare as prepare_hierarchical, PreparedGaussianHierarchy,
};
#[path = "bayes/hierarchical_agreement.rs"]
mod hierarchical_agreement;
#[path = "bayes/hierarchical_sbc.rs"]
mod hierarchical_sbc;
#[path = "bayes/hierarchical_sensitivity.rs"]
mod hierarchical_sensitivity;
#[path = "bayes/inhomogeneous_poisson.rs"]
mod inhomogeneous_poisson;
#[path = "bayes/inhomogeneous_poisson_fit.rs"]
mod inhomogeneous_poisson_fit;
#[path = "bayes/inla.rs"]
mod inla;
#[path = "bayes/input_file.rs"]
mod input_file;
#[path = "bayes/joint_mark.rs"]
mod joint_mark;
#[path = "bayes/laplace.rs"]
mod laplace;
#[path = "bayes/late_fusion.rs"]
mod late_fusion;
#[path = "bayes/matern_cluster.rs"]
mod matern_cluster;
#[path = "bayes/meta_analysis.rs"]
mod meta_analysis;
#[path = "bayes/mixture_of_experts.rs"]
mod mixture_of_experts;
#[path = "bayes/model_comparison.rs"]
mod model_comparison;
#[path = "bayes/multi_output_gp.rs"]
mod multi_output_gp;
#[path = "bayes/multiscale_kernel.rs"]
mod multiscale_kernel;
#[path = "bayes/multitype.rs"]
mod multitype;
#[path = "bayes/nngp.rs"]
mod nngp;
#[path = "bayes/partial_fused_gromov.rs"]
mod partial_fused_gromov;
#[path = "bayes/point_process_ppc.rs"]
mod point_process_ppc;
#[path = "bayes/posterior_predictive_lab.rs"]
mod posterior_predictive_lab;
#[path = "bayes/posterior_validation.rs"]
mod posterior_validation;
#[path = "bayes/prediction_calibration.rs"]
mod prediction_calibration;
#[path = "bayes/prediction_safety.rs"]
mod prediction_safety;
#[path = "bayes/predictive_process.rs"]
mod predictive_process;
#[path = "bayes/predictive_stacking.rs"]
mod predictive_stacking;
#[path = "bayes/prior_sensitivity.rs"]
mod prior_sensitivity;
#[path = "bayes/psis_loo.rs"]
mod psis_loo;
#[path = "bayes/rejection_abc.rs"]
mod rejection_abc;
#[path = "bayes/replicated.rs"]
mod replicated;
#[path = "bayes/replicated_arbitrary_window_lgcp_agreement.rs"]
mod replicated_arbitrary_window_lgcp_agreement;
#[path = "bayes/replicated_arbitrary_window_lgcp_fit.rs"]
mod replicated_arbitrary_window_lgcp_fit;
#[path = "bayes/replicated_arbitrary_window_lgcp_inferred_kernel.rs"]
mod replicated_arbitrary_window_lgcp_inferred_kernel;
#[path = "bayes/replicated_arbitrary_window_lgcp_inferred_kernel_agreement.rs"]
mod replicated_arbitrary_window_lgcp_inferred_kernel_agreement;
#[path = "bayes/replicated_arbitrary_window_lgcp_inferred_kernel_sbc.rs"]
mod replicated_arbitrary_window_lgcp_inferred_kernel_sbc;
pub(super) use replicated_arbitrary_window_lgcp_inferred_kernel::{
    execute as execute_replicated_arbitrary_window_lgcp_inferred_kernel,
    prepare as prepare_replicated_arbitrary_window_lgcp_inferred_kernel,
    PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    ResultDocument as ReplicatedArbitraryWindowLgcpInferredKernelResult,
};
#[path = "bayes/correlated_replicated_arbitrary_window_multitype_lgcp.rs"]
pub(crate) mod correlated_replicated_arbitrary_window_multitype_lgcp;
#[path = "bayes/joint_replicated_location_mark.rs"]
pub(crate) mod joint_replicated_location_mark;
#[path = "bayes/replicated_arbitrary_window_lgcp_sbc.rs"]
mod replicated_arbitrary_window_lgcp_sbc;
#[path = "bayes/replicated_arbitrary_window_lgcp_sensitivity.rs"]
mod replicated_arbitrary_window_lgcp_sensitivity;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp.rs"]
mod replicated_arbitrary_window_multitype_lgcp;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_agreement.rs"]
mod replicated_arbitrary_window_multitype_lgcp_agreement;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_inferred_kernel.rs"]
mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement.rs"]
mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration.rs"]
mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc.rs"]
pub(crate) mod replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc;
pub(super) use replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{
    execute as execute_replicated_arbitrary_window_multitype_lgcp_inferred_kernel,
    prepare as prepare_replicated_arbitrary_window_multitype_lgcp_inferred_kernel,
    Args as ReplicatedArbitraryWindowMultitypeLgcpInferredKernelArgs,
    Output as ReplicatedArbitraryWindowMultitypeLgcpInferredKernelOutput,
    Prepared as PreparedReplicatedArbitraryWindowMultitypeLgcpInferredKernel,
};
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_numpyro.rs"]
mod replicated_arbitrary_window_multitype_lgcp_numpyro;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_prior_calibration.rs"]
mod replicated_arbitrary_window_multitype_lgcp_prior_calibration;
#[path = "bayes/replicated_arbitrary_window_multitype_lgcp_sensitivity.rs"]
mod replicated_arbitrary_window_multitype_lgcp_sensitivity;
pub(super) use replicated_arbitrary_window_lgcp_fit::{
    execute as execute_replicated_arbitrary_window_lgcp_fit,
    prepare as prepare_replicated_arbitrary_window_lgcp_fit,
    PreparedReplicatedArbitraryWindowLgcpFit,
    ResultDocument as ReplicatedArbitraryWindowLgcpFitResult,
};
pub(super) use replicated_arbitrary_window_multitype_lgcp::{
    execute_prepared as execute_replicated_arbitrary_window_multitype_lgcp,
    prepare as prepare_replicated_arbitrary_window_multitype_lgcp,
    Args as ReplicatedArbitraryWindowMultitypeLgcpArgs,
    Output as ReplicatedArbitraryWindowMultitypeLgcpResult,
    Prepared as PreparedReplicatedArbitraryWindowMultitypeLgcp,
};
#[path = "bayes/retrieval.rs"]
pub(crate) mod retrieval;
#[path = "bayes/sar.rs"]
mod sar;
#[path = "bayes/sar_fit.rs"]
mod sar_fit;
#[path = "bayes/sbc.rs"]
mod sbc;
#[path = "bayes/sbi_sbc.rs"]
mod sbi_sbc;
#[path = "bayes/simulation_ood.rs"]
mod simulation_ood;
#[path = "bayes/smc.rs"]
mod smc;
#[path = "bayes/smc_abc.rs"]
mod smc_abc;
#[path = "bayes/spatial_varying_coefficient.rs"]
pub(crate) mod spatial_varying_coefficient;
#[path = "bayes/strauss.rs"]
mod strauss;
#[path = "bayes/strauss_gibbs.rs"]
mod strauss_gibbs;
#[path = "bayes/strauss_pseudolikelihood.rs"]
mod strauss_pseudolikelihood;
#[path = "bayes/student_t_hierarchy.rs"]
mod student_t_hierarchy;
pub(super) use student_t_hierarchy::{
    execute as execute_student_t_hierarchy, prepare as prepare_student_t_hierarchy,
    PreparedStudentTHierarchy,
};
#[path = "bayes/student_t_hierarchy_agreement.rs"]
mod student_t_hierarchy_agreement;
#[path = "bayes/student_t_hierarchy_sbc.rs"]
mod student_t_hierarchy_sbc;
#[path = "bayes/student_t_hierarchy_sensitivity.rs"]
mod student_t_hierarchy_sensitivity;
#[path = "bayes/synthetic_likelihood.rs"]
mod synthetic_likelihood;
#[path = "bayes/thomas.rs"]
mod thomas;
#[path = "bayes/thomas_minimum_contrast.rs"]
mod thomas_minimum_contrast;
#[path = "bayes/transport.rs"]
mod transport;
#[path = "bayes/variational_gp.rs"]
mod variational_gp;
#[path = "bayes/weights.rs"]
mod weights;

#[path = "bayes/cli_error.rs"]
mod cli_error;
#[path = "bayes/cli_schema.rs"]
mod cli_schema;
#[path = "bayes/normal_mean.rs"]
mod normal_mean;
#[path = "bayes/worker_process.rs"]
mod worker_process;

pub(super) use cli_error::{into_marklab_error, BayesCliError};
pub(super) use cli_schema::{
    run_beta_binomial_group_gender_slide_hierarchy_agreement_cli,
    run_beta_binomial_group_gender_slide_hierarchy_sbc_cli,
    run_beta_binomial_group_gender_slide_hierarchy_sensitivity_cli, run_cli,
    run_dirichlet_multinomial_group_agreement_cli, run_dirichlet_multinomial_group_cli,
    run_dirichlet_multinomial_group_sbc_cli, run_dirichlet_multinomial_group_sensitivity_cli,
};
pub(super) use normal_mean::{execute_normal_mean, prepare_normal_mean, PreparedNormalMean};
pub(super) use worker_process::{publish_json, run_worker};

use normal_mean::{observations_digest, read_observations, run_normal_mean};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
