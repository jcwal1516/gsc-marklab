use marklab_bayes::{BetaBinomialAgreementPolicy, BetaBinomialAgreementResult};

use super::cli_schema::BetaBinomialHierarchyAgreementArgs;
use super::{beta_binomial_hierarchy, cross_backend, CliError};

pub(super) fn run(args: BetaBinomialHierarchyAgreementArgs) -> Result<(), CliError> {
    let BetaBinomialHierarchyAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_probability_tolerance,
        minimum_concentration_tolerance,
        minimum_patient_tolerance,
        out,
    } = args;
    let output_path = out;
    let prepared = beta_binomial_hierarchy::prepare(model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(BetaBinomialAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                BetaBinomialAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_probability_tolerance,
                    minimum_concentration_tolerance,
                    minimum_patient_tolerance,
                },
            )?)
        },
    )
}
