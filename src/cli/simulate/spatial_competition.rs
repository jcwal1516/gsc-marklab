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

/// Arguments of `marklab simulate spatial-competition`.
#[derive(clap::Args, Debug)]
pub struct SpatialCompetitionArgs {
    /// Initial density CSV with position_um, density_a, and density_b columns.
    #[arg(long)]
    input: PathBuf,
    /// Species A diffusion coefficient in square micrometers per time unit.
    #[arg(long)]
    diffusion_a_um2_per_time: f64,
    /// Species B diffusion coefficient in square micrometers per time unit.
    #[arg(long)]
    diffusion_b_um2_per_time: f64,
    /// Species A intrinsic growth rate per time unit.
    #[arg(long)]
    growth_a_per_time: f64,
    /// Species B intrinsic growth rate per time unit.
    #[arg(long)]
    growth_b_per_time: f64,
    /// Carrying-capacity density for species A.
    #[arg(long)]
    carrying_a: f64,
    /// Carrying-capacity density for species B.
    #[arg(long)]
    carrying_b: f64,
    /// Effect of species B density on competition experienced by species A.
    #[arg(long)]
    competition_a_from_b: f64,
    /// Effect of species A density on competition experienced by species B.
    #[arg(long)]
    competition_b_from_a: f64,
    /// Treatment-induced loss rate for species A per time unit.
    #[arg(long)]
    treatment_a_per_time: f64,
    /// Treatment-induced loss rate for species B per time unit.
    #[arg(long)]
    treatment_b_per_time: f64,
    /// Time at which to stop the simulation.
    #[arg(long)]
    final_time: f64,
    /// Time increment for each solver step.
    #[arg(long)]
    time_step: f64,
    /// Fraction of each carrying capacity used to flag extinction.
    #[arg(long)]
    extinction_threshold_fraction: f64,
    /// Number of solver steps between recorded snapshots.
    #[arg(long)]
    record_every_steps: u32,
    /// Maximum grid-cell updates summed over both species and all time steps.
    #[arg(long)]
    maximum_cell_species_steps: u64,
    /// File to write the result to.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: SpatialCompetitionArgs) -> Result<()> {
    let SpatialCompetitionArgs {
        input,
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
        out,
    } = args;
    let bytes = super::read_input(&input, "spatial-competition")?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
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
