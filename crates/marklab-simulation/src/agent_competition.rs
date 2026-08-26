use std::collections::HashSet;

use rand::{distributions::Open01, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::StandardNormal;
use serde::{Deserialize, Serialize};

use super::SimulationError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSpecies {
    A,
    B,
}

impl AgentSpecies {
    fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub agent_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub species: AgentSpecies,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularAgentWindow {
    pub xmin_um: f64,
    pub ymin_um: f64,
    pub xmax_um: f64,
    pub ymax_um: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesRates {
    pub birth_rate: f64,
    pub death_rate: f64,
    pub move_rate: f64,
    pub switch_rate: f64,
}

#[derive(Clone, Debug)]
pub struct AgentCompetitionSpec {
    pub window: RectangularAgentWindow,
    pub agents: Vec<Agent>,
    pub species_a: SpeciesRates,
    pub species_b: SpeciesRates,
    pub competition_radius_um: f64,
    pub competition_death_per_opposite_neighbor: f64,
    pub birth_jitter_sd_um: f64,
    pub move_sd_um: f64,
    pub final_time: f64,
    pub seed: u64,
    pub maximum_events: u32,
    pub maximum_agents: u32,
    pub maximum_pair_visits: u64,
    pub retain_events: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentCompetitionEvent {
    pub event_index: u32,
    pub time: f64,
    pub event: &'static str,
    pub agent_id: String,
    pub related_agent_id: Option<String>,
    pub species_before: AgentSpecies,
    pub species_after: Option<AgentSpecies>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AgentCompetitionEventCounts {
    pub birth: u32,
    pub death: u32,
    pub move_event: u32,
    pub switch: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentCompetitionResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub window: RectangularAgentWindow,
    pub seed: u64,
    pub final_time: f64,
    pub simulated_time: f64,
    pub initial_agent_count: u32,
    pub final_agent_count: u32,
    pub final_species_a_count: u32,
    pub final_species_b_count: u32,
    pub maximum_agents_observed: u32,
    pub completed_events: u32,
    pub event_counts: AgentCompetitionEventCounts,
    pub retained_event_count: u32,
    pub events_truncated: bool,
    pub event_log: Vec<AgentCompetitionEvent>,
    pub final_agents: Vec<Agent>,
    pub pair_visits: u64,
    pub maximum_events: u32,
    pub maximum_agents: u32,
    pub maximum_pair_visits: u64,
    pub termination: &'static str,
    pub neighborhood_algorithm: &'static str,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

pub fn simulate_agent_competition(
    spec: AgentCompetitionSpec,
) -> Result<AgentCompetitionResult, SimulationError> {
    validate(&spec)?;
    let initial_agent_count = spec.agents.len() as u32;
    let mut agents = spec.agents.clone();
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut time = 0.0;
    let mut completed_events = 0_u32;
    let mut pair_visits = 0_u64;
    let mut maximum_agents_observed = agents.len() as u32;
    let mut event_counts = AgentCompetitionEventCounts::default();
    let mut event_log = Vec::new();
    let mut next_birth_id = 1_u64;
    let termination = loop {
        if agents.is_empty() {
            break "all_agents_removed";
        }
        if completed_events == spec.maximum_events {
            break "maximum_events_reached";
        }
        let (rates, total_rate, visits) = rates(&agents, &spec)?;
        pair_visits = pair_visits
            .checked_add(visits)
            .ok_or_else(|| SimulationError::Numerical("pair-visit count overflowed".into()))?;
        if pair_visits > spec.maximum_pair_visits {
            return Err(SimulationError::Invalid(format!(
                "{pair_visits} pair visits exceed the declared resource bound"
            )));
        }
        if total_rate == 0.0 {
            break "no_active_rates";
        }
        let wait_uniform: f64 = rng.sample(Open01);
        let wait = -wait_uniform.ln() / total_rate;
        if time + wait > spec.final_time {
            time = spec.final_time;
            break "final_time_reached";
        }
        time += wait;
        let selection = rng.gen::<f64>() * total_rate;
        let (agent_index, event_kind) = select_event(&rates, selection).ok_or_else(|| {
            SimulationError::Numerical("failed to select a positive-rate event".into())
        })?;
        completed_events += 1;
        let record = apply_event(
            &mut agents,
            agent_index,
            event_kind,
            time,
            completed_events,
            &spec,
            &mut rng,
            &mut next_birth_id,
            &mut event_counts,
        )?;
        if event_log.len() < spec.retain_events as usize {
            event_log.push(record);
        }
        maximum_agents_observed = maximum_agents_observed.max(agents.len() as u32);
    };
    let final_species_a_count = agents
        .iter()
        .filter(|agent| agent.species == AgentSpecies::A)
        .count() as u32;
    let final_species_b_count = agents.len() as u32 - final_species_a_count;
    let mut final_agents = agents;
    final_agents.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
    Ok(AgentCompetitionResult {
        format: "marklab.agent_competition",
        version: 1,
        simulator_id: "two_species_spatial_gillespie_exact_pair_scan",
        simulator_version: 1,
        window: spec.window,
        seed: spec.seed,
        final_time: spec.final_time,
        simulated_time: time,
        initial_agent_count,
        final_agent_count: final_agents.len() as u32,
        final_species_a_count,
        final_species_b_count,
        maximum_agents_observed,
        completed_events,
        event_counts,
        retained_event_count: event_log.len() as u32,
        events_truncated: completed_events > event_log.len() as u32,
        event_log,
        final_agents,
        pair_visits,
        maximum_events: spec.maximum_events,
        maximum_agents: spec.maximum_agents,
        maximum_pair_visits: spec.maximum_pair_visits,
        termination,
        neighborhood_algorithm: "bounded_exact_pair_scan_recomputed_each_event",
        random_seed_namespace: "agent_competition_v1_chacha20",
        claim_status: "experimental_agent_simulation_not_evolutionary_or_treatment_truth",
    })
}

fn rates(
    agents: &[Agent],
    spec: &AgentCompetitionSpec,
) -> Result<(Vec<[f64; 4]>, f64, u64), SimulationError> {
    let mut opposite_neighbors = vec![0_u32; agents.len()];
    let radius_squared = spec.competition_radius_um.powi(2);
    let mut visits = 0_u64;
    for left in 0..agents.len() {
        for right in left + 1..agents.len() {
            visits += 1;
            let dx = agents[left].x_um - agents[right].x_um;
            let dy = agents[left].y_um - agents[right].y_um;
            if agents[left].species != agents[right].species
                && dx.mul_add(dx, dy * dy) <= radius_squared
            {
                opposite_neighbors[left] += 1;
                opposite_neighbors[right] += 1;
            }
        }
    }
    let at_capacity = agents.len() >= spec.maximum_agents as usize;
    let mut total = 0.0;
    let rates = agents
        .iter()
        .zip(opposite_neighbors)
        .map(|(agent, neighbors)| {
            let base = match agent.species {
                AgentSpecies::A => spec.species_a,
                AgentSpecies::B => spec.species_b,
            };
            let values = [
                if at_capacity { 0.0 } else { base.birth_rate },
                base.death_rate
                    + spec.competition_death_per_opposite_neighbor * f64::from(neighbors),
                base.move_rate,
                base.switch_rate,
            ];
            total += values.iter().sum::<f64>();
            values
        })
        .collect::<Vec<_>>();
    if !total.is_finite() {
        return Err(SimulationError::Numerical(
            "agent event rate became non-finite".into(),
        ));
    }
    Ok((rates, total, visits))
}

fn select_event(rates: &[[f64; 4]], mut selection: f64) -> Option<(usize, usize)> {
    let mut last_positive = None;
    for (agent_index, agent_rates) in rates.iter().enumerate() {
        for (event_kind, rate) in agent_rates.iter().enumerate() {
            if *rate > 0.0 {
                last_positive = Some((agent_index, event_kind));
                if selection < *rate {
                    return last_positive;
                }
                selection -= *rate;
            }
        }
    }
    last_positive
}

#[allow(clippy::too_many_arguments)]
fn apply_event(
    agents: &mut Vec<Agent>,
    index: usize,
    event_kind: usize,
    time: f64,
    event_index: u32,
    spec: &AgentCompetitionSpec,
    rng: &mut ChaCha20Rng,
    next_birth_id: &mut u64,
    counts: &mut AgentCompetitionEventCounts,
) -> Result<AgentCompetitionEvent, SimulationError> {
    let before = agents[index].clone();
    match event_kind {
        0 => {
            counts.birth += 1;
            let id = format!("sim-{}", *next_birth_id);
            *next_birth_id = next_birth_id
                .checked_add(1)
                .ok_or_else(|| SimulationError::Numerical("birth ID overflowed".into()))?;
            let x = reflect(
                before.x_um + normal(rng) * spec.birth_jitter_sd_um,
                spec.window.xmin_um,
                spec.window.xmax_um,
            );
            let y = reflect(
                before.y_um + normal(rng) * spec.birth_jitter_sd_um,
                spec.window.ymin_um,
                spec.window.ymax_um,
            );
            agents.push(Agent {
                agent_id: id.clone(),
                x_um: x,
                y_um: y,
                species: before.species,
            });
            Ok(event_record(
                event_index,
                time,
                "birth",
                before.agent_id,
                Some(id),
                before.species,
                Some(before.species),
            ))
        }
        1 => {
            counts.death += 1;
            agents.remove(index);
            Ok(event_record(
                event_index,
                time,
                "death",
                before.agent_id,
                None,
                before.species,
                None,
            ))
        }
        2 => {
            counts.move_event += 1;
            agents[index].x_um = reflect(
                before.x_um + normal(rng) * spec.move_sd_um,
                spec.window.xmin_um,
                spec.window.xmax_um,
            );
            agents[index].y_um = reflect(
                before.y_um + normal(rng) * spec.move_sd_um,
                spec.window.ymin_um,
                spec.window.ymax_um,
            );
            Ok(event_record(
                event_index,
                time,
                "move",
                before.agent_id,
                None,
                before.species,
                Some(before.species),
            ))
        }
        3 => {
            counts.switch += 1;
            agents[index].species = before.species.other();
            Ok(event_record(
                event_index,
                time,
                "switch",
                before.agent_id,
                None,
                before.species,
                Some(agents[index].species),
            ))
        }
        _ => Err(SimulationError::Numerical(
            "agent event kind is invalid".into(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn event_record(
    event_index: u32,
    time: f64,
    event: &'static str,
    agent_id: String,
    related_agent_id: Option<String>,
    species_before: AgentSpecies,
    species_after: Option<AgentSpecies>,
) -> AgentCompetitionEvent {
    AgentCompetitionEvent {
        event_index,
        time,
        event,
        agent_id,
        related_agent_id,
        species_before,
        species_after,
    }
}

fn normal(rng: &mut ChaCha20Rng) -> f64 {
    rng.sample(StandardNormal)
}

fn reflect(value: f64, minimum: f64, maximum: f64) -> f64 {
    let width = maximum - minimum;
    let phase = (value - minimum).rem_euclid(2.0 * width);
    if phase <= width {
        minimum + phase
    } else {
        maximum - (phase - width)
    }
}

fn validate(spec: &AgentCompetitionSpec) -> Result<(), SimulationError> {
    let rates = [
        spec.species_a.birth_rate,
        spec.species_a.death_rate,
        spec.species_a.move_rate,
        spec.species_a.switch_rate,
        spec.species_b.birth_rate,
        spec.species_b.death_rate,
        spec.species_b.move_rate,
        spec.species_b.switch_rate,
        spec.competition_death_per_opposite_neighbor,
    ];
    let controls_valid = !spec.agents.is_empty()
        && spec.agents.len() <= spec.maximum_agents as usize
        && rates.iter().all(|rate| rate.is_finite() && *rate >= 0.0)
        && spec.window.xmin_um.is_finite()
        && spec.window.ymin_um.is_finite()
        && spec.window.xmax_um.is_finite()
        && spec.window.ymax_um.is_finite()
        && spec.window.xmin_um < spec.window.xmax_um
        && spec.window.ymin_um < spec.window.ymax_um
        && spec.competition_radius_um.is_finite()
        && spec.competition_radius_um > 0.0
        && spec.birth_jitter_sd_um.is_finite()
        && spec.birth_jitter_sd_um >= 0.0
        && spec.move_sd_um.is_finite()
        && spec.move_sd_um >= 0.0
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && (1..=1_000_000).contains(&spec.maximum_events)
        && (1..=1_000_000).contains(&spec.maximum_agents)
        && (1..=250_000_000).contains(&spec.maximum_pair_visits)
        && spec.retain_events <= spec.maximum_events
        && spec.retain_events <= 100_000;
    if !controls_valid {
        return Err(SimulationError::Invalid(
            "agent competition controls are invalid".into(),
        ));
    }
    let mut ids = HashSet::new();
    if spec.agents.iter().any(|agent| {
        agent.agent_id.trim().is_empty()
            || agent.agent_id.trim() != agent.agent_id
            || agent.agent_id.starts_with("sim-")
            || !ids.insert(agent.agent_id.as_str())
            || !agent.x_um.is_finite()
            || !agent.y_um.is_finite()
            || !(spec.window.xmin_um..=spec.window.xmax_um).contains(&agent.x_um)
            || !(spec.window.ymin_um..=spec.window.ymax_um).contains(&agent.y_um)
    }) {
        return Err(SimulationError::Invalid(
            "agent identities or coordinates are invalid".into(),
        ));
    }
    Ok(())
}
