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

/// Arguments of `marklab simulate growth-front`.
#[derive(clap::Args, Debug)]
pub struct GrowthFrontArgs {
    /// Initial density CSV with position_um and density columns.
    #[arg(long)]
    input: PathBuf,
    /// Diffusion coefficient in square micrometers per time unit.
    #[arg(long)]
    diffusion_um2_per_time: f64,
    /// Logistic growth rate per time unit.
    #[arg(long)]
    growth_rate_per_time: f64,
    /// Density at the logistic carrying capacity.
    #[arg(long)]
    carrying_capacity: f64,
    /// Time at which to stop the simulation.
    #[arg(long)]
    final_time: f64,
    /// Time increment for each solver step.
    #[arg(long)]
    time_step: f64,
    /// Fraction of carrying capacity used to locate the growth front.
    #[arg(long)]
    front_threshold_fraction: f64,
    /// Number of solver steps between recorded snapshots.
    #[arg(long)]
    record_every_steps: u32,
    /// Maximum number of grid-cell time-step updates.
    #[arg(long)]
    maximum_cell_steps: u64,
    /// File to write the result to.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: GrowthFrontArgs) -> Result<()> {
    let GrowthFrontArgs {
        input,
        diffusion_um2_per_time,
        growth_rate_per_time,
        carrying_capacity,
        final_time,
        time_step,
        front_threshold_fraction,
        record_every_steps,
        maximum_cell_steps,
        out,
    } = args;
    let bytes = super::read_input(&input, "growth-front")?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
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
