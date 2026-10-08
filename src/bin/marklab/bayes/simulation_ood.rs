use std::{fs, path::PathBuf};

use marklab_sbi::{detect_simulation_ood, SimulationOodSpec};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

pub(super) fn run(input_path: PathBuf, output_path: PathBuf) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "simulation OOD input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path,
        source,
    })?;
    let spec: SimulationOodSpec = serde_json::from_slice(&bytes)?;
    let result =
        detect_simulation_ood(spec).map_err(|error| BayesCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}
