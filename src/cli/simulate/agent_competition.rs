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

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    input: PathBuf,
    seed: u64,
    maximum_events: u32,
    maximum_agents: u32,
    maximum_pair_visits: u64,
    retain_events: u32,
    out: PathBuf,
) -> Result<()> {
    let metadata = fs::metadata(&input).map_err(|source| MarklabError::io(&input, source))?;
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(MarklabError::Validation(
            "agent-competition input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input).map_err(|source| MarklabError::io(&input, source))?;
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
