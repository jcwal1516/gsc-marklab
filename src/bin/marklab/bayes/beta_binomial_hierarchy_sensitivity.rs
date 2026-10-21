use marklab_bayes::{
    BetaBinomialSensitivityResult, BetaBinomialSensitivityScenario,
    BetaBinomialSensitivityScenarioRun,
};

use super::cli_schema::{BetaBinomialHierarchyModelArgs, BetaBinomialHierarchySensitivityArgs};
use super::{beta_binomial_hierarchy, cross_backend, publish_json, CliError};

pub(super) fn run(args: BetaBinomialHierarchySensitivityArgs) -> Result<(), CliError> {
    let BetaBinomialHierarchySensitivityArgs {
        model,
        lower_scale_multiplier,
        upper_scale_multiplier,
        material_standardized_shift,
        out,
    } = args;
    let output_path = out;
    cross_backend::validate_sensitivity_controls(
        lower_scale_multiplier,
        upper_scale_multiplier,
        material_standardized_shift,
        "beta-binomial sensitivity multipliers or material-shift threshold are invalid",
    )?;
    let scenarios = [
        (
            "baseline",
            model.population_alpha,
            model.population_beta,
            model.concentration_prior_sd,
        ),
        (
            "population_alpha_lower",
            model.population_alpha * lower_scale_multiplier,
            model.population_beta,
            model.concentration_prior_sd,
        ),
        (
            "population_alpha_upper",
            model.population_alpha * upper_scale_multiplier,
            model.population_beta,
            model.concentration_prior_sd,
        ),
        (
            "population_beta_lower",
            model.population_alpha,
            model.population_beta * lower_scale_multiplier,
            model.concentration_prior_sd,
        ),
        (
            "population_beta_upper",
            model.population_alpha,
            model.population_beta * upper_scale_multiplier,
            model.concentration_prior_sd,
        ),
        (
            "concentration_sd_lower",
            model.population_alpha,
            model.population_beta,
            model.concentration_prior_sd * lower_scale_multiplier,
        ),
        (
            "concentration_sd_upper",
            model.population_alpha,
            model.population_beta,
            model.concentration_prior_sd * upper_scale_multiplier,
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "beta-binomial input changed across sensitivity scenarios",
        |(_scenario_id, alpha, beta, concentration_sd)| {
            beta_binomial_hierarchy::prepare(BetaBinomialHierarchyModelArgs {
                population_alpha: alpha,
                population_beta: beta,
                concentration_prior_sd: concentration_sd,
                ..model.clone()
            })
        },
        |(scenario_id, alpha, beta, concentration_sd), _prepared, result| {
            BetaBinomialSensitivityScenarioRun {
                scenario: BetaBinomialSensitivityScenario {
                    scenario_id: scenario_id.into(),
                    population_alpha: alpha,
                    population_beta: beta,
                    concentration_prior_sd: concentration_sd,
                },
                result,
            }
        },
    )?;
    let result = BetaBinomialSensitivityResult::new(
        input_identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
