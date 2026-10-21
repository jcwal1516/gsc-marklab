use std::{fs, path::PathBuf};

use marklab_simulation::{summary_matching_loss, SimulationError, SummaryMatchingSpec};

use crate::{MarklabError, Result};

pub(crate) fn run(input_path: PathBuf, output_path: PathBuf) -> Result<()> {
    let bytes = super::read_input(&input_path, "summary-matching")?;
    let spec: SummaryMatchingSpec = serde_json::from_slice(&bytes)?;
    let result = summary_matching_loss(spec).map_err(map)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    fs::write(&output_path, serde_json::to_vec_pretty(&result)?)
        .map_err(|source| MarklabError::io(&output_path, source))
}

fn map(error: SimulationError) -> MarklabError {
    MarklabError::Validation(error.to_string())
}
