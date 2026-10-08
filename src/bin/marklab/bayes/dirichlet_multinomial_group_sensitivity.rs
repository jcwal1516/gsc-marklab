use std::path::PathBuf;

use marklab_bayes::{
    DirichletMultinomialGroupSensitivityResult, DirichletMultinomialGroupSensitivityScenario,
    DirichletMultinomialGroupSensitivityScenarioRun, NutsSamplingSpec,
};

use super::{dirichlet_multinomial_group, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    logit_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
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
            "Dirichlet-multinomial sensitivity multipliers or material-shift threshold are invalid"
                .into(),
        ));
    }
    let scenarios = [
        (
            "baseline",
            logit_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
        ),
        (
            "logit_sd_lower",
            logit_prior_sd * lower_scale_multiplier,
            group_effect_prior_sd,
            concentration_prior_sd,
        ),
        (
            "logit_sd_upper",
            logit_prior_sd * upper_scale_multiplier,
            group_effect_prior_sd,
            concentration_prior_sd,
        ),
        (
            "group_effect_sd_lower",
            logit_prior_sd,
            group_effect_prior_sd * lower_scale_multiplier,
            concentration_prior_sd,
        ),
        (
            "group_effect_sd_upper",
            logit_prior_sd,
            group_effect_prior_sd * upper_scale_multiplier,
            concentration_prior_sd,
        ),
        (
            "concentration_sd_lower",
            logit_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd * lower_scale_multiplier,
        ),
        (
            "concentration_sd_upper",
            logit_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd * upper_scale_multiplier,
        ),
    ];
    let mut input_digest = None;
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(scenarios.len());
    for (scenario_id, logit_sd, group_sd, concentration_sd) in scenarios {
        let prepared = dirichlet_multinomial_group::prepare(
            input_path.clone(),
            reference_group.clone(),
            comparison_group.clone(),
            logit_sd,
            group_sd,
            concentration_sd,
            sampling.clone(),
            timeout_seconds,
        )?;
        if let Some(expected) = &input_digest {
            if expected != &prepared.input_identity.patient_data_sha256 {
                return Err(BayesCliError::Input(
                    "Dirichlet-multinomial input changed across sensitivity scenarios".into(),
                ));
            }
        } else {
            input_digest = Some(prepared.input_identity.patient_data_sha256.clone());
            input_identity = Some(prepared.input_identity.clone());
        }
        let result = dirichlet_multinomial_group::execute(&prepared)?;
        runs.push(DirichletMultinomialGroupSensitivityScenarioRun {
            scenario: DirichletMultinomialGroupSensitivityScenario {
                scenario_id: scenario_id.into(),
                logit_prior_sd: logit_sd,
                group_effect_prior_sd: group_sd,
                concentration_prior_sd: concentration_sd,
            },
            result,
        });
    }
    let result = DirichletMultinomialGroupSensitivityResult::new(
        input_identity.expect("baseline scenario exists"),
        sampling.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
