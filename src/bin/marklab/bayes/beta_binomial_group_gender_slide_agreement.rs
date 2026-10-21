use marklab_bayes::{
    BetaBinomialGroupGenderSlideAgreementPolicy, BetaBinomialGroupGenderSlideAgreementResult,
};

use super::cli_schema::BetaBinomialGroupGenderSlideHierarchyAgreementArgs;
use super::{beta_binomial_group_gender_slide_hierarchy, cross_backend, CliError};

pub(super) fn run(
    args: BetaBinomialGroupGenderSlideHierarchyAgreementArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupGenderSlideHierarchyAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_probability_tolerance,
        minimum_log_odds_tolerance,
        minimum_patient_sd_tolerance,
        minimum_concentration_tolerance,
        minimum_patient_probability_tolerance,
        minimum_patient_effect_tolerance,
        out: output_path,
    } = args;
    let prepared = beta_binomial_group_gender_slide_hierarchy::prepare(model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(BetaBinomialGroupGenderSlideAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                BetaBinomialGroupGenderSlideAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_probability_tolerance,
                    minimum_log_odds_tolerance,
                    minimum_patient_sd_tolerance,
                    minimum_concentration_tolerance,
                    minimum_patient_probability_tolerance,
                    minimum_patient_effect_tolerance,
                },
            )?)
        },
    )
}
