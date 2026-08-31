use super::*;

#[path = "cli_schema/command_dispatch.rs"]
mod command_dispatch;

pub(crate) use command_dispatch::{
    run_beta_binomial_group_gender_slide_hierarchy_agreement_cli,
    run_beta_binomial_group_gender_slide_hierarchy_sbc_cli,
    run_beta_binomial_group_gender_slide_hierarchy_sensitivity_cli, run_cli,
    run_dirichlet_multinomial_group_agreement_cli, run_dirichlet_multinomial_group_cli,
    run_dirichlet_multinomial_group_sbc_cli, run_dirichlet_multinomial_group_sensitivity_cli,
};

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

#[derive(Debug, Args)]
struct BetaBinomialGroupGenderSlideHierarchyAgreementArgs {
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
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_probability_tolerance: f64,
    #[arg(long)]
    minimum_log_odds_tolerance: f64,
    #[arg(long)]
    minimum_patient_sd_tolerance: f64,
    #[arg(long)]
    minimum_concentration_tolerance: f64,
    #[arg(long)]
    minimum_patient_probability_tolerance: f64,
    #[arg(long)]
    minimum_patient_effect_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct BetaBinomialGroupGenderSlideHierarchyAgreementCli {
    #[command(subcommand)]
    command: BetaBinomialGroupGenderSlideHierarchyAgreementTopLevel,
}

#[derive(Debug, Subcommand)]
enum BetaBinomialGroupGenderSlideHierarchyAgreementTopLevel {
    Bayes {
        #[command(subcommand)]
        command: BetaBinomialGroupGenderSlideHierarchyAgreementCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BetaBinomialGroupGenderSlideHierarchyAgreementCommand {
    BetaBinomialGroupGenderSlideHierarchyAgreement(
        Box<BetaBinomialGroupGenderSlideHierarchyAgreementArgs>,
    ),
}

#[derive(Debug, Args)]
struct BetaBinomialGroupGenderSlideHierarchySensitivityArgs {
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
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct BetaBinomialGroupGenderSlideHierarchySensitivityCli {
    #[command(subcommand)]
    command: BetaBinomialGroupGenderSlideHierarchySensitivityTopLevel,
}

#[derive(Debug, Subcommand)]
enum BetaBinomialGroupGenderSlideHierarchySensitivityTopLevel {
    Bayes {
        #[command(subcommand)]
        command: BetaBinomialGroupGenderSlideHierarchySensitivityCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BetaBinomialGroupGenderSlideHierarchySensitivityCommand {
    BetaBinomialGroupGenderSlideHierarchySensitivity(
        Box<BetaBinomialGroupGenderSlideHierarchySensitivityArgs>,
    ),
}

#[derive(Debug, Args)]
struct BetaBinomialGroupGenderSlideHierarchySbcArgs {
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
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct BetaBinomialGroupGenderSlideHierarchySbcCli {
    #[command(subcommand)]
    command: BetaBinomialGroupGenderSlideHierarchySbcTopLevel,
}

#[derive(Debug, Subcommand)]
enum BetaBinomialGroupGenderSlideHierarchySbcTopLevel {
    Bayes {
        #[command(subcommand)]
        command: BetaBinomialGroupGenderSlideHierarchySbcCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BetaBinomialGroupGenderSlideHierarchySbcCommand {
    BetaBinomialGroupGenderSlideHierarchySbc(Box<BetaBinomialGroupGenderSlideHierarchySbcArgs>),
}

#[derive(Debug, Args)]
struct DirichletMultinomialGroupArgs {
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
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct DirichletMultinomialGroupCli {
    #[command(subcommand)]
    command: DirichletMultinomialGroupTopLevel,
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupTopLevel {
    Bayes {
        #[command(subcommand)]
        command: DirichletMultinomialGroupCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupCommand {
    DirichletMultinomialGroup(Box<DirichletMultinomialGroupArgs>),
}

#[derive(Debug, Args)]
struct DirichletMultinomialGroupAgreementArgs {
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
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_probability_tolerance: f64,
    #[arg(long)]
    minimum_concentration_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct DirichletMultinomialGroupAgreementCli {
    #[command(subcommand)]
    command: DirichletMultinomialGroupAgreementTopLevel,
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupAgreementTopLevel {
    Bayes {
        #[command(subcommand)]
        command: DirichletMultinomialGroupAgreementCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupAgreementCommand {
    DirichletMultinomialGroupAgreement(Box<DirichletMultinomialGroupAgreementArgs>),
}

#[derive(Debug, Args)]
struct DirichletMultinomialGroupSensitivityArgs {
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
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct DirichletMultinomialGroupSensitivityCli {
    #[command(subcommand)]
    command: DirichletMultinomialGroupSensitivityTopLevel,
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupSensitivityTopLevel {
    Bayes {
        #[command(subcommand)]
        command: DirichletMultinomialGroupSensitivityCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupSensitivityCommand {
    DirichletMultinomialGroupSensitivity(Box<DirichletMultinomialGroupSensitivityArgs>),
}

#[derive(Debug, Args)]
struct DirichletMultinomialGroupSbcArgs {
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
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct DirichletMultinomialGroupSbcCli {
    #[command(subcommand)]
    command: DirichletMultinomialGroupSbcTopLevel,
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupSbcTopLevel {
    Bayes {
        #[command(subcommand)]
        command: DirichletMultinomialGroupSbcCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DirichletMultinomialGroupSbcCommand {
    DirichletMultinomialGroupSbc(Box<DirichletMultinomialGroupSbcArgs>),
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
    BetaBinomialGroupGenderRegression {
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
    BetaBinomialGroupGenderRegressionAgreement {
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
    BetaBinomialGroupGenderRegressionSensitivity {
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
    BetaBinomialGroupGenderRegressionSbc {
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
    BetaBinomialGroupGenderSlideHierarchy {
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
    BetaBinomialGroupRegressionSbc {
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
