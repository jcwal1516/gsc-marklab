use marklab_bayes::{
    DirichletMultinomialGroupAgreementPolicy, DirichletMultinomialGroupAgreementResult,
};

use super::cli_schema::DirichletMultinomialGroupAgreementArgs;
use super::{cross_backend, dirichlet_multinomial_group, CliError};

pub(super) fn run(args: DirichletMultinomialGroupAgreementArgs) -> Result<(), CliError> {
    let DirichletMultinomialGroupAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_probability_tolerance,
        minimum_concentration_tolerance,
        out: output_path,
    } = args;
    let prepared = dirichlet_multinomial_group::prepare(model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(DirichletMultinomialGroupAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                DirichletMultinomialGroupAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_probability_tolerance,
                    minimum_concentration_tolerance,
                },
            )?)
        },
    )
}
