use std::path::PathBuf;

use marklab_bayes::{
    GriddedLgcpSensitivityResult, GriddedLgcpSensitivityScenario,
    GriddedLgcpSensitivityScenarioRun, NutsSamplingSpec,
};

use super::{gridded_lgcp_fit, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    lower_scale_multiplier: f64,
    upper_scale_multiplier: f64,
    material_standardized_shift: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    if !lower_scale_multiplier.is_finite()
        || !(0.0..1.0).contains(&lower_scale_multiplier)
        || !upper_scale_multiplier.is_finite()
        || !(1.0..=10.0).contains(&upper_scale_multiplier)
        || !material_standardized_shift.is_finite()
        || material_standardized_shift <= 0.0
    {
        return Err(BayesCliError::Input(
            "gridded LGCP sensitivity multipliers or material-shift threshold are invalid".into(),
        ));
    }
    let scenarios = [
        (
            "baseline",
            intercept_prior_sd,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
        ),
        (
            "intercept_sd_lower",
            intercept_prior_sd * lower_scale_multiplier,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
        ),
        (
            "intercept_sd_upper",
            intercept_prior_sd * upper_scale_multiplier,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
        ),
        (
            "coefficient_sd_lower",
            intercept_prior_sd,
            coefficient_prior_sd * lower_scale_multiplier,
            field_amplitude,
            field_length_scale_um,
        ),
        (
            "coefficient_sd_upper",
            intercept_prior_sd,
            coefficient_prior_sd * upper_scale_multiplier,
            field_amplitude,
            field_length_scale_um,
        ),
        (
            "field_amplitude_lower",
            intercept_prior_sd,
            coefficient_prior_sd,
            field_amplitude * lower_scale_multiplier,
            field_length_scale_um,
        ),
        (
            "field_amplitude_upper",
            intercept_prior_sd,
            coefficient_prior_sd,
            field_amplitude * upper_scale_multiplier,
            field_length_scale_um,
        ),
        (
            "field_length_scale_lower",
            intercept_prior_sd,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um * lower_scale_multiplier,
        ),
        (
            "field_length_scale_upper",
            intercept_prior_sd,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um * upper_scale_multiplier,
        ),
    ];
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(scenarios.len());
    for (scenario_id, intercept_sd, coefficient_sd, amplitude, length_scale) in scenarios {
        let prepared = gridded_lgcp_fit::prepare(
            events_path.clone(),
            grid_path.clone(),
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_sd,
            coefficient_prior_mean,
            coefficient_sd,
            amplitude,
            length_scale,
            jitter,
            sampling.clone(),
            timeout_seconds,
            None,
        )?;
        if let Some(expected) = &input_identity {
            if expected
                != &(
                    prepared.input_identity.events_sha256.clone(),
                    prepared.input_identity.grid_sha256.clone(),
                )
            {
                return Err(BayesCliError::Input(
                    "gridded LGCP inputs changed across sensitivity scenarios".into(),
                ));
            }
        } else {
            input_identity = Some((
                prepared.input_identity.events_sha256.clone(),
                prepared.input_identity.grid_sha256.clone(),
            ));
        }
        let scenario = GriddedLgcpSensitivityScenario {
            scenario_id: scenario_id.into(),
            intercept_prior_sd: intercept_sd,
            coefficient_prior_sd: coefficient_sd,
            field_amplitude: amplitude,
            field_length_scale_um: length_scale,
            covariance_sha256: prepared.request.covariance_sha256.clone(),
        };
        let result = gridded_lgcp_fit::execute(&prepared)?;
        runs.push(GriddedLgcpSensitivityScenarioRun { scenario, result });
    }
    let identity = gridded_lgcp_fit::prepare(
        events_path,
        grid_path,
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
        sampling.clone(),
        timeout_seconds,
        None,
    )?
    .input_identity;
    if input_identity.as_ref()
        != Some(&(identity.events_sha256.clone(), identity.grid_sha256.clone()))
    {
        return Err(BayesCliError::Input(
            "gridded LGCP inputs changed before sensitivity publication".into(),
        ));
    }
    let result = GriddedLgcpSensitivityResult::new(
        identity,
        sampling.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
