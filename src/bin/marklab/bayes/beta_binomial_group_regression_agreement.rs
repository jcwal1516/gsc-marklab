use marklab_bayes::{BetaBinomialGroupAgreementPolicy, BetaBinomialGroupAgreementResult};

use super::cli_schema::BetaBinomialGroupRegressionAgreementArgs;
use super::{beta_binomial_group_regression, cross_backend, CliError};

pub(super) fn run(args: BetaBinomialGroupRegressionAgreementArgs) -> Result<(), CliError> {
    let BetaBinomialGroupRegressionAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_probability_tolerance,
        minimum_log_odds_tolerance,
        minimum_concentration_tolerance,
        minimum_patient_tolerance,
        out,
    } = args;
    let output_path = out;
    let prepared = beta_binomial_group_regression::prepare(model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(BetaBinomialGroupAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                BetaBinomialGroupAgreementPolicy {
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
