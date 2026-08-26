#![forbid(unsafe_code)]

use serde::Serialize;
use thiserror::Error;

mod agent_competition;
mod competition;
mod level_set;
mod mechanistic;
mod reaction_diffusion;
mod summary_matching;
mod vascular_transport;

pub use agent_competition::{
    simulate_agent_competition, Agent, AgentCompetitionEvent, AgentCompetitionEventCounts,
    AgentCompetitionResult, AgentCompetitionSpec, AgentSpecies, RectangularAgentWindow,
    SpeciesRates,
};
pub use competition::{
    simulate_spatial_competition, CompetitionExtinctionEvent, CompetitionInitialPoint,
    CompetitionSnapshot, CompetitionSolverDiagnostics, CompetitionSpec, CompetitionStatePoint,
    SpatialCompetitionResult,
};
pub use level_set::{
    evolve_interface_level_set, LevelSetInterfaceDiagnostics, LevelSetResult, LevelSetSnapshot,
    LevelSetSolverDiagnostics, LevelSetSpec, LevelSetStateCell,
};
pub use mechanistic::{
    simulate_mechanistic_tissue, MechanisticCouplingDiagnostics, MechanisticIntervalSummary,
    MechanisticTissueResult, MechanisticTissueSpec,
};
pub use reaction_diffusion::{
    simulate_reaction_diffusion, ReactionDiffusionPatternDiagnostics, ReactionDiffusionResult,
    ReactionDiffusionSnapshot, ReactionDiffusionSolverDiagnostics, ReactionDiffusionSpec,
    ReactionDiffusionStateCell, ReactionModel,
};
pub use summary_matching::{
    soft_pair_histogram, summary_matching_loss, SoftHistogramNormalization, SoftPairHistogramBin,
    SoftPairHistogramResult, SummaryLossComponent, SummaryMatchingLossResult,
    SummaryMatchingResult, SummaryMatchingSpec, SummaryPoint, SummaryWindow,
};
pub use vascular_transport::{
    simulate_vascular_transport, MappedUptakeCell, MappedVesselSource, UptakeCell,
    VascularHypoxicRegion, VascularTransportResult, VascularTransportSnapshot,
    VascularTransportSolverDiagnostics, VascularTransportSpec, VascularTransportStateCell,
    VesselSource,
};

#[derive(Clone, Debug)]
pub struct GrowthFrontInitialPoint {
    pub position_um: f64,
    pub density: f64,
}

#[derive(Clone, Debug)]
pub struct GrowthFrontSpec {
    pub initial: Vec<GrowthFrontInitialPoint>,
    pub diffusion_um2_per_time: f64,
    pub growth_rate_per_time: f64,
    pub carrying_capacity: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub front_threshold_fraction: f64,
    pub record_every_steps: u32,
    pub maximum_cell_steps: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrowthFrontStatePoint {
    pub position_um: f64,
    pub density: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrowthFrontSnapshot {
    pub time: f64,
    pub total_density_mass: f64,
    pub front_position_um: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrowthFrontSolverDiagnostics {
    pub method: &'static str,
    pub boundary_condition: &'static str,
    pub completed_steps: u32,
    pub cell_steps: u64,
    pub maximum_cfl_number: f64,
    pub cfl_limit: f64,
    pub positivity_violations: u64,
    pub minimum_density: f64,
    pub maximum_density: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrowthFrontResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub state_variable: &'static str,
    pub density_unit: &'static str,
    pub position_unit: &'static str,
    pub time_unit: &'static str,
    pub diffusion_um2_per_time: f64,
    pub growth_rate_per_time: f64,
    pub carrying_capacity: f64,
    pub final_time: f64,
    pub requested_time_step: f64,
    pub grid_spacing_um: f64,
    pub front_threshold_fraction: f64,
    pub initial_total_density_mass: f64,
    pub final_total_density_mass: f64,
    pub initial_front_position_um: Option<f64>,
    pub final_front_position_um: Option<f64>,
    pub observed_front_speed_um_per_time: Option<f64>,
    pub theoretical_planar_speed_um_per_time: Option<f64>,
    pub front_status: &'static str,
    pub initial_state: Vec<GrowthFrontStatePoint>,
    pub final_state: Vec<GrowthFrontStatePoint>,
    pub trajectory: Vec<GrowthFrontSnapshot>,
    pub solver: GrowthFrontSolverDiagnostics,
    pub differentiability_status: &'static str,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("invalid simulation: {0}")]
    Invalid(String),
    #[error("simulation numerical failure: {0}")]
    Numerical(String),
}

pub fn simulate_growth_front(spec: GrowthFrontSpec) -> Result<GrowthFrontResult, SimulationError> {
    let grid_spacing = validate_growth_front(&spec)?;
    let initial_state = spec
        .initial
        .iter()
        .map(|point| GrowthFrontStatePoint {
            position_um: point.position_um,
            density: point.density,
        })
        .collect::<Vec<_>>();
    let mut density = spec
        .initial
        .iter()
        .map(|point| point.density)
        .collect::<Vec<_>>();
    let positions = spec
        .initial
        .iter()
        .map(|point| point.position_um)
        .collect::<Vec<_>>();
    let threshold = spec.front_threshold_fraction * spec.carrying_capacity;
    let initial_mass = trapezoidal_mass(&density, grid_spacing);
    let initial_front = front_position(&positions, &density, threshold);
    let maximum_cfl = if spec.diffusion_um2_per_time == 0.0 {
        0.0
    } else {
        spec.diffusion_um2_per_time * spec.time_step / grid_spacing.powi(2)
    };
    let mut trajectory = vec![GrowthFrontSnapshot {
        time: 0.0,
        total_density_mass: initial_mass,
        front_position_um: initial_front,
    }];
    let mut time = 0.0;
    let mut completed_steps = 0_u32;
    let mut positivity_violations = 0_u64;
    while time < spec.final_time {
        let dt = spec.time_step.min(spec.final_time - time);
        reaction_step(
            &mut density,
            spec.growth_rate_per_time,
            spec.carrying_capacity,
            dt / 2.0,
        )?;
        diffusion_step(&mut density, spec.diffusion_um2_per_time, grid_spacing, dt)?;
        reaction_step(
            &mut density,
            spec.growth_rate_per_time,
            spec.carrying_capacity,
            dt / 2.0,
        )?;
        for value in &mut density {
            if *value < -1e-12 || *value > spec.carrying_capacity * (1.0 + 1e-12) {
                positivity_violations += 1;
            }
            if !value.is_finite() {
                return Err(SimulationError::Numerical(
                    "density became non-finite".into(),
                ));
            }
            *value = value.clamp(0.0, spec.carrying_capacity);
        }
        if positivity_violations != 0 {
            return Err(SimulationError::Numerical(
                "density left the invariant interval".into(),
            ));
        }
        time += dt;
        completed_steps = completed_steps
            .checked_add(1)
            .ok_or_else(|| SimulationError::Numerical("completed step count overflowed".into()))?;
        if completed_steps.is_multiple_of(spec.record_every_steps) || time == spec.final_time {
            trajectory.push(GrowthFrontSnapshot {
                time,
                total_density_mass: trapezoidal_mass(&density, grid_spacing),
                front_position_um: front_position(&positions, &density, threshold),
            });
        }
    }
    let final_front = front_position(&positions, &density, threshold);
    let observed_speed = initial_front
        .zip(final_front)
        .map(|(initial, final_position)| (final_position - initial) / spec.final_time);
    let theoretical_speed = (spec.diffusion_um2_per_time > 0.0 && spec.growth_rate_per_time > 0.0)
        .then(|| 2.0 * (spec.diffusion_um2_per_time * spec.growth_rate_per_time).sqrt());
    let front_status = if initial_front.is_some() && final_front.is_some() {
        "available_single_threshold_front"
    } else {
        "unavailable_no_threshold_crossing"
    };
    let final_mass = trapezoidal_mass(&density, grid_spacing);
    let minimum_density = density.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum_density = density.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let cell_steps = u64::from(completed_steps) * density.len() as u64;
    Ok(GrowthFrontResult {
        format: "marklab.fisher_kpp_growth_front",
        version: 1,
        simulator_id: "fisher_kpp_1d_no_flux",
        simulator_version: 1,
        dimensionality: 1,
        state_variable: "density",
        density_unit: "relative_density",
        position_unit: "micrometre",
        time_unit: "declared_time_unit",
        diffusion_um2_per_time: spec.diffusion_um2_per_time,
        growth_rate_per_time: spec.growth_rate_per_time,
        carrying_capacity: spec.carrying_capacity,
        final_time: spec.final_time,
        requested_time_step: spec.time_step,
        grid_spacing_um: grid_spacing,
        front_threshold_fraction: spec.front_threshold_fraction,
        initial_total_density_mass: initial_mass,
        final_total_density_mass: final_mass,
        initial_front_position_um: initial_front,
        final_front_position_um: final_front,
        observed_front_speed_um_per_time: observed_speed,
        theoretical_planar_speed_um_per_time: theoretical_speed,
        front_status,
        initial_state,
        final_state: positions
            .into_iter()
            .zip(density)
            .map(|(position_um, density)| GrowthFrontStatePoint {
                position_um,
                density,
            })
            .collect(),
        trajectory,
        solver: GrowthFrontSolverDiagnostics {
            method: "strang_exact_logistic_plus_explicit_centered_diffusion",
            boundary_condition: "no_flux_reflected_ghost_cells",
            completed_steps,
            cell_steps,
            maximum_cfl_number: maximum_cfl,
            cfl_limit: 0.5,
            positivity_violations,
            minimum_density,
            maximum_density,
        },
        differentiability_status: "piecewise_differentiable_except_front_extraction",
        random_seed_namespace: "deterministic_no_stochastic_source",
        claim_status: "experimental_mechanistic_simulation_not_tumor_forecast",
    })
}

fn validate_growth_front(spec: &GrowthFrontSpec) -> Result<f64, SimulationError> {
    let controls_valid = (3..=100_000).contains(&spec.initial.len())
        && spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.growth_rate_per_time.is_finite()
        && spec.growth_rate_per_time >= 0.0
        && spec.carrying_capacity.is_finite()
        && spec.carrying_capacity > 0.0
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && spec.time_step.is_finite()
        && spec.time_step > 0.0
        && spec.front_threshold_fraction.is_finite()
        && spec.front_threshold_fraction > 0.0
        && spec.front_threshold_fraction < 1.0
        && spec.record_every_steps > 0
        && (1..=250_000_000).contains(&spec.maximum_cell_steps);
    if !controls_valid {
        return Err(SimulationError::Invalid(
            "dimensions, parameters, or controls are invalid".into(),
        ));
    }
    if spec.initial.iter().any(|point| {
        !point.position_um.is_finite()
            || !point.density.is_finite()
            || point.density < 0.0
            || point.density > spec.carrying_capacity
    }) {
        return Err(SimulationError::Invalid(
            "positions or initial densities are invalid".into(),
        ));
    }
    let grid_spacing = spec.initial[1].position_um - spec.initial[0].position_um;
    if !grid_spacing.is_finite() || grid_spacing <= 0.0 {
        return Err(SimulationError::Invalid(
            "grid positions must increase with positive spacing".into(),
        ));
    }
    let spacing_tolerance = 1e-10 * (1.0 + grid_spacing.abs());
    if spec.initial.windows(2).any(|pair| {
        let spacing = pair[1].position_um - pair[0].position_um;
        spacing <= 0.0 || (spacing - grid_spacing).abs() > spacing_tolerance
    }) {
        return Err(SimulationError::Invalid(
            "growth-front grid must be strictly increasing and equally spaced".into(),
        ));
    }
    let maximum_steps = (spec.final_time / spec.time_step).ceil();
    if !maximum_steps.is_finite() || maximum_steps > f64::from(u32::MAX) {
        return Err(SimulationError::Invalid(
            "growth-front step count is invalid".into(),
        ));
    }
    let cell_steps = maximum_steps as u64 * spec.initial.len() as u64;
    if cell_steps > spec.maximum_cell_steps {
        return Err(SimulationError::Invalid(format!(
            "{cell_steps} cell-steps exceed the declared resource bound"
        )));
    }
    let cfl = spec.diffusion_um2_per_time * spec.time_step / grid_spacing.powi(2);
    if !cfl.is_finite() || cfl > 0.5 + 1e-14 {
        return Err(SimulationError::Invalid(format!(
            "diffusion CFL {cfl} exceeds 0.5"
        )));
    }
    Ok(grid_spacing)
}

fn reaction_step(
    density: &mut [f64],
    growth_rate: f64,
    carrying_capacity: f64,
    dt: f64,
) -> Result<(), SimulationError> {
    if growth_rate == 0.0 || dt == 0.0 {
        return Ok(());
    }
    let decay = (-growth_rate * dt).exp();
    for value in density {
        if *value == 0.0 {
            continue;
        }
        let denominator = *value + (carrying_capacity - *value) * decay;
        *value = carrying_capacity * *value / denominator;
        if !value.is_finite() {
            return Err(SimulationError::Numerical(
                "logistic reaction became non-finite".into(),
            ));
        }
    }
    Ok(())
}

fn diffusion_step(
    density: &mut [f64],
    diffusion: f64,
    spacing: f64,
    dt: f64,
) -> Result<(), SimulationError> {
    if diffusion == 0.0 || dt == 0.0 {
        return Ok(());
    }
    let cfl = diffusion * dt / spacing.powi(2);
    let previous = density.to_vec();
    density[0] = previous[0] + 2.0 * cfl * (previous[1] - previous[0]);
    for index in 1..density.len() - 1 {
        density[index] = previous[index]
            + cfl * (previous[index - 1] - 2.0 * previous[index] + previous[index + 1]);
    }
    let last = density.len() - 1;
    density[last] = previous[last] + 2.0 * cfl * (previous[last - 1] - previous[last]);
    if density.iter().any(|value| !value.is_finite()) {
        return Err(SimulationError::Numerical(
            "diffusion step became non-finite".into(),
        ));
    }
    Ok(())
}

fn trapezoidal_mass(density: &[f64], spacing: f64) -> f64 {
    let endpoints = (density[0] + density[density.len() - 1]) / 2.0;
    spacing * (endpoints + density[1..density.len() - 1].iter().sum::<f64>())
}

fn front_position(positions: &[f64], density: &[f64], threshold: f64) -> Option<f64> {
    positions
        .windows(2)
        .zip(density.windows(2))
        .filter_map(|(position, value)| {
            if value[0] >= threshold && value[1] < threshold {
                let fraction = (value[0] - threshold) / (value[0] - value[1]);
                Some(position[0] + fraction * (position[1] - position[0]))
            } else {
                None
            }
        })
        .next_back()
}
