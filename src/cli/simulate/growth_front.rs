use std::{fs, path::PathBuf};

use marklab_simulation::{
    simulate_growth_front, GrowthFrontInitialPoint, GrowthFrontSpec, SimulationError,
};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputRow {
    position_um: f64,
    density: f64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    input: PathBuf,
    diffusion_um2_per_time: f64,
    growth_rate_per_time: f64,
    carrying_capacity: f64,
    final_time: f64,
    time_step: f64,
    front_threshold_fraction: f64,
    record_every_steps: u32,
    maximum_cell_steps: u64,
    out: PathBuf,
) -> Result<()> {
    let metadata = fs::metadata(&input).map_err(|source| MarklabError::io(&input, source))?;
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(MarklabError::Validation(
            "growth-front input exceeds 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(&input)
        .map_err(|error| MarklabError::Validation(format!("growth-front CSV failed: {error}")))?;
    if !reader.headers()?.iter().eq(["position_um", "density"]) {
        return Err(MarklabError::Validation(
            "growth-front CSV header differs".into(),
        ));
    }
    let initial = reader
        .deserialize::<InputRow>()
        .map(|row| {
            row.map(|row| GrowthFrontInitialPoint {
                position_um: row.position_um,
                density: row.density,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| MarklabError::Validation(format!("growth-front CSV failed: {error}")))?;
    let result = simulate_growth_front(GrowthFrontSpec {
        initial,
        diffusion_um2_per_time,
        growth_rate_per_time,
        carrying_capacity,
        final_time,
        time_step,
        front_threshold_fraction,
        record_every_steps,
        maximum_cell_steps,
    })
    .map_err(map)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    let bytes = serde_json::to_vec_pretty(&result)?;
    fs::write(&out, bytes).map_err(|source| MarklabError::io(&out, source))
}

fn map(error: SimulationError) -> MarklabError {
    MarklabError::Validation(error.to_string())
}
