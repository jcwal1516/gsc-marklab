use std::path::{Path, PathBuf};

use marklab_bayes::{fit_sparse_car, SparseCarError, SparseCarSpec};

use super::{input_file, publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

pub(super) fn run(input_path: PathBuf, output_path: PathBuf) -> Result<(), BayesCliError> {
    let (spec, _) = read_spec(&input_path)?;
    let result = fit_sparse_car(&spec).map_err(map_error)?;
    publish_json(&output_path, &result)
}

pub(crate) fn read_spec(input_path: &Path) -> Result<(SparseCarSpec, Vec<u8>), BayesCliError> {
    let bytes = input_file::read_regular_file(
        input_path,
        MAXIMUM_INPUT_BYTES,
        "sparse CAR input must be a regular file within 16 MiB",
    )?;
    let spec: SparseCarSpec = serde_json::from_slice(&bytes)?;
    Ok((spec, bytes))
}

fn map_error(error: SparseCarError) -> BayesCliError {
    match error {
        SparseCarError::InvalidInput(message) | SparseCarError::ResourceLimit(message) => {
            BayesCliError::Input(message)
        }
        SparseCarError::Numerical(message) => BayesCliError::Backend(message),
    }
}
