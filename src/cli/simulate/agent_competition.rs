use std::{fs, path::PathBuf};

use marklab_simulation::{
    simulate_agent_competition, Agent, AgentCompetitionSpec, RectangularAgentWindow,
    SimulationError, SpeciesRates,
};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    window: RectangularAgentWindow,
    agents: Vec<Agent>,
    species_a: SpeciesRates,
    species_b: SpeciesRates,
    competition_radius_um: f64,
    competition_death_per_opposite_neighbor: f64,
    birth_jitter_sd_um: f64,
    move_sd_um: f64,
    final_time: f64,
}

/// Arguments of `marklab simulate agent-competition`.
#[derive(clap::Args, Debug)]
pub struct AgentCompetitionArgs {
    /// JSON with window, agents, species_a/species_b rates, competition and movement settings.
    #[arg(long)]
    input: PathBuf,
    /// Random seed; the same seed gives the same result.
    #[arg(long)]
    seed: u64,
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: u32,
    /// Upper bound on agents.
    #[arg(long)]
    maximum_agents: u32,
    /// Maximum number of cell pairs visited.
    #[arg(long)]
    maximum_pair_visits: u64,
    /// Maximum event-log entries retained in the simulation result.
    #[arg(long)]
    retain_events: u32,
    /// File to write the result to.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: AgentCompetitionArgs) -> Result<()> {
    let AgentCompetitionArgs {
        input,
        seed,
        maximum_events,
        maximum_agents,
        maximum_pair_visits,
        retain_events,
        out,
    } = args;
    let bytes = super::read_input(&input, "agent-competition")?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = simulate_agent_competition(AgentCompetitionSpec {
        window: input.window,
        agents: input.agents,
        species_a: input.species_a,
        species_b: input.species_b,
        competition_radius_um: input.competition_radius_um,
        competition_death_per_opposite_neighbor: input.competition_death_per_opposite_neighbor,
        birth_jitter_sd_um: input.birth_jitter_sd_um,
        move_sd_um: input.move_sd_um,
        final_time: input.final_time,
        seed,
        maximum_events,
        maximum_agents,
        maximum_pair_visits,
        retain_events,
    })
    .map_err(map)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    fs::write(&out, serde_json::to_vec_pretty(&result)?)
        .map_err(|source| MarklabError::io(&out, source))
}

fn map(error: SimulationError) -> MarklabError {
    MarklabError::Validation(error.to_string())
}
