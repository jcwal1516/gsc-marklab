use marklab_bayes::{
    DirichletMultinomialGroupSensitivityResult, DirichletMultinomialGroupSensitivityScenario,
    DirichletMultinomialGroupSensitivityScenarioRun,
};

use super::cli_schema::{
    DirichletMultinomialGroupModelArgs, DirichletMultinomialGroupSensitivityArgs,
};
use super::{cross_backend, dirichlet_multinomial_group, publish_json, CliError};

pub(super) fn run(args: DirichletMultinomialGroupSensitivityArgs) -> Result<(), CliError> {
    let DirichletMultinomialGroupSensitivityArgs {
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
        "Dirichlet-multinomial sensitivity multipliers or material-shift threshold are invalid",
    )?;
    let scenarios = [
        (
            "baseline",
            model.logit_prior_sd,
            model.group_effect_prior_sd,
            model.concentration_prior_sd,
        ),
        (
            "logit_sd_lower",
            model.logit_prior_sd * lower_scale_multiplier,
            model.group_effect_prior_sd,
            model.concentration_prior_sd,
        ),
        (
            "logit_sd_upper",
            model.logit_prior_sd * upper_scale_multiplier,
            model.group_effect_prior_sd,
            model.concentration_prior_sd,
        ),
        (
            "group_effect_sd_lower",
            model.logit_prior_sd,
            model.group_effect_prior_sd * lower_scale_multiplier,
            model.concentration_prior_sd,
        ),
        (
            "group_effect_sd_upper",
            model.logit_prior_sd,
            model.group_effect_prior_sd * upper_scale_multiplier,
            model.concentration_prior_sd,
        ),
        (
            "concentration_sd_lower",
            model.logit_prior_sd,
            model.group_effect_prior_sd,
            model.concentration_prior_sd * lower_scale_multiplier,
        ),
        (
            "concentration_sd_upper",
            model.logit_prior_sd,
            model.group_effect_prior_sd,
            model.concentration_prior_sd * upper_scale_multiplier,
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "Dirichlet-multinomial input changed across sensitivity scenarios",
        |(_scenario_id, logit_sd, group_sd, concentration_sd)| {
            dirichlet_multinomial_group::prepare(DirichletMultinomialGroupModelArgs {
                logit_prior_sd: logit_sd,
                group_effect_prior_sd: group_sd,
                concentration_prior_sd: concentration_sd,
                ..model.clone()
            })
        },
        |(scenario_id, logit_sd, group_sd, concentration_sd), _prepared, result| {
            DirichletMultinomialGroupSensitivityScenarioRun {
                scenario: DirichletMultinomialGroupSensitivityScenario {
                    scenario_id: scenario_id.into(),
                    logit_prior_sd: logit_sd,
                    group_effect_prior_sd: group_sd,
                    concentration_prior_sd: concentration_sd,
                },
                result,
            }
        },
    )?;
    let result = DirichletMultinomialGroupSensitivityResult::new(
        input_identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
