use marklab_bayes::{
    StudentTHierarchySensitivityResult, StudentTSensitivityScenario, StudentTSensitivityScenarioRun,
};

use super::cli_schema::{StudentTHierarchyModelArgs, StudentTHierarchySensitivityArgs};
use super::{cross_backend, publish_json, student_t_hierarchy, CliError};

pub(super) fn run(args: StudentTHierarchySensitivityArgs) -> Result<(), CliError> {
    let StudentTHierarchySensitivityArgs {
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
        "Student-t sensitivity multipliers or material-shift threshold are invalid",
    )?;
    let scenarios = [
        (
            "baseline",
            model.global_prior_sd,
            model.between_patient_sd_prior,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "global_sd_lower",
            model.global_prior_sd * lower_scale_multiplier,
            model.between_patient_sd_prior,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "global_sd_upper",
            model.global_prior_sd * upper_scale_multiplier,
            model.between_patient_sd_prior,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "between_sd_lower",
            model.global_prior_sd,
            model.between_patient_sd_prior * lower_scale_multiplier,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "between_sd_upper",
            model.global_prior_sd,
            model.between_patient_sd_prior * upper_scale_multiplier,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "observation_sd_lower",
            model.global_prior_sd,
            model.between_patient_sd_prior,
            model.observation_sd_prior * lower_scale_multiplier,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "observation_sd_upper",
            model.global_prior_sd,
            model.between_patient_sd_prior,
            model.observation_sd_prior * upper_scale_multiplier,
            model.degrees_of_freedom_excess_rate,
        ),
        (
            "degrees_of_freedom_rate_lower",
            model.global_prior_sd,
            model.between_patient_sd_prior,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate * lower_scale_multiplier,
        ),
        (
            "degrees_of_freedom_rate_upper",
            model.global_prior_sd,
            model.between_patient_sd_prior,
            model.observation_sd_prior,
            model.degrees_of_freedom_excess_rate * upper_scale_multiplier,
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "Student-t input changed across sensitivity scenarios",
        |(_scenario_id, global_sd, between_sd, observation_sd, df_rate)| {
            student_t_hierarchy::prepare(StudentTHierarchyModelArgs {
                global_prior_sd: global_sd,
                between_patient_sd_prior: between_sd,
                observation_sd_prior: observation_sd,
                degrees_of_freedom_excess_rate: df_rate,
                ..model.clone()
            })
        },
        |(scenario_id, global_sd, between_sd, observation_sd, df_rate), _prepared, result| {
            StudentTSensitivityScenarioRun {
                scenario: StudentTSensitivityScenario {
                    scenario_id: scenario_id.into(),
                    global_prior_sd: global_sd,
                    between_patient_sd_prior: between_sd,
                    observation_sd_prior: observation_sd,
                    degrees_of_freedom_excess_rate: df_rate,
                },
                result,
            }
        },
    )?;
    let result = StudentTHierarchySensitivityResult::new(
        input_identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
