use marklab_bayes::{
    BetaBinomialGroupGenderAgreementPolicy, BetaBinomialGroupGenderAgreementResult,
};

use super::cli_schema::BetaBinomialGroupGenderRegressionAgreementArgs;
use super::{beta_binomial_group_gender_regression, cross_backend, CliError};

pub(super) fn run(args: BetaBinomialGroupGenderRegressionAgreementArgs) -> Result<(), CliError> {
    let BetaBinomialGroupGenderRegressionAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_probability_tolerance,
        minimum_log_odds_tolerance,
        minimum_concentration_tolerance,
        minimum_patient_tolerance,
        out,
    } = args;
    let output_path = out;
    let prepared = beta_binomial_group_gender_regression::prepare(model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(BetaBinomialGroupGenderAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                BetaBinomialGroupGenderAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_probability_tolerance,
                    minimum_log_odds_tolerance,
                    minimum_concentration_tolerance,
                    minimum_patient_tolerance,
                },
            )?)
        },
    )
}
