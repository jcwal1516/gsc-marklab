use marklab_bayes::{
    BetaBinomialGroupGenderSlideSensitivityResult, BetaBinomialGroupGenderSlideSensitivityScenario,
    BetaBinomialGroupGenderSlideSensitivityScenarioRun,
};

use super::cli_schema::{
    BetaBinomialGroupGenderSlideHierarchyModelArgs,
    BetaBinomialGroupGenderSlideHierarchySensitivityArgs,
};
use super::{beta_binomial_group_gender_slide_hierarchy, cross_backend, publish_json, CliError};

pub(super) fn run(
    args: BetaBinomialGroupGenderSlideHierarchySensitivityArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupGenderSlideHierarchySensitivityArgs {
        model,
        lower_scale_multiplier,
        upper_scale_multiplier,
        material_standardized_shift,
        out: output_path,
    } = args;
    cross_backend::validate_sensitivity_controls(
        lower_scale_multiplier,
        upper_scale_multiplier,
        material_standardized_shift,
        "slide hierarchy sensitivity controls are invalid",
    )?;
    let baseline = (
        model.intercept_prior_sd,
        model.group_effect_prior_sd,
        model.gender_effect_prior_sd,
        model.patient_log_odds_sd_prior_sd,
        model.slide_concentration_prior_sd,
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
                baseline.4,
            ),
        ),
        (
            "intercept_sd_upper",
            (
                baseline.0 * upper_scale_multiplier,
                baseline.1,
                baseline.2,
                baseline.3,
                baseline.4,
            ),
        ),
        (
            "group_effect_sd_lower",
            (
                baseline.0,
                baseline.1 * lower_scale_multiplier,
                baseline.2,
                baseline.3,
                baseline.4,
            ),
        ),
        (
            "group_effect_sd_upper",
            (
                baseline.0,
                baseline.1 * upper_scale_multiplier,
                baseline.2,
                baseline.3,
                baseline.4,
            ),
        ),
        (
            "gender_effect_sd_lower",
            (
                baseline.0,
                baseline.1,
                baseline.2 * lower_scale_multiplier,
                baseline.3,
                baseline.4,
            ),
        ),
        (
            "gender_effect_sd_upper",
            (
                baseline.0,
                baseline.1,
                baseline.2 * upper_scale_multiplier,
                baseline.3,
                baseline.4,
            ),
        ),
        (
            "patient_sd_lower",
            (
                baseline.0,
                baseline.1,
                baseline.2,
                baseline.3 * lower_scale_multiplier,
                baseline.4,
            ),
        ),
        (
            "patient_sd_upper",
            (
                baseline.0,
                baseline.1,
                baseline.2,
                baseline.3 * upper_scale_multiplier,
                baseline.4,
            ),
        ),
        (
            "slide_concentration_sd_lower",
            (
                baseline.0,
                baseline.1,
                baseline.2,
                baseline.3,
                baseline.4 * lower_scale_multiplier,
            ),
        ),
        (
            "slide_concentration_sd_upper",
            (
                baseline.0,
                baseline.1,
                baseline.2,
                baseline.3,
                baseline.4 * upper_scale_multiplier,
            ),
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "slide hierarchy input changed across sensitivity scenarios",
        |(_scenario_id, (intercept_sd, group_sd, gender_sd, patient_sd, concentration_sd))| {
            beta_binomial_group_gender_slide_hierarchy::prepare(
                BetaBinomialGroupGenderSlideHierarchyModelArgs {
                    intercept_prior_sd: intercept_sd,
                    group_effect_prior_sd: group_sd,
                    gender_effect_prior_sd: gender_sd,
                    patient_log_odds_sd_prior_sd: patient_sd,
                    slide_concentration_prior_sd: concentration_sd,
                    ..model.clone()
                },
            )
        },
        |(scenario_id, (intercept_sd, group_sd, gender_sd, patient_sd, concentration_sd)),
         _prepared,
         result| BetaBinomialGroupGenderSlideSensitivityScenarioRun {
            scenario: BetaBinomialGroupGenderSlideSensitivityScenario {
                scenario_id: scenario_id.into(),
                intercept_prior_sd: intercept_sd,
                group_effect_prior_sd: group_sd,
                gender_effect_prior_sd: gender_sd,
                patient_log_odds_sd_prior_sd: patient_sd,
                slide_concentration_prior_sd: concentration_sd,
            },
            result,
        },
    )?;
    let result = BetaBinomialGroupGenderSlideSensitivityResult::new(
        input_identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
