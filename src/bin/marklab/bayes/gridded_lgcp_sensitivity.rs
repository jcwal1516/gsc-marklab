use marklab_bayes::{
    GriddedLgcpSensitivityResult, GriddedLgcpSensitivityScenario, GriddedLgcpSensitivityScenarioRun,
};

use super::cli_schema::{GriddedLgcpModelArgs, GriddedLgcpSensitivityArgs};
use super::{cross_backend, gridded_lgcp_fit, publish_json, CliError};

pub(super) fn run(args: GriddedLgcpSensitivityArgs) -> Result<(), CliError> {
    let GriddedLgcpSensitivityArgs {
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
        "gridded LGCP sensitivity multipliers or material-shift threshold are invalid",
    )?;
    let scenarios = [
        (
            "baseline",
            model.intercept_prior_sd,
            model.coefficient_prior_sd,
            model.field_amplitude,
            model.field_length_scale_um,
        ),
        (
            "intercept_sd_lower",
            model.intercept_prior_sd * lower_scale_multiplier,
            model.coefficient_prior_sd,
            model.field_amplitude,
            model.field_length_scale_um,
        ),
        (
            "intercept_sd_upper",
            model.intercept_prior_sd * upper_scale_multiplier,
            model.coefficient_prior_sd,
            model.field_amplitude,
            model.field_length_scale_um,
        ),
        (
            "coefficient_sd_lower",
            model.intercept_prior_sd,
            model.coefficient_prior_sd * lower_scale_multiplier,
            model.field_amplitude,
            model.field_length_scale_um,
        ),
        (
            "coefficient_sd_upper",
            model.intercept_prior_sd,
            model.coefficient_prior_sd * upper_scale_multiplier,
            model.field_amplitude,
            model.field_length_scale_um,
        ),
        (
            "field_amplitude_lower",
            model.intercept_prior_sd,
            model.coefficient_prior_sd,
            model.field_amplitude * lower_scale_multiplier,
            model.field_length_scale_um,
        ),
        (
            "field_amplitude_upper",
            model.intercept_prior_sd,
            model.coefficient_prior_sd,
            model.field_amplitude * upper_scale_multiplier,
            model.field_length_scale_um,
        ),
        (
            "field_length_scale_lower",
            model.intercept_prior_sd,
            model.coefficient_prior_sd,
            model.field_amplitude,
            model.field_length_scale_um * lower_scale_multiplier,
        ),
        (
            "field_length_scale_upper",
            model.intercept_prior_sd,
            model.coefficient_prior_sd,
            model.field_amplitude,
            model.field_length_scale_um * upper_scale_multiplier,
        ),
    ];
    let (input_identity, runs) = cross_backend::sensitivity(
        scenarios,
        "gridded LGCP inputs changed across sensitivity scenarios",
        |(_scenario_id, intercept_sd, coefficient_sd, amplitude, length_scale)| {
            gridded_lgcp_fit::prepare(
                None,
                GriddedLgcpModelArgs {
                    intercept_prior_sd: intercept_sd,
                    coefficient_prior_sd: coefficient_sd,
                    field_amplitude: amplitude,
                    field_length_scale_um: length_scale,
                    ..model.clone()
                },
            )
        },
        |(scenario_id, intercept_sd, coefficient_sd, amplitude, length_scale), prepared, result| {
            let scenario = GriddedLgcpSensitivityScenario {
                scenario_id: scenario_id.into(),
                intercept_prior_sd: intercept_sd,
                coefficient_prior_sd: coefficient_sd,
                field_amplitude: amplitude,
                field_length_scale_um: length_scale,
                covariance_sha256: prepared.request.covariance_sha256.clone(),
            };
            GriddedLgcpSensitivityScenarioRun { scenario, result }
        },
    )?;
    let identity = gridded_lgcp_fit::prepare(None, model.clone())?.input_identity;
    cross_backend::ensure_same_input::<gridded_lgcp_fit::PreparedGriddedLgcpFit>(
        &input_identity,
        &identity,
        "gridded LGCP inputs changed before sensitivity publication",
    )?;
    let result = GriddedLgcpSensitivityResult::new(
        identity,
        model.nuts.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
