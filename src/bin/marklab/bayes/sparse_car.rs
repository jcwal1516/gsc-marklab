use std::path::Path;

use marklab_bayes::{fit_sparse_car, SparseCarError, SparseCarSpec};

use super::cli_schema::SparseCarFitArgs;
use super::{publish_json, MAXIMUM_INPUT_BYTES};
use crate::cli_error::CliError;
use crate::input_file;

pub(super) fn run(args: SparseCarFitArgs) -> Result<(), CliError> {
    let SparseCarFitArgs { input, out } = args;
    let input_path = input;
    let output_path = out;
    let (spec, _) = read_spec(&input_path)?;
    let result = fit_sparse_car(&spec).map_err(map_error)?;
    publish_json(&output_path, &result)
}

pub(crate) fn read_spec(input_path: &Path) -> Result<(SparseCarSpec, Vec<u8>), CliError> {
    let bytes = input_file::read_regular_file(
        input_path,
        MAXIMUM_INPUT_BYTES,
        "sparse CAR input must be a regular file within 16 MiB",
    )?;
    let spec: SparseCarSpec = serde_json::from_slice(&bytes)?;
    Ok((spec, bytes))
}

fn map_error(error: SparseCarError) -> CliError {
    match error {
        SparseCarError::InvalidInput(message) | SparseCarError::ResourceLimit(message) => {
            CliError::Input(message)
        }
        SparseCarError::Numerical(message) => CliError::Backend(message),
    }
}
