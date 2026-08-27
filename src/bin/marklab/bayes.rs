use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BayesError, NormalMeanInputIdentity, NormalMeanSpec, NormalMeanWorkerRequest,
    NutsSamplingSpec, SmcSamplingSpec, WorkerResult,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[path = "bayes/anisotropic_gp3d.rs"]
mod anisotropic_gp3d;
#[path = "bayes/berman_turner.rs"]
mod berman_turner;
#[path = "bayes/beta_binomial_hierarchy.rs"]
mod beta_binomial_hierarchy;
pub(super) use beta_binomial_hierarchy::{
    execute as execute_beta_binomial_hierarchy, prepare as prepare_beta_binomial_hierarchy,
    PreparedBetaBinomialHierarchy,
};
#[path = "bayes/beta_binomial_group_regression.rs"]
mod beta_binomial_group_regression;
pub(super) use beta_binomial_group_regression::{
    execute as execute_beta_binomial_group_regression,
    prepare as prepare_beta_binomial_group_regression, PreparedBetaBinomialGroupRegression,
};
#[path = "bayes/beta_binomial_group_regression_agreement.rs"]
mod beta_binomial_group_regression_agreement;
#[path = "bayes/beta_binomial_group_regression_sensitivity.rs"]
mod beta_binomial_group_regression_sensitivity;
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
mod embedding_spatial;
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
#[path = "bayes/retrieval.rs"]
mod retrieval;
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
mod spatial_varying_coefficient;
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

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAXIMUM_WORKER_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct BayesCli {
    #[command(subcommand)]
    command: BayesTopLevel,
}

#[derive(Debug, Subcommand)]
enum BayesTopLevel {
    Bayes {
        #[command(subcommand)]
        command: BayesCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BayesCommand {
    NormalMean {
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
    BetaBinomialHierarchy {
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
    BetaBinomialHierarchyAgreement {
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
        maximum_standardized_difference: f64,
        #[arg(long)]
        minimum_probability_tolerance: f64,
        #[arg(long)]
        minimum_concentration_tolerance: f64,
        #[arg(long)]
        minimum_patient_tolerance: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialHierarchySensitivity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        population_alpha: f64,
        #[arg(long)]
        population_beta: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        lower_scale_multiplier: f64,
        #[arg(long)]
        upper_scale_multiplier: f64,
        #[arg(long)]
        material_standardized_shift: f64,
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
    BetaBinomialHierarchySbc {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        population_alpha: f64,
        #[arg(long)]
        population_beta: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        replicates: u32,
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
        minimum_rank_uniformity_p_value: f64,
        #[arg(long)]
        minimum_coverage_90: f64,
        #[arg(long)]
        maximum_coverage_90: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupRegression {
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
    BetaBinomialGroupRegressionAgreement {
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
        maximum_standardized_difference: f64,
        #[arg(long)]
        minimum_probability_tolerance: f64,
        #[arg(long)]
        minimum_log_odds_tolerance: f64,
        #[arg(long)]
        minimum_concentration_tolerance: f64,
        #[arg(long)]
        minimum_patient_tolerance: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupRegressionSensitivity {
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
        lower_scale_multiplier: f64,
        #[arg(long)]
        upper_scale_multiplier: f64,
        #[arg(long)]
        material_standardized_shift: f64,
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
    StudentTHierarchyAgreement {
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
        maximum_standardized_difference: f64,
        #[arg(long)]
        minimum_location_scale_tolerance: f64,
        #[arg(long)]
        minimum_degrees_of_freedom_tolerance: f64,
        #[arg(long)]
        minimum_patient_tolerance: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    StudentTHierarchySensitivity {
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
        lower_scale_multiplier: f64,
        #[arg(long)]
        upper_scale_multiplier: f64,
        #[arg(long)]
        material_standardized_shift: f64,
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
    StudentTHierarchySbc {
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
        replicates: u32,
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
        minimum_rank_uniformity_p_value: f64,
        #[arg(long)]
        minimum_coverage_90: f64,
        #[arg(long)]
        maximum_coverage_90: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalNormal {
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
    HierarchicalNormalAgreement {
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
        maximum_standardized_difference: f64,
        #[arg(long)]
        minimum_absolute_tolerance: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalNormalPriorSensitivity {
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
        lower_scale_multiplier: f64,
        #[arg(long)]
        upper_scale_multiplier: f64,
        #[arg(long)]
        material_standardized_shift: f64,
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
    HierarchicalNormalSbc {
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        patients: u32,
        #[arg(long)]
        observations_per_patient: u32,
        #[arg(long)]
        replicates: u32,
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
        minimum_rank_uniformity_p_value: f64,
        #[arg(long)]
        minimum_coverage_90: f64,
        #[arg(long)]
        maximum_coverage_90: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MetaAnalysis {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        covariate_name: String,
        #[arg(long, allow_hyphen_values = true)]
        new_site_covariate: f64,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        covariate_prior_sd: f64,
        #[arg(long)]
        heterogeneity_prior_sd: f64,
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
    GpRegression {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        predict: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        mean_prior_mean: f64,
        #[arg(long)]
        mean_prior_sd: f64,
        #[arg(long)]
        amplitude_prior_sd: f64,
        #[arg(long)]
        length_scale_prior_sd_um: f64,
        #[arg(long)]
        noise_prior_sd: f64,
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
    #[command(name = "anisotropic-gp-3d")]
    AnisotropicGp3d {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        predict: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        mean_prior_mean: f64,
        #[arg(long)]
        mean_prior_sd: f64,
        #[arg(long)]
        amplitude_prior_sd: f64,
        #[arg(long)]
        length_scale_x_prior_sd_um: f64,
        #[arg(long)]
        length_scale_y_prior_sd_um: f64,
        #[arg(long)]
        length_scale_z_prior_sd_um: f64,
        #[arg(long)]
        noise_prior_sd: f64,
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
    MultiOutputGp {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        predict: PathBuf,
        #[arg(long)]
        output_a_name: String,
        #[arg(long)]
        output_b_name: String,
        #[arg(long)]
        mean_prior_sd: f64,
        #[arg(long)]
        amplitude_prior_sd: f64,
        #[arg(long)]
        length_scale_prior_sd_um: f64,
        #[arg(long)]
        loading_b_prior_sd: f64,
        #[arg(long)]
        noise_a_sd: f64,
        #[arg(long)]
        noise_b_sd: f64,
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
    VariationalGp {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        predict: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        mean_prior_mean: f64,
        #[arg(long)]
        mean_prior_sd: f64,
        #[arg(long)]
        amplitude_prior_sd: f64,
        #[arg(long)]
        length_scale_prior_sd_um: f64,
        #[arg(long)]
        noise_prior_sd: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        inducing_points: u32,
        #[arg(long)]
        starts: u32,
        #[arg(long)]
        iterations: u32,
        #[arg(long)]
        learning_rate: f64,
        #[arg(long)]
        posterior_draws: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    PredictiveProcess {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        knots: PathBuf,
        #[arg(long)]
        amplitude: f64,
        #[arg(long)]
        length_scale_um: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        diagonal_correction: bool,
        #[arg(long)]
        out: PathBuf,
    },
    NngpDensity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        mean: f64,
        #[arg(long)]
        amplitude: f64,
        #[arg(long)]
        length_scale_um: f64,
        #[arg(long)]
        neighbors: usize,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        variance_tolerance: f64,
        #[arg(long)]
        full_reference: bool,
        #[arg(long)]
        out: PathBuf,
    },
    ValidateWeights {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long, value_enum)]
        symmetry: weights::CliSymmetry,
        #[arg(long, value_enum)]
        diagonal: weights::CliDiagonal,
        #[arg(long, value_enum)]
        normalization: weights::CliNormalization,
        #[arg(long)]
        out: PathBuf,
    },
    CarDensity {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        field: PathBuf,
        #[arg(long, value_enum)]
        mode: car::CliCarMode,
        #[arg(long)]
        tau: f64,
        #[arg(long, allow_hyphen_values = true)]
        rho: f64,
        #[arg(long)]
        constraint_tolerance: f64,
        #[arg(long, value_enum)]
        island_policy: car::CliIslandPolicy,
        #[arg(long)]
        out: PathBuf,
    },
    GmrfDensity {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        precision: PathBuf,
        #[arg(long)]
        field: PathBuf,
        #[arg(long)]
        constraints: PathBuf,
        #[arg(long)]
        constraint_tolerance: f64,
        #[arg(long)]
        out: PathBuf,
    },
    SarLikelihood {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        coefficients: PathBuf,
        #[arg(long, value_enum)]
        model: sar::CliSarModel,
        #[arg(long, allow_hyphen_values = true)]
        rho: f64,
        #[arg(long)]
        sigma: f64,
        #[arg(long, value_enum)]
        interpretation: sar::CliSarInterpretation,
        #[arg(long)]
        out: PathBuf,
    },
    SarFit {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long, value_enum)]
        model: sar::CliSarModel,
        #[arg(long, value_enum)]
        interpretation: sar::CliSarInterpretation,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        rho_bound: f64,
        #[arg(long)]
        sigma_prior_sd: f64,
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
    BymFit {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        structured_sd_prior: f64,
        #[arg(long)]
        unstructured_sd_prior: f64,
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
    Bym2Fit {
        #[arg(long)]
        regions: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        total_sd_prior: f64,
        #[arg(long)]
        phi_alpha: f64,
        #[arg(long)]
        phi_beta: f64,
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
    SpatialVaryingCoefficient {
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
    DistanceToResource {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        patient_effect_prior_sd: f64,
        #[arg(long)]
        known_noise_sd: f64,
        #[arg(long)]
        out: PathBuf,
    },
    RejectionAbcGrowthFront {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_um2_per_time: f64,
        #[arg(long)]
        carrying_capacity: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        front_threshold_fraction: f64,
        #[arg(long)]
        observed_final_mass: f64,
        #[arg(long)]
        mass_scale: f64,
        #[arg(long)]
        growth_rate_prior_min: f64,
        #[arg(long)]
        growth_rate_prior_max: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        accepted_draws: u32,
        #[arg(long)]
        maximum_proposals: u32,
        #[arg(long)]
        maximum_cell_steps_per_proposal: u64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SmcAbcGrowthFront {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_um2_per_time: f64,
        #[arg(long)]
        carrying_capacity: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        front_threshold_fraction: f64,
        #[arg(long)]
        observed_final_mass: f64,
        #[arg(long)]
        mass_scale: f64,
        #[arg(long)]
        growth_rate_prior_min: f64,
        #[arg(long)]
        growth_rate_prior_max: f64,
        #[arg(long, value_delimiter = ',')]
        epsilon_schedule: Vec<f64>,
        #[arg(long)]
        particles: u32,
        #[arg(long)]
        maximum_proposals_per_stage: u32,
        #[arg(long)]
        maximum_cell_steps_per_proposal: u64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SyntheticLikelihoodGrowthFront {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_um2_per_time: f64,
        #[arg(long)]
        carrying_capacity: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        front_threshold_fraction: f64,
        #[arg(long)]
        observed_final_mass: f64,
        #[arg(long)]
        observed_maximum_density: f64,
        #[arg(long)]
        mass_noise_sd: f64,
        #[arg(long)]
        maximum_density_noise_sd: f64,
        #[arg(long)]
        growth_rate_prior_min: f64,
        #[arg(long)]
        growth_rate_prior_max: f64,
        #[arg(long)]
        replicates: u32,
        #[arg(long)]
        covariance_shrinkage: f64,
        #[arg(long)]
        iterations: u32,
        #[arg(long)]
        burn_in: u32,
        #[arg(long)]
        proposal_sd: f64,
        #[arg(long)]
        maximum_cell_steps_per_simulation: u64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GrowthFrontRejectionAbcSbc {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_um2_per_time: f64,
        #[arg(long)]
        carrying_capacity: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        front_threshold_fraction: f64,
        #[arg(long)]
        mass_scale: f64,
        #[arg(long)]
        growth_rate_prior_min: f64,
        #[arg(long)]
        growth_rate_prior_max: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        posterior_draws: u32,
        #[arg(long)]
        maximum_proposals_per_replicate: u32,
        #[arg(long)]
        replicates: u32,
        #[arg(long)]
        coverage_probability: f64,
        #[arg(long)]
        maximum_cell_steps_per_simulation: u64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SimulationOod {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    GrowthFrontPosteriorPredictiveLab {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_um2_per_time: f64,
        #[arg(long)]
        carrying_capacity: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        front_threshold_fraction: f64,
        #[arg(long)]
        observed_final_mass: f64,
        #[arg(long)]
        observed_maximum_density: f64,
        #[arg(long)]
        replicates: u32,
        #[arg(long)]
        interval_probability: f64,
        #[arg(long)]
        discrepancy_alpha: f64,
        #[arg(long)]
        maximum_cell_steps_per_replicate: u64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    NormalMeanSmc {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        particles: u32,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        ess_target: f64,
        #[arg(long)]
        correlation_threshold: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    PoissonLogRateLaplace {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long, allow_hyphen_values = true)]
        initial_log_rate: f64,
        #[arg(long)]
        max_iterations: u32,
        #[arg(long)]
        gradient_tolerance: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    PoissonLognormalInla {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        latent_mean: f64,
        #[arg(long)]
        tau_shape: f64,
        #[arg(long)]
        tau_rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        log_tau_min: f64,
        #[arg(long, allow_hyphen_values = true)]
        log_tau_max: f64,
        #[arg(long)]
        grid_points: u32,
        #[arg(long)]
        endpoint_mass_limit: f64,
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
    PsisLoo {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        model_name: String,
        #[arg(long)]
        likelihood_target: String,
        #[arg(long)]
        data_identity_sha256: String,
        #[arg(long)]
        preprocessing_identity_sha256: String,
        #[arg(long)]
        heldout_unit: String,
        #[arg(long)]
        relative_efficiency: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    CompareModels {
        #[arg(long)]
        input: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    NormalMeanSbc {
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        observations_per_replicate: u32,
        #[arg(long)]
        replicates: u32,
        #[arg(long)]
        posterior_draws: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    NormalMeanPriorSensitivity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        priors: PathBuf,
        #[arg(long)]
        base_prior: String,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long, allow_hyphen_values = true)]
        decision_threshold: f64,
        #[arg(long)]
        decision_probability_threshold: f64,
        #[arg(long)]
        material_mean_shift: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    InhomogeneousPoissonLikelihood {
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        quadrature: PathBuf,
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
        intercept: f64,
        #[arg(long, allow_hyphen_values = true)]
        coefficient: f64,
        #[arg(long)]
        out: PathBuf,
    },
    FitInhomogeneousPoisson {
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        quadrature: PathBuf,
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
    BermanTurnerRefinement {
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        coarse_quadrature: PathBuf,
        #[arg(long)]
        fine_quadrature: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        coarse_grid_x: u32,
        #[arg(long)]
        coarse_grid_y: u32,
        #[arg(long)]
        fine_grid_x: u32,
        #[arg(long)]
        fine_grid_y: u32,
        #[arg(long, allow_hyphen_values = true)]
        intercept: f64,
        #[arg(long, allow_hyphen_values = true)]
        coefficient: f64,
        #[arg(long)]
        convergence_tolerance: f64,
        #[arg(long)]
        out: PathBuf,
    },
    BuildGriddedLgcp {
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
        out: PathBuf,
    },
    FitGriddedLgcp {
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
    GriddedLgcpAgreement {
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
        maximum_standardized_difference: f64,
        #[arg(long)]
        minimum_parameter_tolerance: f64,
        #[arg(long)]
        minimum_field_tolerance: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GriddedLgcpSbc {
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
        replicates: u32,
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
        minimum_rank_uniformity_p_value: f64,
        #[arg(long)]
        minimum_coverage_90: f64,
        #[arg(long)]
        maximum_coverage_90: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GriddedLgcpSensitivity {
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
        lower_scale_multiplier: f64,
        #[arg(long)]
        upper_scale_multiplier: f64,
        #[arg(long)]
        material_standardized_shift: f64,
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
    GriddedLgcpSpatialPpc {
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
        replicates: u32,
        #[arg(long)]
        prediction_seed: u64,
        #[arg(long)]
        maximum_total_points: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SimulateGriddedLgcpPosteriorPredictive {
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
        replicates: u32,
        #[arg(long)]
        prediction_seed: u64,
        #[arg(long)]
        maximum_total_points: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SimulateThomasProcess {
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        kappa_parent_per_um2: f64,
        #[arg(long)]
        mu_offspring: f64,
        #[arg(long)]
        sigma_um: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_parents: u64,
        #[arg(long)]
        maximum_offspring: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SimulateMaternClusterProcess {
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        kappa_parent_per_um2: f64,
        #[arg(long)]
        mu_offspring: f64,
        #[arg(long)]
        radius_um: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_parents: u64,
        #[arg(long)]
        maximum_offspring: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FitThomasMinimumContrast {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        observed_intensity_per_um2: f64,
        #[arg(long)]
        kappa_min_per_um2: f64,
        #[arg(long)]
        kappa_max_per_um2: f64,
        #[arg(long)]
        sigma_min_um: f64,
        #[arg(long)]
        sigma_max_um: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    StraussStatistics {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        interaction_radius_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        proposal_x_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        proposal_y_um: f64,
        #[arg(long)]
        beta_per_um2: f64,
        #[arg(long)]
        gamma: f64,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SimulateStraussBirthDeath {
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        beta_per_um2: f64,
        #[arg(long)]
        gamma: f64,
        #[arg(long)]
        interaction_radius_um: f64,
        #[arg(long)]
        iterations: u32,
        #[arg(long)]
        burn_in: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_points: u32,
        #[arg(long)]
        maximum_neighbor_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FitStraussPseudolikelihood {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        interaction_radius_um: f64,
        #[arg(long)]
        coarse_grid_x: u32,
        #[arg(long)]
        coarse_grid_y: u32,
        #[arg(long)]
        fine_grid_x: u32,
        #[arg(long)]
        fine_grid_y: u32,
        #[arg(long)]
        beta_min_per_um2: f64,
        #[arg(long)]
        beta_max_per_um2: f64,
        #[arg(long)]
        gamma_min: f64,
        #[arg(long)]
        gamma_max: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        maximum_neighbor_visits: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GeyerSaturationStatistic {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        interaction_radius_um: f64,
        #[arg(long)]
        saturation: u32,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MultitypePapangelou {
        #[arg(long)]
        points: PathBuf,
        #[arg(long)]
        baselines: PathBuf,
        #[arg(long)]
        interactions: PathBuf,
        #[arg(long)]
        proposal_type: String,
        #[arg(long, allow_hyphen_values = true)]
        proposal_x_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        proposal_y_um: f64,
        #[arg(long)]
        maximum_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BuildJointLocationMarkModel {
        #[arg(long)]
        points: PathBuf,
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
        #[arg(long)]
        reference_mark: String,
        #[arg(long)]
        location_prior_sd: f64,
        #[arg(long)]
        mark_coefficient_prior_sd: f64,
        #[arg(long)]
        mark_field_prior_sd: f64,
        #[arg(long)]
        out: PathBuf,
    },
    BuildJointContinuousMarkModel {
        #[arg(long)]
        points: PathBuf,
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
        #[arg(long)]
        known_mark_noise_sd: f64,
        #[arg(long)]
        shared_field_amplitude: f64,
        #[arg(long)]
        shared_field_length_scale_um: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        mark_loading_prior_sd: f64,
        #[arg(long)]
        private_field_prior_sd: f64,
        #[arg(long)]
        out: PathBuf,
    },
    BuildJointLocationEmbeddingFactorModel {
        #[arg(long)]
        points: PathBuf,
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
        #[arg(long)]
        factors: u32,
        #[arg(long)]
        field_amplitude: f64,
        #[arg(long)]
        field_length_scale_um: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        loading_prior_sd: f64,
        #[arg(long)]
        noise_prior_sd: f64,
        #[arg(long)]
        out: PathBuf,
    },
    BuildReplicatedHierarchicalLgcp {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        field_policy: String,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        patient_intercept_prior_sd: f64,
        #[arg(long)]
        population_field_amplitude: f64,
        #[arg(long)]
        population_field_length_scale_um: f64,
        #[arg(long)]
        replicate_field_amplitude: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        out: PathBuf,
    },
    PointProcessPosteriorPredictiveDiagnostics {
        #[arg(long)]
        observed: PathBuf,
        #[arg(long)]
        replicated: PathBuf,
        #[arg(long)]
        radii: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    VectorSemivariogram {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        weights: Option<PathBuf>,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    ProjectedEmbeddingVariograms {
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
    EmbeddingCrossCovarianceByDistance {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        maximum_matrix_elements: u64,
        #[arg(long)]
        out: PathBuf,
    },
    CrossModalCovarianceByDistance {
        #[arg(long)]
        a_input: PathBuf,
        #[arg(long)]
        b_input: PathBuf,
        #[arg(long)]
        pairs: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        mean_policy: String,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_pairs: u64,
        #[arg(long)]
        maximum_component_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    KernelMarkCorrelation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        kernel: String,
        #[arg(long)]
        global_reference_tolerance: f64,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    EmbeddingSpatialDependenceEnvelope {
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
        maximum_pair_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GraphDirichletEnergy {
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        laplacian: String,
        #[arg(long)]
        normalization: String,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GraphSmoothnessPermutationTest {
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
        maximum_component_edge_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    LocalEmbeddingRoughness {
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    TestCellPatchComplementarity {
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
        out: PathBuf,
    },
    MultiscaleEmbeddingKernel {
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
        maximum_component_scale_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    RetrieveAnalogousRegions {
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
    ApplyAbstention {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        maximum_uncertainty: f64,
        #[arg(long)]
        maximum_ood_score: f64,
        #[arg(long)]
        out: PathBuf,
    },
    OodScore {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        shrinkage: f64,
        #[arg(long)]
        validation_quantile: f64,
        #[arg(long)]
        out: PathBuf,
    },
    CalibratePredictions {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        method: String,
        #[arg(long)]
        bins: u32,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GroupedConformal {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        l2_penalty: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    LateFusion {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        l2_penalty: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    PredictiveStacking {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MixtureOfExpertsFusion {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        l2_penalty: f64,
        #[arg(long)]
        entropy_regularization: f64,
        #[arg(long)]
        ood_validation_quantile: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SinkhornOt {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        cost: PathBuf,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        out: PathBuf,
    },
    EntropicSoftAssignment {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        cost: PathBuf,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        dustbin_cost: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        out: PathBuf,
    },
    FusedGromovWasserstein {
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
    PartialFusedGromovWasserstein {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        transported_mass: f64,
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
    UnbalancedSinkhorn {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        cost: PathBuf,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        tau_source: f64,
        #[arg(long)]
        tau_target: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        out: PathBuf,
    },
    PartialOt {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        cost: PathBuf,
        #[arg(long)]
        transported_mass: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(super) enum BayesCliError {
    #[error("Bayesian input error: {0}")]
    Input(String),
    #[error("Bayesian model failed: {0}")]
    Model(#[from] BayesError),
    #[error("Bayesian backend failed: {0}")]
    Backend(String),
    #[error("Bayesian I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Bayesian CSV boundary failed: {0}")]
    Csv(#[from] csv::Error),
    #[error("Bayesian JSON boundary failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub(super) fn into_marklab_error(error: BayesCliError) -> marklab::MarklabError {
    match error {
        BayesCliError::Input(message) => marklab::MarklabError::Validation(message),
        BayesCliError::Model(BayesError::InvalidSpec(message)) => {
            marklab::MarklabError::Validation(message)
        }
        BayesCliError::Model(error) => marklab::MarklabError::Compute(error.to_string()),
        BayesCliError::Backend(message) => marklab::MarklabError::Compute(message),
        BayesCliError::Io { path, source } => marklab::MarklabError::io(path, source),
        BayesCliError::Csv(error) => marklab::MarklabError::Csv(error),
        BayesCliError::Json(error) => marklab::MarklabError::Json(error),
    }
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    match BayesCli::parse_from(std::env::args_os()).command {
        BayesTopLevel::Bayes {
            command:
                BayesCommand::StudentTHierarchy {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    observation_sd_prior,
                    degrees_of_freedom_excess_rate,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => student_t_hierarchy::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::StudentTHierarchyAgreement {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    observation_sd_prior,
                    degrees_of_freedom_excess_rate,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    maximum_standardized_difference,
                    minimum_location_scale_tolerance,
                    minimum_degrees_of_freedom_tolerance,
                    minimum_patient_tolerance,
                    timeout_seconds,
                    out,
                },
        } => student_t_hierarchy_agreement::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            maximum_standardized_difference,
            minimum_location_scale_tolerance,
            minimum_degrees_of_freedom_tolerance,
            minimum_patient_tolerance,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::StudentTHierarchySensitivity {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    observation_sd_prior,
                    degrees_of_freedom_excess_rate,
                    lower_scale_multiplier,
                    upper_scale_multiplier,
                    material_standardized_shift,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => student_t_hierarchy_sensitivity::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            lower_scale_multiplier,
            upper_scale_multiplier,
            material_standardized_shift,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::StudentTHierarchySbc {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    observation_sd_prior,
                    degrees_of_freedom_excess_rate,
                    replicates,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    minimum_rank_uniformity_p_value,
                    minimum_coverage_90,
                    maximum_coverage_90,
                    timeout_seconds,
                    out,
                },
        } => student_t_hierarchy_sbc::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            replicates,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            minimum_rank_uniformity_p_value,
            minimum_coverage_90,
            maximum_coverage_90,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialHierarchy {
                    input,
                    population_alpha,
                    population_beta,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_hierarchy::run(
            input,
            population_alpha,
            population_beta,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialHierarchyAgreement {
                    input,
                    population_alpha,
                    population_beta,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    maximum_standardized_difference,
                    minimum_probability_tolerance,
                    minimum_concentration_tolerance,
                    minimum_patient_tolerance,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_hierarchy_agreement::run(
            input,
            population_alpha,
            population_beta,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            maximum_standardized_difference,
            minimum_probability_tolerance,
            minimum_concentration_tolerance,
            minimum_patient_tolerance,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialHierarchySensitivity {
                    input,
                    population_alpha,
                    population_beta,
                    concentration_prior_sd,
                    lower_scale_multiplier,
                    upper_scale_multiplier,
                    material_standardized_shift,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_hierarchy_sensitivity::run(
            input,
            population_alpha,
            population_beta,
            concentration_prior_sd,
            lower_scale_multiplier,
            upper_scale_multiplier,
            material_standardized_shift,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialHierarchySbc {
                    input,
                    population_alpha,
                    population_beta,
                    concentration_prior_sd,
                    replicates,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    minimum_rank_uniformity_p_value,
                    minimum_coverage_90,
                    maximum_coverage_90,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_hierarchy_sbc::run(
            input,
            population_alpha,
            population_beta,
            concentration_prior_sd,
            replicates,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            minimum_rank_uniformity_p_value,
            minimum_coverage_90,
            maximum_coverage_90,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialGroupRegression {
                    input,
                    reference_group,
                    comparison_group,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_group_regression::run(
            input,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialGroupRegressionAgreement {
                    input,
                    reference_group,
                    comparison_group,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    maximum_standardized_difference,
                    minimum_probability_tolerance,
                    minimum_log_odds_tolerance,
                    minimum_concentration_tolerance,
                    minimum_patient_tolerance,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_group_regression_agreement::run(
            input,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            maximum_standardized_difference,
            minimum_probability_tolerance,
            minimum_log_odds_tolerance,
            minimum_concentration_tolerance,
            minimum_patient_tolerance,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BetaBinomialGroupRegressionSensitivity {
                    input,
                    reference_group,
                    comparison_group,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    lower_scale_multiplier,
                    upper_scale_multiplier,
                    material_standardized_shift,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => beta_binomial_group_regression_sensitivity::run(
            input,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            lower_scale_multiplier,
            upper_scale_multiplier,
            material_standardized_shift,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::NormalMean {
                    input,
                    prior_mean,
                    prior_sd,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_normal_mean(
            input,
            prior_mean,
            prior_sd,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::HierarchicalNormal {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => hierarchical::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::HierarchicalNormalAgreement {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    maximum_standardized_difference,
                    minimum_absolute_tolerance,
                    timeout_seconds,
                    out,
                },
        } => hierarchical_agreement::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            maximum_standardized_difference,
            minimum_absolute_tolerance,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::HierarchicalNormalPriorSensitivity {
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    known_sigma,
                    lower_scale_multiplier,
                    upper_scale_multiplier,
                    material_standardized_shift,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => hierarchical_sensitivity::run(
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            lower_scale_multiplier,
            upper_scale_multiplier,
            material_standardized_shift,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::HierarchicalNormalSbc {
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    known_sigma,
                    patients,
                    observations_per_patient,
                    replicates,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    minimum_rank_uniformity_p_value,
                    minimum_coverage_90,
                    maximum_coverage_90,
                    timeout_seconds,
                    out,
                },
        } => hierarchical_sbc::run(
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            patients,
            observations_per_patient,
            replicates,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            minimum_rank_uniformity_p_value,
            minimum_coverage_90,
            maximum_coverage_90,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::MetaAnalysis {
                    input,
                    covariate_name,
                    new_site_covariate,
                    global_prior_mean,
                    global_prior_sd,
                    covariate_prior_sd,
                    heterogeneity_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => meta_analysis::run(
            input,
            covariate_name,
            new_site_covariate,
            global_prior_mean,
            global_prior_sd,
            covariate_prior_sd,
            heterogeneity_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GpRegression {
                    input,
                    predict,
                    mean_prior_mean,
                    mean_prior_sd,
                    amplitude_prior_sd,
                    length_scale_prior_sd_um,
                    noise_prior_sd,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => gp::run(
            input,
            predict,
            mean_prior_mean,
            mean_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            noise_prior_sd,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::AnisotropicGp3d {
                    input,
                    predict,
                    mean_prior_mean,
                    mean_prior_sd,
                    amplitude_prior_sd,
                    length_scale_x_prior_sd_um,
                    length_scale_y_prior_sd_um,
                    length_scale_z_prior_sd_um,
                    noise_prior_sd,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => anisotropic_gp3d::run(
            input,
            predict,
            mean_prior_mean,
            mean_prior_sd,
            amplitude_prior_sd,
            [
                length_scale_x_prior_sd_um,
                length_scale_y_prior_sd_um,
                length_scale_z_prior_sd_um,
            ],
            noise_prior_sd,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::MultiOutputGp {
                    input,
                    predict,
                    output_a_name,
                    output_b_name,
                    mean_prior_sd,
                    amplitude_prior_sd,
                    length_scale_prior_sd_um,
                    loading_b_prior_sd,
                    noise_a_sd,
                    noise_b_sd,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => multi_output_gp::run(
            input,
            predict,
            output_a_name,
            output_b_name,
            mean_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            loading_b_prior_sd,
            noise_a_sd,
            noise_b_sd,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::VariationalGp {
                    input,
                    predict,
                    mean_prior_mean,
                    mean_prior_sd,
                    amplitude_prior_sd,
                    length_scale_prior_sd_um,
                    noise_prior_sd,
                    jitter,
                    inducing_points,
                    starts,
                    iterations,
                    learning_rate,
                    posterior_draws,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => variational_gp::run(
            input,
            predict,
            mean_prior_mean,
            mean_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            noise_prior_sd,
            jitter,
            inducing_points,
            starts,
            iterations,
            learning_rate,
            posterior_draws,
            seed,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PredictiveProcess {
                    input,
                    knots,
                    amplitude,
                    length_scale_um,
                    jitter,
                    diagonal_correction,
                    out,
                },
        } => predictive_process::run(
            input,
            knots,
            amplitude,
            length_scale_um,
            jitter,
            diagonal_correction,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::NngpDensity {
                    input,
                    mean,
                    amplitude,
                    length_scale_um,
                    neighbors,
                    jitter,
                    variance_tolerance,
                    full_reference,
                    out,
                },
        } => nngp::run(
            input,
            mean,
            amplitude,
            length_scale_um,
            neighbors,
            jitter,
            variance_tolerance,
            full_reference,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::ValidateWeights {
                    regions,
                    edges,
                    symmetry,
                    diagonal,
                    normalization,
                    out,
                },
        } => weights::run(regions, edges, symmetry, diagonal, normalization, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::CarDensity {
                    regions,
                    edges,
                    field,
                    mode,
                    tau,
                    rho,
                    constraint_tolerance,
                    island_policy,
                    out,
                },
        } => car::run(
            regions,
            edges,
            field,
            mode,
            tau,
            rho,
            constraint_tolerance,
            island_policy,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GmrfDensity {
                    regions,
                    precision,
                    field,
                    constraints,
                    constraint_tolerance,
                    out,
                },
        } => gmrf::run(
            regions,
            precision,
            field,
            constraints,
            constraint_tolerance,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SarLikelihood {
                    regions,
                    edges,
                    data,
                    coefficients,
                    model,
                    rho,
                    sigma,
                    interpretation,
                    out,
                },
        } => sar::run(
            regions,
            edges,
            data,
            coefficients,
            model,
            rho,
            sigma,
            interpretation,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SarFit {
                    regions,
                    edges,
                    data,
                    model,
                    interpretation,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_sd,
                    rho_bound,
                    sigma_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => sar_fit::run(
            regions,
            edges,
            data,
            model,
            interpretation,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            rho_bound,
            sigma_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BymFit {
                    regions,
                    edges,
                    data,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_sd,
                    structured_sd_prior,
                    unstructured_sd_prior,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => bym::run(
            regions,
            edges,
            data,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            structured_sd_prior,
            unstructured_sd_prior,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::Bym2Fit {
                    regions,
                    edges,
                    data,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_sd,
                    total_sd_prior,
                    phi_alpha,
                    phi_beta,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => bym2::run(
            regions,
            edges,
            data,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            total_sd_prior,
            phi_alpha,
            phi_beta,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SpatialVaryingCoefficient {
                    input,
                    global_predictor_name,
                    spatial_predictor_name,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_sd,
                    amplitude_prior_sd,
                    length_scale_prior_sd_um,
                    known_noise_sd,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => spatial_varying_coefficient::run(
            input,
            global_predictor_name,
            spatial_predictor_name,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            known_noise_sd,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::DistanceToResource {
                    input,
                    coefficient_prior_sd,
                    patient_effect_prior_sd,
                    known_noise_sd,
                    out,
                },
        } => distance_to_resource::run(
            input,
            coefficient_prior_sd,
            patient_effect_prior_sd,
            known_noise_sd,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::RejectionAbcGrowthFront {
                    input,
                    diffusion_um2_per_time,
                    carrying_capacity,
                    final_time,
                    time_step,
                    front_threshold_fraction,
                    observed_final_mass,
                    mass_scale,
                    growth_rate_prior_min,
                    growth_rate_prior_max,
                    epsilon,
                    accepted_draws,
                    maximum_proposals,
                    maximum_cell_steps_per_proposal,
                    seed,
                    out,
                },
        } => rejection_abc::run(
            input,
            diffusion_um2_per_time,
            carrying_capacity,
            final_time,
            time_step,
            front_threshold_fraction,
            observed_final_mass,
            mass_scale,
            growth_rate_prior_min,
            growth_rate_prior_max,
            epsilon,
            accepted_draws,
            maximum_proposals,
            maximum_cell_steps_per_proposal,
            seed,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SmcAbcGrowthFront {
                    input,
                    diffusion_um2_per_time,
                    carrying_capacity,
                    final_time,
                    time_step,
                    front_threshold_fraction,
                    observed_final_mass,
                    mass_scale,
                    growth_rate_prior_min,
                    growth_rate_prior_max,
                    epsilon_schedule,
                    particles,
                    maximum_proposals_per_stage,
                    maximum_cell_steps_per_proposal,
                    seed,
                    out,
                },
        } => smc_abc::run(
            input,
            diffusion_um2_per_time,
            carrying_capacity,
            final_time,
            time_step,
            front_threshold_fraction,
            observed_final_mass,
            mass_scale,
            growth_rate_prior_min,
            growth_rate_prior_max,
            epsilon_schedule,
            particles,
            maximum_proposals_per_stage,
            maximum_cell_steps_per_proposal,
            seed,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SyntheticLikelihoodGrowthFront {
                    input,
                    diffusion_um2_per_time,
                    carrying_capacity,
                    final_time,
                    time_step,
                    front_threshold_fraction,
                    observed_final_mass,
                    observed_maximum_density,
                    mass_noise_sd,
                    maximum_density_noise_sd,
                    growth_rate_prior_min,
                    growth_rate_prior_max,
                    replicates,
                    covariance_shrinkage,
                    iterations,
                    burn_in,
                    proposal_sd,
                    maximum_cell_steps_per_simulation,
                    seed,
                    out,
                },
        } => synthetic_likelihood::run(
            input,
            diffusion_um2_per_time,
            carrying_capacity,
            final_time,
            time_step,
            front_threshold_fraction,
            observed_final_mass,
            observed_maximum_density,
            mass_noise_sd,
            maximum_density_noise_sd,
            growth_rate_prior_min,
            growth_rate_prior_max,
            replicates,
            covariance_shrinkage,
            iterations,
            burn_in,
            proposal_sd,
            maximum_cell_steps_per_simulation,
            seed,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GrowthFrontRejectionAbcSbc {
                    input,
                    diffusion_um2_per_time,
                    carrying_capacity,
                    final_time,
                    time_step,
                    front_threshold_fraction,
                    mass_scale,
                    growth_rate_prior_min,
                    growth_rate_prior_max,
                    epsilon,
                    posterior_draws,
                    maximum_proposals_per_replicate,
                    replicates,
                    coverage_probability,
                    maximum_cell_steps_per_simulation,
                    seed,
                    out,
                },
        } => sbi_sbc::run(
            input,
            diffusion_um2_per_time,
            carrying_capacity,
            final_time,
            time_step,
            front_threshold_fraction,
            mass_scale,
            growth_rate_prior_min,
            growth_rate_prior_max,
            epsilon,
            posterior_draws,
            maximum_proposals_per_replicate,
            replicates,
            coverage_probability,
            maximum_cell_steps_per_simulation,
            seed,
            out,
        ),
        BayesTopLevel::Bayes {
            command: BayesCommand::SimulationOod { input, out },
        } => simulation_ood::run(input, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GrowthFrontPosteriorPredictiveLab {
                    input,
                    diffusion_um2_per_time,
                    carrying_capacity,
                    final_time,
                    time_step,
                    front_threshold_fraction,
                    observed_final_mass,
                    observed_maximum_density,
                    replicates,
                    interval_probability,
                    discrepancy_alpha,
                    maximum_cell_steps_per_replicate,
                    seed,
                    out,
                },
        } => posterior_predictive_lab::run(
            input,
            diffusion_um2_per_time,
            carrying_capacity,
            final_time,
            time_step,
            front_threshold_fraction,
            observed_final_mass,
            observed_maximum_density,
            replicates,
            interval_probability,
            discrepancy_alpha,
            maximum_cell_steps_per_replicate,
            seed,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::NormalMeanSmc {
                    input,
                    prior_mean,
                    prior_sd,
                    known_sigma,
                    particles,
                    chains,
                    ess_target,
                    correlation_threshold,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => smc::run(
            input,
            prior_mean,
            prior_sd,
            known_sigma,
            SmcSamplingSpec {
                particles,
                chains,
                ess_target,
                correlation_threshold,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PoissonLogRateLaplace {
                    input,
                    prior_mean,
                    prior_sd,
                    initial_log_rate,
                    max_iterations,
                    gradient_tolerance,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => laplace::run(
            input,
            prior_mean,
            prior_sd,
            initial_log_rate,
            max_iterations,
            gradient_tolerance,
            seed,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PoissonLognormalInla {
                    input,
                    latent_mean,
                    tau_shape,
                    tau_rate,
                    log_tau_min,
                    log_tau_max,
                    grid_points,
                    endpoint_mass_limit,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => inla::run(
            input,
            latent_mean,
            tau_shape,
            tau_rate,
            log_tau_min,
            log_tau_max,
            grid_points,
            endpoint_mass_limit,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PsisLoo {
                    input,
                    model_name,
                    likelihood_target,
                    data_identity_sha256,
                    preprocessing_identity_sha256,
                    heldout_unit,
                    relative_efficiency,
                    timeout_seconds,
                    out,
                },
        } => psis_loo::run(
            input,
            model_name,
            likelihood_target,
            data_identity_sha256,
            preprocessing_identity_sha256,
            heldout_unit,
            relative_efficiency,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command: BayesCommand::CompareModels { input, out },
        } => model_comparison::run(input, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::NormalMeanSbc {
                    prior_mean,
                    prior_sd,
                    known_sigma,
                    observations_per_replicate,
                    replicates,
                    posterior_draws,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => sbc::run(
            prior_mean,
            prior_sd,
            known_sigma,
            observations_per_replicate,
            replicates,
            posterior_draws,
            seed,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::NormalMeanPriorSensitivity {
                    input,
                    priors,
                    base_prior,
                    known_sigma,
                    decision_threshold,
                    decision_probability_threshold,
                    material_mean_shift,
                    timeout_seconds,
                    out,
                },
        } => prior_sensitivity::run(
            input,
            priors,
            base_prior,
            known_sigma,
            decision_threshold,
            decision_probability_threshold,
            material_mean_shift,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::InhomogeneousPoissonLikelihood {
                    events,
                    quadrature,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept,
                    coefficient,
                    out,
                },
        } => inhomogeneous_poisson::run(
            events,
            quadrature,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept,
            coefficient,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::FitInhomogeneousPoisson {
                    events,
                    quadrature,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => inhomogeneous_poisson_fit::run(
            events,
            quadrature,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BermanTurnerRefinement {
                    events,
                    coarse_quadrature,
                    fine_quadrature,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    coarse_grid_x,
                    coarse_grid_y,
                    fine_grid_x,
                    fine_grid_y,
                    intercept,
                    coefficient,
                    convergence_tolerance,
                    out,
                },
        } => berman_turner::run(
            events,
            coarse_quadrature,
            fine_quadrature,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            coarse_grid_x,
            coarse_grid_y,
            fine_grid_x,
            fine_grid_y,
            intercept,
            coefficient,
            convergence_tolerance,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BuildGriddedLgcp {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    out,
                },
        } => gridded_lgcp::run(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::FitGriddedLgcp {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => gridded_lgcp_fit::run(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GriddedLgcpAgreement {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    maximum_standardized_difference,
                    minimum_parameter_tolerance,
                    minimum_field_tolerance,
                    timeout_seconds,
                    out,
                },
        } => gridded_lgcp_agreement::run(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            maximum_standardized_difference,
            minimum_parameter_tolerance,
            minimum_field_tolerance,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GriddedLgcpSbc {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    replicates,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    minimum_rank_uniformity_p_value,
                    minimum_coverage_90,
                    maximum_coverage_90,
                    timeout_seconds,
                    out,
                },
        } => gridded_lgcp_sbc::run(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            replicates,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            minimum_rank_uniformity_p_value,
            minimum_coverage_90,
            maximum_coverage_90,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GriddedLgcpSensitivity {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    lower_scale_multiplier,
                    upper_scale_multiplier,
                    material_standardized_shift,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => gridded_lgcp_sensitivity::run(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            lower_scale_multiplier,
            upper_scale_multiplier,
            material_standardized_shift,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GriddedLgcpSpatialPpc {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    replicates,
                    prediction_seed,
                    maximum_total_points,
                    timeout_seconds,
                    out,
                },
        } => gridded_lgcp_spatial_ppc::run(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            replicates,
            prediction_seed,
            maximum_total_points,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SimulateGriddedLgcpPosteriorPredictive {
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    replicates,
                    prediction_seed,
                    maximum_total_points,
                    timeout_seconds,
                    out,
                },
        } => gridded_lgcp_fit::run_predictive(
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            replicates,
            prediction_seed,
            maximum_total_points,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SimulateThomasProcess {
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    kappa_parent_per_um2,
                    mu_offspring,
                    sigma_um,
                    seed,
                    maximum_parents,
                    maximum_offspring,
                    out,
                },
        } => thomas::run(
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            kappa_parent_per_um2,
            mu_offspring,
            sigma_um,
            seed,
            maximum_parents,
            maximum_offspring,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SimulateMaternClusterProcess {
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    kappa_parent_per_um2,
                    mu_offspring,
                    radius_um,
                    seed,
                    maximum_parents,
                    maximum_offspring,
                    out,
                },
        } => matern_cluster::run(
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            kappa_parent_per_um2,
            mu_offspring,
            radius_um,
            seed,
            maximum_parents,
            maximum_offspring,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::FitThomasMinimumContrast {
                    input,
                    observed_intensity_per_um2,
                    kappa_min_per_um2,
                    kappa_max_per_um2,
                    sigma_min_um,
                    sigma_max_um,
                    maximum_iterations,
                    timeout_seconds,
                    out,
                },
        } => thomas_minimum_contrast::run(
            input,
            observed_intensity_per_um2,
            kappa_min_per_um2,
            kappa_max_per_um2,
            sigma_min_um,
            sigma_max_um,
            maximum_iterations,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::StraussStatistics {
                    input,
                    interaction_radius_um,
                    proposal_x_um,
                    proposal_y_um,
                    beta_per_um2,
                    gamma,
                    maximum_pair_visits,
                    out,
                },
        } => strauss::run(
            input,
            interaction_radius_um,
            proposal_x_um,
            proposal_y_um,
            beta_per_um2,
            gamma,
            maximum_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SimulateStraussBirthDeath {
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    beta_per_um2,
                    gamma,
                    interaction_radius_um,
                    iterations,
                    burn_in,
                    seed,
                    maximum_points,
                    maximum_neighbor_visits,
                    out,
                },
        } => strauss_gibbs::run(
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            beta_per_um2,
            gamma,
            interaction_radius_um,
            iterations,
            burn_in,
            seed,
            maximum_points,
            maximum_neighbor_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::FitStraussPseudolikelihood {
                    input,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    interaction_radius_um,
                    coarse_grid_x,
                    coarse_grid_y,
                    fine_grid_x,
                    fine_grid_y,
                    beta_min_per_um2,
                    beta_max_per_um2,
                    gamma_min,
                    gamma_max,
                    maximum_iterations,
                    maximum_neighbor_visits,
                    timeout_seconds,
                    out,
                },
        } => strauss_pseudolikelihood::run(
            input,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            interaction_radius_um,
            coarse_grid_x,
            coarse_grid_y,
            fine_grid_x,
            fine_grid_y,
            beta_min_per_um2,
            beta_max_per_um2,
            gamma_min,
            gamma_max,
            maximum_iterations,
            maximum_neighbor_visits,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GeyerSaturationStatistic {
                    input,
                    interaction_radius_um,
                    saturation,
                    maximum_pair_visits,
                    out,
                },
        } => geyer::run(
            input,
            interaction_radius_um,
            saturation,
            maximum_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::MultitypePapangelou {
                    points,
                    baselines,
                    interactions,
                    proposal_type,
                    proposal_x_um,
                    proposal_y_um,
                    maximum_visits,
                    out,
                },
        } => multitype::run(
            points,
            baselines,
            interactions,
            proposal_type,
            proposal_x_um,
            proposal_y_um,
            maximum_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BuildJointLocationMarkModel {
                    points,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    reference_mark,
                    location_prior_sd,
                    mark_coefficient_prior_sd,
                    mark_field_prior_sd,
                    out,
                },
        } => joint_mark::run(
            points,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            reference_mark,
            location_prior_sd,
            mark_coefficient_prior_sd,
            mark_field_prior_sd,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BuildJointContinuousMarkModel {
                    points,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    known_mark_noise_sd,
                    shared_field_amplitude,
                    shared_field_length_scale_um,
                    jitter,
                    mark_loading_prior_sd,
                    private_field_prior_sd,
                    out,
                },
        } => joint_mark::run_continuous(
            points,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            known_mark_noise_sd,
            shared_field_amplitude,
            shared_field_length_scale_um,
            jitter,
            mark_loading_prior_sd,
            private_field_prior_sd,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BuildJointLocationEmbeddingFactorModel {
                    points,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    factors,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    loading_prior_sd,
                    noise_prior_sd,
                    out,
                },
        } => embedding_factor::run(
            points,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            factors,
            field_amplitude,
            field_length_scale_um,
            jitter,
            loading_prior_sd,
            noise_prior_sd,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::BuildReplicatedHierarchicalLgcp {
                    input,
                    field_policy,
                    global_prior_sd,
                    patient_intercept_prior_sd,
                    population_field_amplitude,
                    population_field_length_scale_um,
                    replicate_field_amplitude,
                    jitter,
                    out,
                },
        } => replicated::run(
            input,
            field_policy,
            global_prior_sd,
            patient_intercept_prior_sd,
            population_field_amplitude,
            population_field_length_scale_um,
            replicate_field_amplitude,
            jitter,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PointProcessPosteriorPredictiveDiagnostics {
                    observed,
                    replicated,
                    radii,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    alpha,
                    maximum_pair_visits,
                    out,
                },
        } => point_process_ppc::run(
            observed,
            replicated,
            radii,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            alpha,
            maximum_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::VectorSemivariogram {
                    input,
                    bins,
                    weights,
                    maximum_pair_visits,
                    out,
                },
        } => embedding_spatial::run_vector_semivariogram(
            input,
            bins,
            weights,
            maximum_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::ProjectedEmbeddingVariograms {
                    input,
                    bins,
                    components,
                    permutations,
                    seed,
                    maximum_pair_visits,
                    timeout_seconds,
                    out,
                },
        } => embedding_spatial::run_projected_variograms(
            input,
            bins,
            components,
            permutations,
            seed,
            maximum_pair_visits,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::EmbeddingCrossCovarianceByDistance {
                    input,
                    bins,
                    maximum_pair_visits,
                    maximum_matrix_elements,
                    out,
                },
        } => embedding_spatial::run_cross_covariance(
            input,
            bins,
            maximum_pair_visits,
            maximum_matrix_elements,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::CrossModalCovarianceByDistance {
                    a_input,
                    b_input,
                    pairs,
                    bins,
                    mean_policy,
                    permutations,
                    seed,
                    maximum_pairs,
                    maximum_component_pair_visits,
                    out,
                },
        } => cross_modal::run(
            a_input,
            b_input,
            pairs,
            bins,
            mean_policy,
            permutations,
            seed,
            maximum_pairs,
            maximum_component_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::KernelMarkCorrelation {
                    input,
                    bins,
                    kernel,
                    global_reference_tolerance,
                    maximum_pair_visits,
                    out,
                },
        } => embedding_kernel::run(
            input,
            bins,
            kernel,
            global_reference_tolerance,
            maximum_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::EmbeddingSpatialDependenceEnvelope {
                    input,
                    bins,
                    curve,
                    permutations,
                    alpha,
                    seed,
                    maximum_pair_visits,
                    out,
                },
        } => embedding_envelope::run(
            input,
            bins,
            curve,
            permutations,
            alpha,
            seed,
            maximum_pair_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GraphDirichletEnergy {
                    nodes,
                    edges,
                    laplacian,
                    normalization,
                    maximum_component_edge_visits,
                    out,
                },
        } => graph_signal::run_energy(
            nodes,
            edges,
            laplacian,
            normalization,
            maximum_component_edge_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GraphSmoothnessPermutationTest {
                    nodes,
                    edges,
                    laplacian,
                    permutations,
                    seed,
                    maximum_component_edge_visits,
                    out,
                },
        } => graph_signal::run_smoothness(
            nodes,
            edges,
            laplacian,
            permutations,
            seed,
            maximum_component_edge_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::LocalEmbeddingRoughness {
                    nodes,
                    edges,
                    epsilon,
                    maximum_component_edge_visits,
                    out,
                },
        } => graph_signal::run_local(nodes, edges, epsilon, maximum_component_edge_visits, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::TestCellPatchComplementarity {
                    input,
                    outer_folds,
                    inner_folds,
                    ridge_alphas,
                    permutations,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => complementarity::run(
            input,
            outer_folds,
            inner_folds,
            ridge_alphas,
            permutations,
            seed,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::MultiscaleEmbeddingKernel {
                    input,
                    weights,
                    sample_a,
                    sample_b,
                    base_kernel,
                    kernel_scale,
                    maximum_component_scale_visits,
                    out,
                },
        } => multiscale_kernel::run(
            input,
            weights,
            sample_a,
            sample_b,
            base_kernel,
            kernel_scale,
            maximum_component_scale_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::RetrieveAnalogousRegions {
                    training,
                    query,
                    k,
                    leakage_policy,
                    maximum_component_candidate_visits,
                    out,
                },
        } => retrieval::run(
            training,
            query,
            k,
            leakage_policy,
            maximum_component_candidate_visits,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::ApplyAbstention {
                    input,
                    maximum_uncertainty,
                    maximum_ood_score,
                    out,
                },
        } => prediction_safety::run_abstention(input, maximum_uncertainty, maximum_ood_score, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::OodScore {
                    input,
                    shrinkage,
                    validation_quantile,
                    out,
                },
        } => prediction_safety::run_ood(input, shrinkage, validation_quantile, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::CalibratePredictions {
                    input,
                    method,
                    bins,
                    timeout_seconds,
                    out,
                },
        } => prediction_calibration::run(input, method, bins, timeout_seconds, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::GroupedConformal {
                    input,
                    alpha,
                    l2_penalty,
                    timeout_seconds,
                    out,
                },
        } => grouped_conformal::run(input, alpha, l2_penalty, timeout_seconds, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::LateFusion {
                    input,
                    l2_penalty,
                    timeout_seconds,
                    out,
                },
        } => late_fusion::run(input, l2_penalty, timeout_seconds, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PredictiveStacking {
                    input,
                    timeout_seconds,
                    out,
                },
        } => predictive_stacking::run(input, timeout_seconds, out),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::MixtureOfExpertsFusion {
                    input,
                    l2_penalty,
                    entropy_regularization,
                    ood_validation_quantile,
                    timeout_seconds,
                    out,
                },
        } => mixture_of_experts::run(
            input,
            l2_penalty,
            entropy_regularization,
            ood_validation_quantile,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::SinkhornOt {
                    source,
                    target,
                    cost,
                    epsilon,
                    tolerance,
                    maximum_iterations,
                    out,
                },
        } => transport::run_sinkhorn(
            source,
            target,
            cost,
            epsilon,
            tolerance,
            maximum_iterations,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::EntropicSoftAssignment {
                    source,
                    target,
                    cost,
                    epsilon,
                    dustbin_cost,
                    tolerance,
                    maximum_iterations,
                    out,
                },
        } => transport::run_soft_assignment(
            source,
            target,
            cost,
            epsilon,
            dustbin_cost,
            tolerance,
            maximum_iterations,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::FusedGromovWasserstein {
                    input,
                    alpha,
                    epsilon,
                    feature_scale,
                    structure_scale,
                    tolerance,
                    maximum_iterations,
                    timeout_seconds,
                    out,
                },
        } => fused_gromov::run(
            input,
            alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PartialFusedGromovWasserstein {
                    input,
                    transported_mass,
                    alpha,
                    epsilon,
                    feature_scale,
                    structure_scale,
                    tolerance,
                    maximum_iterations,
                    timeout_seconds,
                    out,
                },
        } => partial_fused_gromov::run(
            input,
            transported_mass,
            alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            timeout_seconds,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::UnbalancedSinkhorn {
                    source,
                    target,
                    cost,
                    epsilon,
                    tau_source,
                    tau_target,
                    tolerance,
                    maximum_iterations,
                    out,
                },
        } => transport::run_unbalanced(
            source,
            target,
            cost,
            epsilon,
            tau_source,
            tau_target,
            tolerance,
            maximum_iterations,
            out,
        ),
        BayesTopLevel::Bayes {
            command:
                BayesCommand::PartialOt {
                    source,
                    target,
                    cost,
                    transported_mass,
                    epsilon,
                    timeout_seconds,
                    out,
                },
        } => transport::run_partial(
            source,
            target,
            cost,
            transported_mass,
            epsilon,
            timeout_seconds,
            out,
        ),
    }
}

fn run_normal_mean(
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = prepare_normal_mean(
        input_path,
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let worker_result = execute_normal_mean(&prepared)?;
    let fit = worker_result.into_fit(prepared.request, prepared.input_identity);
    publish_json(&output_path, &fit)
}

pub(super) struct PreparedNormalMean {
    pub(super) request: NormalMeanWorkerRequest,
    pub(super) request_bytes: Vec<u8>,
    pub(super) request_sha256: String,
    pub(super) input_identity: NormalMeanInputIdentity,
    pub(super) timeout_seconds: u64,
}

pub(super) fn prepare_normal_mean(
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
) -> Result<PreparedNormalMean, BayesCliError> {
    let observations = read_observations(&input_path)?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path.clone(),
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NormalMeanWorkerRequest::new(
        &NormalMeanSpec {
            prior_mean,
            prior_sd,
            known_sigma,
            observations: observations.clone(),
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedNormalMean {
        request,
        request_bytes,
        request_sha256,
        input_identity: NormalMeanInputIdentity {
            path: input_path.display().to_string(),
            observation_count: observations.len(),
            observations_sha256: observations_digest(&observations),
        },
        timeout_seconds,
    })
}

pub(super) fn execute_normal_mean(
    prepared: &PreparedNormalMean,
) -> Result<WorkerResult, BayesCliError> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let worker_result: WorkerResult = serde_json::from_slice(&result_bytes)?;
    worker_result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(worker_result)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRow {
    observation: f64,
}

fn read_observations(path: &Path) -> Result<Vec<f64>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(BayesCliError::Input(format!(
            "input must be a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"
        )));
    }
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["observation"]) {
        return Err(BayesCliError::Input(
            "normal-mean CSV headers must be exactly: observation".into(),
        ));
    }
    let mut observations = Vec::new();
    for row in reader.deserialize::<ObservationRow>() {
        observations.push(row?.observation);
        if observations.len() > 100_000 {
            return Err(BayesCliError::Input(
                "observation count exceeds 100000".into(),
            ));
        }
    }
    Ok(observations)
}

fn observations_digest(observations: &[f64]) -> String {
    let mut canonical = b"marklab-normal-mean-observations-v1\0".to_vec();
    canonical.extend_from_slice(&(observations.len() as u64).to_be_bytes());
    for observation in observations {
        canonical.extend_from_slice(&observation.to_bits().to_be_bytes());
    }
    sha256_hex(&canonical)
}

pub(super) fn run_worker(
    repository: &Path,
    worker_file_name: &str,
    request: &[u8],
    timeout_seconds: u64,
) -> Result<Vec<u8>, BayesCliError> {
    if std::env::var_os("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION").is_some() {
        return Err(BayesCliError::Backend(
            "external backend execution is disabled by MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"
                .into(),
        ));
    }
    let interpreter = repository.join("target/pymc-venv/bin/python");
    let worker = repository.join("workers/python").join(worker_file_name);
    if !interpreter.is_file() {
        return Err(BayesCliError::Backend(format!(
            "pinned Python environment is missing at {}; run `UV_PROJECT_ENVIRONMENT={}/target/pymc-venv uv sync --locked --python /usr/local/bin/python3.12 --no-python-downloads` from workers/python",
            interpreter.display(),
            repository.display()
        )));
    }
    if !worker.is_file() {
        return Err(BayesCliError::Backend(format!(
            "static worker is missing at {}",
            worker.display()
        )));
    }
    let cache = repository.join("target/pymc-cache");
    fs::create_dir_all(&cache).map_err(|source| BayesCliError::Io {
        path: cache.clone(),
        source,
    })?;
    let mut child = Command::new(&interpreter)
        .arg("-I")
        .arg(&worker)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("OMP_NUM_THREADS", "1")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .env(
            "PYTENSOR_FLAGS",
            format!("base_compiledir={}", cache.display()),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| BayesCliError::Io {
            path: interpreter.clone(),
            source,
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| BayesCliError::Backend("worker stdout was not piped".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| BayesCliError::Backend("worker stderr was not piped".into()))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout, MAXIMUM_WORKER_OUTPUT_BYTES));
    let stderr_reader = thread::spawn(move || read_bounded(stderr, MAXIMUM_WORKER_OUTPUT_BYTES));
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(BayesCliError::Backend("worker stdin was not piped".into()));
    };
    let write_result = stdin.write_all(request);
    drop(stdin);
    if let Err(source) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(BayesCliError::Io {
            path: worker.clone(),
            source,
        });
    }

    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|source| BayesCliError::Io {
            path: worker.clone(),
            source,
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(BayesCliError::Backend(format!(
                "external backend worker exceeded the {timeout_seconds}-second limit"
            )));
        }
        thread::sleep(Duration::from_millis(25));
    };

    let (stdout, stdout_exceeded) = join_reader(stdout_reader, "stdout")?;
    let (stderr, stderr_exceeded) = join_reader(stderr_reader, "stderr")?;
    validate_process_result(status, stdout, stdout_exceeded, stderr, stderr_exceeded)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut exceeded = false;
    let mut chunk = [0_u8; 8_192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let available = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..count.min(available)]);
        exceeded |= count > available;
    }
    Ok((retained, exceeded))
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stream: &str,
) -> Result<(Vec<u8>, bool), BayesCliError> {
    reader
        .join()
        .map_err(|_| BayesCliError::Backend(format!("worker {stream} reader panicked")))?
        .map_err(|source| BayesCliError::Io {
            path: PathBuf::from(format!("external backend worker {stream}")),
            source,
        })
}

fn validate_process_result(
    status: ExitStatus,
    stdout: Vec<u8>,
    stdout_exceeded: bool,
    stderr: Vec<u8>,
    stderr_exceeded: bool,
) -> Result<Vec<u8>, BayesCliError> {
    if stdout_exceeded || stderr_exceeded {
        return Err(BayesCliError::Backend(
            "external backend worker exceeded the 16 MiB output limit".into(),
        ));
    }
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        return Err(BayesCliError::Backend(format!(
            "external backend worker exited with {status}: {}",
            detail.trim()
        )));
    }
    if stdout.is_empty() {
        return Err(BayesCliError::Backend(
            "external backend worker succeeded without a JSON result".into(),
        ));
    }
    Ok(stdout)
}

pub(super) fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), BayesCliError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(BayesCliError::Input(format!(
                "output already exists: {}",
                path.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(BayesCliError::Io {
                path: path.to_owned(),
                source,
            })
        }
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| BayesCliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| BayesCliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-bayes-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let publication = (|| -> Result<(), BayesCliError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| BayesCliError::Io {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| BayesCliError::Io {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| BayesCliError::Io {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}
