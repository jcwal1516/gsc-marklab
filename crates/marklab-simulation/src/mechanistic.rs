use serde::Serialize;

use super::{
    evolve_interface_level_set, simulate_agent_competition, simulate_reaction_diffusion,
    simulate_vascular_transport, Agent, AgentCompetitionSpec, LevelSetSpec, LevelSetStateCell,
    ReactionDiffusionSpec, ReactionDiffusionStateCell, ReactionModel, SimulationError,
    VascularTransportSpec, VascularTransportStateCell,
};

#[derive(Clone, Debug)]
pub struct MechanisticTissueSpec {
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub initial_density_row_major: Vec<f64>,
    pub initial_phi_row_major: Vec<f64>,
    pub vascular: VascularTransportSpec,
    pub agents: AgentCompetitionSpec,
    pub oxygen_half_saturation: f64,
    pub base_density_growth_rate_per_time: f64,
    pub density_carrying_capacity: f64,
    pub density_diffusion_um2_per_time: f64,
    pub interface_base_speed_um_per_time: f64,
    pub interface_density_speed_weight: f64,
    pub interface_oxygen_speed_weight: f64,
    pub interface_curvature_weight_um2_per_time: f64,
    pub agent_hypoxia_death_rate: f64,
    pub coupled_intervals: u32,
    pub interval_time: f64,
    pub vascular_time_step: f64,
    pub density_time_step: f64,
    pub interface_time_step: f64,
    pub hypoxia_threshold: f64,
    pub maximum_cell_steps_per_module_interval: u64,
    pub maximum_reinitialization_distance_visits_per_interval: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MechanisticIntervalSummary {
    pub interval: u32,
    pub elapsed_time: f64,
    pub mean_oxygen: f64,
    pub oxygen_saturation: f64,
    pub mean_density: f64,
    pub effective_density_growth_rate_per_time: f64,
    pub mean_interface_speed_um_per_time: f64,
    pub agent_count: u32,
    pub agent_events: u32,
    pub vascular_mass_balance_residual: f64,
    pub vascular_cell_steps: u64,
    pub density_cell_steps: u64,
    pub interface_cell_steps: u64,
    pub agent_pair_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MechanisticCouplingDiagnostics {
    pub coupling_order: [&'static str; 4],
    pub completed_intervals: u32,
    pub maximum_vascular_mass_balance_residual: f64,
    pub total_vascular_cell_steps: u64,
    pub total_density_cell_steps: u64,
    pub total_interface_cell_steps: u64,
    pub total_agent_events: u64,
    pub total_agent_pair_visits: u64,
    pub grid_alignment_violations: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MechanisticTissueResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub total_time: f64,
    pub intervals: Vec<MechanisticIntervalSummary>,
    pub final_oxygen: Vec<VascularTransportStateCell>,
    pub final_density: Vec<ReactionDiffusionStateCell>,
    pub final_interface: Vec<LevelSetStateCell>,
    pub final_agents: Vec<Agent>,
    pub coupling_diagnostics: MechanisticCouplingDiagnostics,
    pub observation_model: &'static str,
    pub claim_status: &'static str,
}

pub fn simulate_mechanistic_tissue(
    spec: MechanisticTissueSpec,
) -> Result<MechanisticTissueResult, SimulationError> {
    validate(&spec)?;
    let cells = spec.grid_x as usize * spec.grid_y as usize;
    let mut oxygen = spec.vascular.initial_concentration_row_major.clone();
    let mut density = spec.initial_density_row_major.clone();
    let mut phi = spec.initial_phi_row_major.clone();
    let mut agents = spec.agents.agents.clone();
    let mut intervals = Vec::with_capacity(spec.coupled_intervals as usize);
    let mut final_oxygen = Vec::new();
    let mut final_density = Vec::new();
    let mut final_interface = Vec::new();
    let mut maximum_mass_residual = 0.0_f64;
    let mut total_vascular_steps = 0_u64;
    let mut total_density_steps = 0_u64;
    let mut total_interface_steps = 0_u64;
    let mut total_agent_events = 0_u64;
    let mut total_pair_visits = 0_u64;

    for interval_index in 0..spec.coupled_intervals {
        let mut vascular_spec = spec.vascular.clone();
        vascular_spec.initial_concentration_row_major = oxygen;
        vascular_spec.final_time = spec.interval_time;
        vascular_spec.time_step = spec.vascular_time_step;
        vascular_spec.hypoxia_threshold = spec.hypoxia_threshold;
        vascular_spec.record_every_steps = u32::MAX;
        vascular_spec.maximum_cell_steps = spec.maximum_cell_steps_per_module_interval;
        let vascular = simulate_vascular_transport(vascular_spec)?;
        oxygen = vascular
            .final_state
            .iter()
            .map(|row| row.concentration)
            .collect();
        let mean_oxygen = mean(&oxygen);
        let oxygen_saturation = mean_oxygen / (mean_oxygen + spec.oxygen_half_saturation);
        let effective_growth = spec.base_density_growth_rate_per_time * oxygen_saturation;

        let density_result = simulate_reaction_diffusion(ReactionDiffusionSpec {
            grid_x: spec.grid_x,
            grid_y: spec.grid_y,
            spacing_x_um: spec.spacing_x_um,
            spacing_y_um: spec.spacing_y_um,
            initial_row_major: density,
            diffusion_um2_per_time: spec.density_diffusion_um2_per_time,
            reaction: ReactionModel::Logistic {
                rate_per_time: effective_growth,
                carrying_capacity: spec.density_carrying_capacity,
            },
            final_time: spec.interval_time,
            time_step: spec.density_time_step,
            record_every_steps: u32::MAX,
            maximum_cell_steps: spec.maximum_cell_steps_per_module_interval,
        })?;
        density = density_result
            .final_state
            .iter()
            .map(|row| row.value)
            .collect();
        let mean_density = mean(&density);
        let speeds = density
            .iter()
            .zip(&oxygen)
            .map(|(density, oxygen)| {
                spec.interface_base_speed_um_per_time
                    + spec.interface_density_speed_weight * density
                    + spec.interface_oxygen_speed_weight * oxygen
            })
            .collect::<Vec<_>>();
        if speeds.iter().any(|value| !value.is_finite()) {
            return Err(SimulationError::Numerical(
                "mechanistic interface coupling became non-finite".into(),
            ));
        }
        let mean_interface_speed = mean(&speeds);
        let interface = evolve_interface_level_set(LevelSetSpec {
            grid_x: spec.grid_x,
            grid_y: spec.grid_y,
            spacing_x_um: spec.spacing_x_um,
            spacing_y_um: spec.spacing_y_um,
            initial_phi_row_major: phi,
            normal_speed_um_per_time_row_major: speeds,
            curvature_weight_um2_per_time: spec.interface_curvature_weight_um2_per_time,
            final_time: spec.interval_time,
            time_step: spec.interface_time_step,
            reinitialize_every_steps: 0,
            record_every_steps: u32::MAX,
            maximum_cell_steps: spec.maximum_cell_steps_per_module_interval,
            maximum_reinitialization_distance_visits: spec
                .maximum_reinitialization_distance_visits_per_interval,
        })?;
        phi = interface.final_state.iter().map(|row| row.phi).collect();

        let (agent_events, agent_pair_visits) = if agents.is_empty() {
            (0, 0)
        } else {
            let mut agent_spec = spec.agents.clone();
            agent_spec.agents = agents;
            agent_spec.final_time = spec.interval_time;
            agent_spec.seed = interval_seed(spec.agents.seed, interval_index);
            agent_spec.species_a = coupled_rates(
                spec.agents.species_a,
                oxygen_saturation,
                spec.agent_hypoxia_death_rate,
            );
            agent_spec.species_b = coupled_rates(
                spec.agents.species_b,
                oxygen_saturation,
                spec.agent_hypoxia_death_rate,
            );
            let agent_result = simulate_agent_competition(agent_spec)?;
            agents = agent_result.final_agents;
            (agent_result.completed_events, agent_result.pair_visits)
        };

        maximum_mass_residual =
            maximum_mass_residual.max(vascular.solver.mass_balance_residual.abs());
        total_vascular_steps = checked_add(total_vascular_steps, vascular.solver.cell_steps)?;
        total_density_steps = checked_add(total_density_steps, density_result.solver.cell_steps)?;
        total_interface_steps = checked_add(total_interface_steps, interface.solver.cell_steps)?;
        total_agent_events = checked_add(total_agent_events, u64::from(agent_events))?;
        total_pair_visits = checked_add(total_pair_visits, agent_pair_visits)?;
        intervals.push(MechanisticIntervalSummary {
            interval: interval_index + 1,
            elapsed_time: f64::from(interval_index + 1) * spec.interval_time,
            mean_oxygen,
            oxygen_saturation,
            mean_density,
            effective_density_growth_rate_per_time: effective_growth,
            mean_interface_speed_um_per_time: mean_interface_speed,
            agent_count: agents.len() as u32,
            agent_events,
            vascular_mass_balance_residual: vascular.solver.mass_balance_residual,
            vascular_cell_steps: vascular.solver.cell_steps,
            density_cell_steps: density_result.solver.cell_steps,
            interface_cell_steps: interface.solver.cell_steps,
            agent_pair_visits,
        });
        final_oxygen = vascular.final_state;
        final_density = density_result.final_state;
        final_interface = interface.final_state;
    }
    debug_assert_eq!(final_oxygen.len(), cells);
    Ok(MechanisticTissueResult {
        format: "marklab.mechanistic_tissue",
        version: 1,
        simulator_id: "one_way_interval_coupled_mechanistic_tissue",
        simulator_version: 1,
        dimensionality: 2,
        grid_x: spec.grid_x,
        grid_y: spec.grid_y,
        spacing_x_um: spec.spacing_x_um,
        spacing_y_um: spec.spacing_y_um,
        total_time: f64::from(spec.coupled_intervals) * spec.interval_time,
        intervals,
        final_oxygen,
        final_density,
        final_interface,
        final_agents: agents,
        coupling_diagnostics: MechanisticCouplingDiagnostics {
            coupling_order: ["vascular", "density", "interface", "agents"],
            completed_intervals: spec.coupled_intervals,
            maximum_vascular_mass_balance_residual: maximum_mass_residual,
            total_vascular_cell_steps: total_vascular_steps,
            total_density_cell_steps: total_density_steps,
            total_interface_cell_steps: total_interface_steps,
            total_agent_events,
            total_agent_pair_visits: total_pair_visits,
            grid_alignment_violations: 0,
        },
        observation_model: "identity_latent_fields_and_agents",
        claim_status: "experimental_coupled_simulation_not_digital_twin",
    })
}

fn validate(spec: &MechanisticTissueSpec) -> Result<(), SimulationError> {
    let cells = spec.grid_x as usize * spec.grid_y as usize;
    let grids_align = spec.grid_x == spec.vascular.grid_x
        && spec.grid_y == spec.vascular.grid_y
        && spec.spacing_x_um == spec.vascular.spacing_x_um
        && spec.spacing_y_um == spec.vascular.spacing_y_um
        && spec.initial_density_row_major.len() == cells
        && spec.initial_phi_row_major.len() == cells
        && spec.agents.window.xmin_um == 0.0
        && spec.agents.window.ymin_um == 0.0
        && spec.agents.window.xmax_um == f64::from(spec.grid_x - 1) * spec.spacing_x_um
        && spec.agents.window.ymax_um == f64::from(spec.grid_y - 1) * spec.spacing_y_um;
    let controls = [
        spec.spacing_x_um,
        spec.spacing_y_um,
        spec.oxygen_half_saturation,
        spec.density_carrying_capacity,
        spec.interval_time,
        spec.vascular_time_step,
        spec.density_time_step,
        spec.interface_time_step,
    ];
    let rates = [
        spec.base_density_growth_rate_per_time,
        spec.density_diffusion_um2_per_time,
        spec.interface_base_speed_um_per_time,
        spec.interface_density_speed_weight,
        spec.interface_oxygen_speed_weight,
        spec.interface_curvature_weight_um2_per_time,
        spec.agent_hypoxia_death_rate,
        spec.hypoxia_threshold,
    ];
    let coupled_cell_work = spec
        .maximum_cell_steps_per_module_interval
        .checked_mul(u64::from(spec.coupled_intervals))
        .and_then(|value| value.checked_mul(3));
    let coupled_agent_events =
        u64::from(spec.agents.maximum_events).checked_mul(u64::from(spec.coupled_intervals));
    let coupled_pair_visits = spec
        .agents
        .maximum_pair_visits
        .checked_mul(u64::from(spec.coupled_intervals));
    if !grids_align
        || !(1..=32).contains(&spec.coupled_intervals)
        || controls
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        || rates.iter().any(|value| !value.is_finite() || *value < 0.0)
        || !(1..=250_000_000).contains(&spec.maximum_cell_steps_per_module_interval)
        || spec.maximum_reinitialization_distance_visits_per_interval > 250_000_000
        || coupled_cell_work.is_none_or(|value| value > 250_000_000)
        || coupled_agent_events.is_none_or(|value| value > 250_000_000)
        || coupled_pair_visits.is_none_or(|value| value > 250_000_000)
    {
        return Err(SimulationError::Invalid(
            "mechanistic coupling grid, parameters, or resources are invalid".into(),
        ));
    }
    Ok(())
}

fn coupled_rates(
    rates: super::SpeciesRates,
    oxygen_saturation: f64,
    hypoxia_death_rate: f64,
) -> super::SpeciesRates {
    super::SpeciesRates {
        birth_rate: rates.birth_rate * oxygen_saturation,
        death_rate: rates.death_rate + hypoxia_death_rate * (1.0 - oxygen_saturation),
        move_rate: rates.move_rate,
        switch_rate: rates.switch_rate,
    }
}

fn interval_seed(seed: u64, interval: u32) -> u64 {
    seed ^ u64::from(interval + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn checked_add(left: u64, right: u64) -> Result<u64, SimulationError> {
    left.checked_add(right)
        .ok_or_else(|| SimulationError::Numerical("mechanistic work accounting overflowed".into()))
}
