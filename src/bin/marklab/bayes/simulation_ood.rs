use std::fs;

use marklab_sbi::{detect_simulation_ood, SimulationOodSpec};

use super::cli_schema::SimulationOodArgs;
use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};

pub(super) fn run(args: SimulationOodArgs) -> Result<(), CliError> {
    let SimulationOodArgs { input, out } = args;
    let input_path = input;
    let output_path = out;
    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "simulation OOD input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| CliError::Io {
        path: input_path,
        source,
    })?;
    let spec: SimulationOodSpec = serde_json::from_slice(&bytes)?;
    let result = detect_simulation_ood(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}
