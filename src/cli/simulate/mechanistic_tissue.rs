use std::{fs, path::PathBuf};

use marklab_simulation::{
    simulate_mechanistic_tissue, Agent, AgentCompetitionSpec, MechanisticTissueSpec,
    RectangularAgentWindow, SimulationError, SpeciesRates, UptakeCell, VascularTransportSpec,
    VesselSource,
};
use serde::Deserialize;

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    grid_x: u32,
    grid_y: u32,
    spacing_x_um: f64,
    spacing_y_um: f64,
    initial_density_row_major: Vec<f64>,
    initial_phi_row_major: Vec<f64>,
    vascular: VascularInput,
    agents: AgentInput,
    coupling: CouplingInput,
    numerics: NumericsInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VascularInput {
    initial_concentration_row_major: Vec<f64>,
    diffusion_um2_per_time_row_major: Vec<f64>,
    velocity_x_um_per_time_row_major: Vec<f64>,
    velocity_y_um_per_time_row_major: Vec<f64>,
    flow_approximation: String,
    vessel_sources: Vec<VesselSource>,
    uptake_cells: Vec<UptakeCell>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentInput {
    window: RectangularAgentWindow,
    initial: Vec<Agent>,
    species_a: SpeciesRates,
    species_b: SpeciesRates,
    competition_radius_um: f64,
    competition_death_per_opposite_neighbor: f64,
    birth_jitter_sd_um: f64,
    move_sd_um: f64,
    seed: u64,
    maximum_events_per_interval: u32,
    maximum_agents: u32,
    maximum_pair_visits_per_interval: u64,
    retain_events_per_interval: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CouplingInput {
    oxygen_half_saturation: f64,
    base_density_growth_rate_per_time: f64,
    density_carrying_capacity: f64,
    density_diffusion_um2_per_time: f64,
    interface_base_speed_um_per_time: f64,
    interface_density_speed_weight: f64,
    interface_oxygen_speed_weight: f64,
    interface_curvature_weight_um2_per_time: f64,
    agent_hypoxia_death_rate: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumericsInput {
    coupled_intervals: u32,
    interval_time: f64,
    vascular_time_step: f64,
    density_time_step: f64,
    interface_time_step: f64,
    hypoxia_threshold: f64,
    maximum_cell_steps_per_module_interval: u64,
    maximum_reinitialization_distance_visits_per_interval: u64,
}

pub(crate) fn run(input_path: PathBuf, output_path: PathBuf) -> Result<()> {
    let metadata =
        fs::metadata(&input_path).map_err(|source| MarklabError::io(&input_path, source))?;
    if metadata.len() > 16 * 1024 * 1024 {
        return Err(MarklabError::Validation(
            "mechanistic-tissue input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| MarklabError::io(&input_path, source))?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = simulate_mechanistic_tissue(MechanisticTissueSpec {
        grid_x: input.grid_x,
        grid_y: input.grid_y,
        spacing_x_um: input.spacing_x_um,
        spacing_y_um: input.spacing_y_um,
        initial_density_row_major: input.initial_density_row_major,
        initial_phi_row_major: input.initial_phi_row_major,
        vascular: VascularTransportSpec {
            grid_x: input.grid_x,
            grid_y: input.grid_y,
            spacing_x_um: input.spacing_x_um,
            spacing_y_um: input.spacing_y_um,
            initial_concentration_row_major: input.vascular.initial_concentration_row_major,
            diffusion_um2_per_time_row_major: input.vascular.diffusion_um2_per_time_row_major,
            velocity_x_um_per_time_row_major: input.vascular.velocity_x_um_per_time_row_major,
            velocity_y_um_per_time_row_major: input.vascular.velocity_y_um_per_time_row_major,
            flow_approximation: input.vascular.flow_approximation,
            vessel_sources: input.vascular.vessel_sources,
            uptake_cells: input.vascular.uptake_cells,
            final_time: input.numerics.interval_time,
            time_step: input.numerics.vascular_time_step,
            hypoxia_threshold: input.numerics.hypoxia_threshold,
            record_every_steps: u32::MAX,
            maximum_cell_steps: input.numerics.maximum_cell_steps_per_module_interval,
        },
        agents: AgentCompetitionSpec {
            window: input.agents.window,
            agents: input.agents.initial,
            species_a: input.agents.species_a,
            species_b: input.agents.species_b,
            competition_radius_um: input.agents.competition_radius_um,
            competition_death_per_opposite_neighbor: input
                .agents
                .competition_death_per_opposite_neighbor,
            birth_jitter_sd_um: input.agents.birth_jitter_sd_um,
            move_sd_um: input.agents.move_sd_um,
            final_time: input.numerics.interval_time,
            seed: input.agents.seed,
            maximum_events: input.agents.maximum_events_per_interval,
            maximum_agents: input.agents.maximum_agents,
            maximum_pair_visits: input.agents.maximum_pair_visits_per_interval,
            retain_events: input.agents.retain_events_per_interval,
        },
        oxygen_half_saturation: input.coupling.oxygen_half_saturation,
        base_density_growth_rate_per_time: input.coupling.base_density_growth_rate_per_time,
        density_carrying_capacity: input.coupling.density_carrying_capacity,
        density_diffusion_um2_per_time: input.coupling.density_diffusion_um2_per_time,
        interface_base_speed_um_per_time: input.coupling.interface_base_speed_um_per_time,
        interface_density_speed_weight: input.coupling.interface_density_speed_weight,
        interface_oxygen_speed_weight: input.coupling.interface_oxygen_speed_weight,
        interface_curvature_weight_um2_per_time: input
            .coupling
            .interface_curvature_weight_um2_per_time,
        agent_hypoxia_death_rate: input.coupling.agent_hypoxia_death_rate,
        coupled_intervals: input.numerics.coupled_intervals,
        interval_time: input.numerics.interval_time,
        vascular_time_step: input.numerics.vascular_time_step,
        density_time_step: input.numerics.density_time_step,
        interface_time_step: input.numerics.interface_time_step,
        hypoxia_threshold: input.numerics.hypoxia_threshold,
        maximum_cell_steps_per_module_interval: input
            .numerics
            .maximum_cell_steps_per_module_interval,
        maximum_reinitialization_distance_visits_per_interval: input
            .numerics
            .maximum_reinitialization_distance_visits_per_interval,
    })
    .map_err(map)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    fs::write(&output_path, serde_json::to_vec_pretty(&result)?)
        .map_err(|source| MarklabError::io(&output_path, source))
}

fn map(error: SimulationError) -> MarklabError {
    MarklabError::Validation(error.to_string())
}
