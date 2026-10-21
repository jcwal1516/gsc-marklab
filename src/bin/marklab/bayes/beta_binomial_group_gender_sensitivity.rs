use marklab_bayes::{
    BetaBinomialGroupGenderSensitivityResult, BetaBinomialGroupGenderSensitivityScenario,
    BetaBinomialGroupGenderSensitivityScenarioRun,
};

use super::cli_schema::{
    BetaBinomialGroupGenderRegressionModelArgs, BetaBinomialGroupGenderRegressionSensitivityArgs,
};
use super::{beta_binomial_group_gender_regression, cross_backend, publish_json, CliError};

pub(super) fn run(args: BetaBinomialGroupGenderRegressionSensitivityArgs) -> Result<(), CliError> {
    let BetaBinomialGroupGenderRegressionSensitivityArgs {
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
        "group/gender sensitivity controls are invalid",
    )?;
    let baseline = (
        model.intercept_prior_sd,
        model.group_effect_prior_sd,
        model.gender_effect_prior_sd,
        model.concentration_prior_sd,
    );
    let scenarios = [
        ("baseline", baseline),
        (
            "intercept_sd_lower",
            (
                baseline.0 * lower_scale_multiplier,
                baseline.1,
                baseline.2,
                baseline.3,
            ),
        ),
        (
            "intercept_sd_upper",
            (
                baseline.0 * upper_scale_multiplier,
                baseline.1,
                baseline.2,
                baseline.3,
            ),
        ),
        (
            "group_effect_sd_lower",
            (
                baseline.0,
                baseline.1 * lower_scale_multiplier,
                baseline.2,
                baseline.3,
            ),
        ),
        (
            "group_effect_sd_upper",
            (
                baseline.0,
                baseline.1 * upper_scale_multiplier,
                baseline.2,
                baseline.3,
            ),
        ),
        (
            "gender_effect_sd_lower",
            (
                baseline.0,
                baseline.1,
                baseline.2 * lower_scale_multiplier,
                baseline.3,
            ),
        ),
        (
            "gender_effect_sd_upper",
            (
                baseline.0,
                baseline.1,
                baseline.2 * upper_scale_multiplier,
                baseline.3,
            ),
        ),
        (
            "concentration_sd_lower",
            (
                baseline.0,
                baseline.1,
                baseline.2,
                baseline.3 * lower_scale_multiplier,
            ),
        ),
        (
            "concentration_sd_upper",
            (
                baseline.0,
                baseline.1,
                baseline.2,
                baseline.3 * upper_scale_multiplier,
            ),
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "group/gender input changed across sensitivity scenarios",
        |(_scenario_id, (intercept_sd, group_sd, gender_sd, concentration_sd))| {
            beta_binomial_group_gender_regression::prepare(
                BetaBinomialGroupGenderRegressionModelArgs {
                    intercept_prior_sd: intercept_sd,
                    group_effect_prior_sd: group_sd,
                    gender_effect_prior_sd: gender_sd,
                    concentration_prior_sd: concentration_sd,
                    ..model.clone()
                },
            )
        },
        |(scenario_id, (intercept_sd, group_sd, gender_sd, concentration_sd)),
         _prepared,
         result| BetaBinomialGroupGenderSensitivityScenarioRun {
            scenario: BetaBinomialGroupGenderSensitivityScenario {
                scenario_id: scenario_id.into(),
                intercept_prior_sd: intercept_sd,
                group_effect_prior_sd: group_sd,
                gender_effect_prior_sd: gender_sd,
                concentration_prior_sd: concentration_sd,
            },
            result,
        },
    )?;
    let result = BetaBinomialGroupGenderSensitivityResult::new(
        input_identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
