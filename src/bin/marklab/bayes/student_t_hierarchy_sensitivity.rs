use std::path::PathBuf;

use marklab_bayes::{
    NutsSamplingSpec, StudentTHierarchySensitivityResult, StudentTSensitivityScenario,
    StudentTSensitivityScenarioRun,
};

use super::{publish_json, student_t_hierarchy, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    observation_sd_prior: f64,
    degrees_of_freedom_excess_rate: f64,
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
            "Student-t sensitivity multipliers or material-shift threshold are invalid".into(),
        ));
    }
    let scenarios = [
        (
            "baseline",
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
        ),
        (
            "global_sd_lower",
            global_prior_sd * lower_scale_multiplier,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
        ),
        (
            "global_sd_upper",
            global_prior_sd * upper_scale_multiplier,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
        ),
        (
            "between_sd_lower",
            global_prior_sd,
            between_patient_sd_prior * lower_scale_multiplier,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
        ),
        (
            "between_sd_upper",
            global_prior_sd,
            between_patient_sd_prior * upper_scale_multiplier,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
        ),
        (
            "observation_sd_lower",
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior * lower_scale_multiplier,
            degrees_of_freedom_excess_rate,
        ),
        (
            "observation_sd_upper",
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior * upper_scale_multiplier,
            degrees_of_freedom_excess_rate,
        ),
        (
            "degrees_of_freedom_rate_lower",
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate * lower_scale_multiplier,
        ),
        (
            "degrees_of_freedom_rate_upper",
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate * upper_scale_multiplier,
        ),
    ];
    let mut input_digest = None;
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(scenarios.len());
    for (scenario_id, global_sd, between_sd, observation_sd, df_rate) in scenarios {
        let prepared = student_t_hierarchy::prepare(
            input_path.clone(),
            global_prior_mean,
            global_sd,
            between_sd,
            observation_sd,
            df_rate,
            sampling.clone(),
            timeout_seconds,
        )?;
        if let Some(expected) = &input_digest {
            if expected != &prepared.input_identity.patient_data_sha256 {
                return Err(BayesCliError::Input(
                    "Student-t input changed across sensitivity scenarios".into(),
                ));
            }
        } else {
            input_digest = Some(prepared.input_identity.patient_data_sha256.clone());
            input_identity = Some(prepared.input_identity.clone());
        }
        let result = student_t_hierarchy::execute(&prepared)?;
        runs.push(StudentTSensitivityScenarioRun {
            scenario: StudentTSensitivityScenario {
                scenario_id: scenario_id.into(),
                global_prior_sd: global_sd,
                between_patient_sd_prior: between_sd,
                observation_sd_prior: observation_sd,
                degrees_of_freedom_excess_rate: df_rate,
            },
            result,
        });
    }
    let result = StudentTHierarchySensitivityResult::new(
        input_identity.expect("baseline scenario exists"),
        sampling.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
