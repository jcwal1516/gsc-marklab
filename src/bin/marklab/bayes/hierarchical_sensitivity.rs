use marklab_bayes::{
    HierarchicalPriorScenario, HierarchicalPriorScenarioRun, HierarchicalPriorSensitivityResult,
};

use super::cli_schema::{HierarchicalNormalModelArgs, HierarchicalNormalPriorSensitivityArgs};
use super::{cross_backend, hierarchical, publish_json, CliError};

pub(super) fn run(args: HierarchicalNormalPriorSensitivityArgs) -> Result<(), CliError> {
    let HierarchicalNormalPriorSensitivityArgs {
        model,
        input,
        lower_scale_multiplier,
        upper_scale_multiplier,
        material_standardized_shift,
        out,
    } = args;
    let input_path = input;
    let output_path = out;
    cross_backend::validate_sensitivity_controls(
        lower_scale_multiplier,
        upper_scale_multiplier,
        material_standardized_shift,
        "hierarchical prior-sensitivity multipliers or material-shift threshold are invalid",
    )?;
    let scenarios = [
        (
            "baseline",
            model.global_prior_sd,
            model.between_patient_sd_prior,
        ),
        (
            "between_patient_sd_lower",
            model.global_prior_sd,
            model.between_patient_sd_prior * lower_scale_multiplier,
        ),
        (
            "between_patient_sd_upper",
            model.global_prior_sd,
            model.between_patient_sd_prior * upper_scale_multiplier,
        ),
        (
            "global_sd_lower",
            model.global_prior_sd * lower_scale_multiplier,
            model.between_patient_sd_prior,
        ),
        (
            "global_sd_upper",
            model.global_prior_sd * upper_scale_multiplier,
            model.between_patient_sd_prior,
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "hierarchical input changed across prior-sensitivity scenarios",
        |(_scenario_id, scenario_global_sd, scenario_between_sd)| {
            hierarchical::prepare(
                input_path.clone(),
                HierarchicalNormalModelArgs {
                    global_prior_sd: scenario_global_sd,
                    between_patient_sd_prior: scenario_between_sd,
                    ..model.clone()
                },
            )
        },
        |(scenario_id, scenario_global_sd, scenario_between_sd), _prepared, result| {
            HierarchicalPriorScenarioRun {
                scenario: HierarchicalPriorScenario {
                    scenario_id: scenario_id.into(),
                    global_prior_sd: scenario_global_sd,
                    between_patient_sd_prior: scenario_between_sd,
                },
                result,
            }
        },
    )?;
    let identity = hierarchical::prepare(input_path, model.clone())?.input_identity;
    cross_backend::ensure_same_input::<hierarchical::PreparedGaussianHierarchy>(
        &input_identity,
        &identity,
        "hierarchical input changed before prior-sensitivity publication",
    )?;
    let result = HierarchicalPriorSensitivityResult::new(
        identity,
        model.known_sigma,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
