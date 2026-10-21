use marklab_bayes::{
    BetaBinomialGroupSensitivityResult, BetaBinomialGroupSensitivityScenario,
    BetaBinomialGroupSensitivityScenarioRun,
};

use super::cli_schema::{
    BetaBinomialGroupRegressionModelArgs, BetaBinomialGroupRegressionSensitivityArgs,
};
use super::{beta_binomial_group_regression, cross_backend, publish_json, CliError};

pub(super) fn run(args: BetaBinomialGroupRegressionSensitivityArgs) -> Result<(), CliError> {
    let BetaBinomialGroupRegressionSensitivityArgs {
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
        "beta-binomial group sensitivity multipliers or material-shift threshold are invalid",
    )?;
    let scenarios = [
        (
            "baseline",
            model.intercept_prior_sd,
            model.group_effect_prior_sd,
            model.concentration_prior_sd,
        ),
        (
            "intercept_sd_lower",
            model.intercept_prior_sd * lower_scale_multiplier,
            model.group_effect_prior_sd,
            model.concentration_prior_sd,
        ),
        (
            "intercept_sd_upper",
            model.intercept_prior_sd * upper_scale_multiplier,
            model.group_effect_prior_sd,
            model.concentration_prior_sd,
        ),
        (
            "group_effect_sd_lower",
            model.intercept_prior_sd,
            model.group_effect_prior_sd * lower_scale_multiplier,
            model.concentration_prior_sd,
        ),
        (
            "group_effect_sd_upper",
            model.intercept_prior_sd,
            model.group_effect_prior_sd * upper_scale_multiplier,
            model.concentration_prior_sd,
        ),
        (
            "concentration_sd_lower",
            model.intercept_prior_sd,
            model.group_effect_prior_sd,
            model.concentration_prior_sd * lower_scale_multiplier,
        ),
        (
            "concentration_sd_upper",
            model.intercept_prior_sd,
            model.group_effect_prior_sd,
            model.concentration_prior_sd * upper_scale_multiplier,
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "beta-binomial group input changed across sensitivity scenarios",
        |(_scenario_id, intercept_sd, group_sd, concentration_sd)| {
            beta_binomial_group_regression::prepare(BetaBinomialGroupRegressionModelArgs {
                intercept_prior_sd: intercept_sd,
                group_effect_prior_sd: group_sd,
                concentration_prior_sd: concentration_sd,
                ..model.clone()
            })
        },
        |(scenario_id, intercept_sd, group_sd, concentration_sd), _prepared, result| {
            BetaBinomialGroupSensitivityScenarioRun {
                scenario: BetaBinomialGroupSensitivityScenario {
                    scenario_id: scenario_id.into(),
                    intercept_prior_sd: intercept_sd,
                    group_effect_prior_sd: group_sd,
                    concentration_prior_sd: concentration_sd,
                },
                result,
            }
        },
    )?;
    let result = BetaBinomialGroupSensitivityResult::new(
        input_identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
