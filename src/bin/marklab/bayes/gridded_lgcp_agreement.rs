use marklab_bayes::{GriddedLgcpAgreementPolicy, GriddedLgcpAgreementResult};

use super::cli_schema::GriddedLgcpAgreementArgs;
use super::{cross_backend, gridded_lgcp_fit, CliError};

pub(super) fn run(args: GriddedLgcpAgreementArgs) -> Result<(), CliError> {
    let GriddedLgcpAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_parameter_tolerance,
        minimum_field_tolerance,
        out,
    } = args;
    let output_path = out;
    let prepared = gridded_lgcp_fit::prepare(None, model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(GriddedLgcpAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                GriddedLgcpAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_parameter_tolerance,
                    minimum_field_tolerance,
                },
            )?)
        },
    )
}
