use std::path::PathBuf;

use marklab_bayes::{
    BetaBinomialGroupGenderSlideSensitivityResult, BetaBinomialGroupGenderSlideSensitivityScenario,
    BetaBinomialGroupGenderSlideSensitivityScenarioRun, NutsSamplingSpec,
};

use super::{beta_binomial_group_gender_slide_hierarchy, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    reference_gender: String,
    comparison_gender: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    gender_effect_prior_sd: f64,
    patient_log_odds_sd_prior_sd: f64,
    slide_concentration_prior_sd: f64,
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
            "slide hierarchy sensitivity controls are invalid".into(),
        ));
    }
    let baseline = (
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        patient_log_odds_sd_prior_sd,
        slide_concentration_prior_sd,
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
    let mut input_digest = None;
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(scenarios.len());
    for (scenario_id, (intercept_sd, group_sd, gender_sd, patient_sd, concentration_sd)) in
        scenarios
    {
        let prepared = beta_binomial_group_gender_slide_hierarchy::prepare(
            input_path.clone(),
            reference_group.clone(),
            comparison_group.clone(),
            reference_gender.clone(),
            comparison_gender.clone(),
            intercept_prior_mean,
            intercept_sd,
            group_sd,
            gender_sd,
            patient_sd,
            concentration_sd,
            sampling.clone(),
            timeout_seconds,
        )?;
        if let Some(expected) = &input_digest {
            if expected != &prepared.input_identity.slide_data_sha256 {
                return Err(BayesCliError::Input(
                    "slide hierarchy input changed across sensitivity scenarios".into(),
                ));
            }
        } else {
            input_digest = Some(prepared.input_identity.slide_data_sha256.clone());
            input_identity = Some(prepared.input_identity.clone());
        }
        let result = beta_binomial_group_gender_slide_hierarchy::execute(&prepared)?;
        runs.push(BetaBinomialGroupGenderSlideSensitivityScenarioRun {
            scenario: BetaBinomialGroupGenderSlideSensitivityScenario {
                scenario_id: scenario_id.into(),
                intercept_prior_sd: intercept_sd,
                group_effect_prior_sd: group_sd,
                gender_effect_prior_sd: gender_sd,
                patient_log_odds_sd_prior_sd: patient_sd,
                slide_concentration_prior_sd: concentration_sd,
            },
            result,
        });
    }
    let result = BetaBinomialGroupGenderSlideSensitivityResult::new(
        input_identity.expect("baseline exists"),
        sampling.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
