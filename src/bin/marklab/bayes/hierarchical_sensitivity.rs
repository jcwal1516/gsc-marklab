use std::path::PathBuf;

use marklab_bayes::{
    HierarchicalPriorScenario, HierarchicalPriorScenarioRun, HierarchicalPriorSensitivityResult,
    NutsSamplingSpec,
};

use super::{hierarchical, publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    known_sigma: f64,
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
            "hierarchical prior-sensitivity multipliers or material-shift threshold are invalid"
                .into(),
        ));
    }
    let scenarios = [
        ("baseline", global_prior_sd, between_patient_sd_prior),
        (
            "between_patient_sd_lower",
            global_prior_sd,
            between_patient_sd_prior * lower_scale_multiplier,
        ),
        (
            "between_patient_sd_upper",
            global_prior_sd,
            between_patient_sd_prior * upper_scale_multiplier,
        ),
        (
            "global_sd_lower",
            global_prior_sd * lower_scale_multiplier,
            between_patient_sd_prior,
        ),
        (
            "global_sd_upper",
            global_prior_sd * upper_scale_multiplier,
            between_patient_sd_prior,
        ),
    ];
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(scenarios.len());
    for (scenario_id, scenario_global_sd, scenario_between_sd) in scenarios {
        let prepared = hierarchical::prepare(
            input_path.clone(),
            global_prior_mean,
            scenario_global_sd,
            scenario_between_sd,
            known_sigma,
            sampling.clone(),
            timeout_seconds,
        )?;
        if let Some(expected) = &input_identity {
            if expected != &prepared.input_identity.patient_data_sha256 {
                return Err(BayesCliError::Input(
                    "hierarchical input changed across prior-sensitivity scenarios".into(),
                ));
            }
        } else {
            input_identity = Some(prepared.input_identity.patient_data_sha256.clone());
        }
        let result = hierarchical::execute(&prepared)?;
        runs.push(HierarchicalPriorScenarioRun {
            scenario: HierarchicalPriorScenario {
                scenario_id: scenario_id.into(),
                global_prior_sd: scenario_global_sd,
                between_patient_sd_prior: scenario_between_sd,
            },
            result,
        });
    }
    let identity = hierarchical::prepare(
        input_path,
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        known_sigma,
        sampling.clone(),
        timeout_seconds,
    )?
    .input_identity;
    if input_identity.as_deref() != Some(identity.patient_data_sha256.as_str()) {
        return Err(BayesCliError::Input(
            "hierarchical input changed before prior-sensitivity publication".into(),
        ));
    }
    let result = HierarchicalPriorSensitivityResult::new(
        identity,
        known_sigma,
        sampling.seed,
        material_standardized_shift,
        runs,
    )?;
    publish_json(&output_path, &result)
}
