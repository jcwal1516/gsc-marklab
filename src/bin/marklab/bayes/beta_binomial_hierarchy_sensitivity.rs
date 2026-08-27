use std::path::PathBuf;

use marklab_bayes::{
    BetaBinomialSensitivityResult, BetaBinomialSensitivityScenario,
    BetaBinomialSensitivityScenarioRun, NutsSamplingSpec,
};

use super::{beta_binomial_hierarchy, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    population_alpha: f64,
    population_beta: f64,
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
            "beta-binomial sensitivity multipliers or material-shift threshold are invalid".into(),
        ));
    }
    let scenarios = [
        (
            "baseline",
            population_alpha,
            population_beta,
            concentration_prior_sd,
        ),
        (
            "population_alpha_lower",
            population_alpha * lower_scale_multiplier,
            population_beta,
            concentration_prior_sd,
        ),
        (
            "population_alpha_upper",
            population_alpha * upper_scale_multiplier,
            population_beta,
            concentration_prior_sd,
        ),
        (
            "population_beta_lower",
            population_alpha,
            population_beta * lower_scale_multiplier,
            concentration_prior_sd,
        ),
        (
            "population_beta_upper",
            population_alpha,
            population_beta * upper_scale_multiplier,
            concentration_prior_sd,
        ),
        (
            "concentration_sd_lower",
            population_alpha,
            population_beta,
            concentration_prior_sd * lower_scale_multiplier,
        ),
        (
            "concentration_sd_upper",
            population_alpha,
            population_beta,
            concentration_prior_sd * upper_scale_multiplier,
        ),
    ];
    let mut input_digest = None;
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(scenarios.len());
    for (scenario_id, alpha, beta, concentration_sd) in scenarios {
        let prepared = beta_binomial_hierarchy::prepare(
            input_path.clone(),
            alpha,
            beta,
            concentration_sd,
            sampling.clone(),
            timeout_seconds,
        )?;
        if let Some(expected) = &input_digest {
            if expected != &prepared.input_identity.patient_data_sha256 {
                return Err(BayesCliError::Input(
                    "beta-binomial input changed across sensitivity scenarios".into(),
                ));
            }
        } else {
            input_digest = Some(prepared.input_identity.patient_data_sha256.clone());
            input_identity = Some(prepared.input_identity.clone());
        }
        let result = beta_binomial_hierarchy::execute(&prepared)?;
        runs.push(BetaBinomialSensitivityScenarioRun {
            scenario: BetaBinomialSensitivityScenario {
                scenario_id: scenario_id.into(),
                population_alpha: alpha,
                population_beta: beta,
                concentration_prior_sd: concentration_sd,
            },
            result,
        });
    }
    let result = BetaBinomialSensitivityResult::new(
        input_identity.expect("baseline scenario exists"),
        sampling.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
