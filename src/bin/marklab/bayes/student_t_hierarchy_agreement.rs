use marklab_bayes::{StudentTHierarchyAgreementPolicy, StudentTHierarchyAgreementResult};

use super::cli_schema::StudentTHierarchyAgreementArgs;
use super::{cross_backend, student_t_hierarchy, CliError};

pub(super) fn run(args: StudentTHierarchyAgreementArgs) -> Result<(), CliError> {
    let StudentTHierarchyAgreementArgs {
        model,
        maximum_standardized_difference,
        minimum_location_scale_tolerance,
        minimum_degrees_of_freedom_tolerance,
        minimum_patient_tolerance,
        out,
    } = args;
    let output_path = out;
    let prepared = student_t_hierarchy::prepare(model)?;
    cross_backend::agreement(
        prepared,
        &output_path,
        |request, input_identity, pymc, numpyro| {
            Ok(StudentTHierarchyAgreementResult::new(
                request,
                input_identity,
                pymc,
                numpyro,
                StudentTHierarchyAgreementPolicy {
                    maximum_standardized_difference,
                    minimum_location_scale_tolerance,
                    minimum_degrees_of_freedom_tolerance,
                    minimum_patient_tolerance,
                },
            )?)
        },
    )
}
