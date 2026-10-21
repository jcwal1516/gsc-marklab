use marklab_bayes::{HierarchicalAgreementPolicy, HierarchicalAgreementResult};

use super::cli_schema::HierarchicalNormalAgreementArgs;
use super::{cross_backend, hierarchical, CliError};

pub(super) fn run(args: HierarchicalNormalAgreementArgs) -> Result<(), CliError> {
    let HierarchicalNormalAgreementArgs {
        model,
        input,
        maximum_standardized_difference,
        minimum_absolute_tolerance,
        out,
    } = args;
    let input_path = input;
    let output_path = out;
    let prepared = hierarchical::prepare(input_path, model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(HierarchicalAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                HierarchicalAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_absolute_tolerance,
                },
            )?)
        },
    )
}
