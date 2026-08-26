use std::{fs, path::PathBuf};

use marklab_simulation::{
    simulate_spatial_competition, CompetitionInitialPoint, CompetitionSpec, SimulationError,
};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputRow {
    position_um: f64,
    density_a: f64,
    density_b: f64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    input: PathBuf,
    diffusion_a_um2_per_time: f64,
    diffusion_b_um2_per_time: f64,
    growth_a_per_time: f64,
    growth_b_per_time: f64,
    carrying_a: f64,
    carrying_b: f64,
    competition_a_from_b: f64,
    competition_b_from_a: f64,
    treatment_a_per_time: f64,
    treatment_b_per_time: f64,
    final_time: f64,
    time_step: f64,
    extinction_threshold_fraction: f64,
    record_every_steps: u32,
    maximum_cell_species_steps: u64,
    out: PathBuf,
) -> Result<()> {
    let metadata = fs::metadata(&input).map_err(|source| MarklabError::io(&input, source))?;
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(MarklabError::Validation(
            "spatial-competition input exceeds 16 MiB".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(&input)
        .map_err(|error| {
            MarklabError::Validation(format!("spatial-competition CSV failed: {error}"))
        })?;
    if !reader
        .headers()?
        .iter()
        .eq(["position_um", "density_a", "density_b"])
    {
        return Err(MarklabError::Validation(
            "spatial-competition CSV header differs".into(),
        ));
    }
    let initial = reader
        .deserialize::<InputRow>()
        .map(|row| {
            row.map(|row| CompetitionInitialPoint {
                position_um: row.position_um,
                density_a: row.density_a,
                density_b: row.density_b,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| {
            MarklabError::Validation(format!("spatial-competition CSV failed: {error}"))
        })?;
    let result = simulate_spatial_competition(CompetitionSpec {
        initial,
        diffusion_a_um2_per_time,
        diffusion_b_um2_per_time,
        growth_a_per_time,
        growth_b_per_time,
        carrying_a,
        carrying_b,
        competition_a_from_b,
        competition_b_from_a,
        treatment_a_per_time,
        treatment_b_per_time,
        final_time,
        time_step,
        extinction_threshold_fraction,
        record_every_steps,
        maximum_cell_species_steps,
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
