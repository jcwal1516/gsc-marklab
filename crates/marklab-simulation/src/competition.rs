use serde::Serialize;

use super::{diffusion_step, reaction_step, trapezoidal_mass, SimulationError};

#[derive(Clone, Debug)]
pub struct CompetitionInitialPoint {
    pub position_um: f64,
    pub density_a: f64,
    pub density_b: f64,
}

#[derive(Clone, Debug)]
pub struct CompetitionSpec {
    pub initial: Vec<CompetitionInitialPoint>,
    pub diffusion_a_um2_per_time: f64,
    pub diffusion_b_um2_per_time: f64,
    pub growth_a_per_time: f64,
    pub growth_b_per_time: f64,
    pub carrying_a: f64,
    pub carrying_b: f64,
    pub competition_a_from_b: f64,
    pub competition_b_from_a: f64,
    pub treatment_a_per_time: f64,
    pub treatment_b_per_time: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub extinction_threshold_fraction: f64,
    pub record_every_steps: u32,
    pub maximum_cell_species_steps: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompetitionStatePoint {
    pub position_um: f64,
    pub density_a: f64,
    pub density_b: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompetitionSnapshot {
    pub time: f64,
    pub total_mass_a: f64,
    pub total_mass_b: f64,
    pub maximum_density_a: f64,
    pub maximum_density_b: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompetitionExtinctionEvent {
    pub species: &'static str,
    pub time: f64,
    pub threshold: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompetitionSolverDiagnostics {
    pub method: &'static str,
    pub boundary_condition: &'static str,
    pub completed_steps: u32,
    pub cell_species_steps: u64,
    pub maximum_cfl_a: f64,
    pub maximum_cfl_b: f64,
    pub cfl_limit: f64,
    pub positivity_violations: u64,
    pub minimum_density: f64,
    pub maximum_density: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpatialCompetitionResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub species: [&'static str; 2],
    pub position_unit: &'static str,
    pub density_unit: &'static str,
    pub time_unit: &'static str,
    pub diffusion_a_um2_per_time: f64,
    pub diffusion_b_um2_per_time: f64,
    pub growth_a_per_time: f64,
    pub growth_b_per_time: f64,
    pub carrying_a: f64,
    pub carrying_b: f64,
    pub competition_a_from_b: f64,
    pub competition_b_from_a: f64,
    pub treatment_a_per_time: f64,
    pub treatment_b_per_time: f64,
    pub final_time: f64,
    pub requested_time_step: f64,
    pub grid_spacing_um: f64,
    pub extinction_threshold_fraction: f64,
    pub initial_total_mass_a: f64,
    pub initial_total_mass_b: f64,
    pub final_total_mass_a: f64,
    pub final_total_mass_b: f64,
    pub final_maximum_density_a: f64,
    pub final_maximum_density_b: f64,
    pub outcome: &'static str,
    pub extinction_events: Vec<CompetitionExtinctionEvent>,
    pub initial_state: Vec<CompetitionStatePoint>,
    pub final_state: Vec<CompetitionStatePoint>,
    pub trajectory: Vec<CompetitionSnapshot>,
    pub solver: CompetitionSolverDiagnostics,
    pub treatment_interpretation: &'static str,
    pub claim_status: &'static str,
}

pub fn simulate_spatial_competition(
    spec: CompetitionSpec,
) -> Result<SpatialCompetitionResult, SimulationError> {
    let spacing = validate(&spec)?;
    let positions = spec
        .initial
        .iter()
        .map(|point| point.position_um)
        .collect::<Vec<_>>();
    let mut density_a = spec
        .initial
        .iter()
        .map(|point| point.density_a)
        .collect::<Vec<_>>();
    let mut density_b = spec
        .initial
        .iter()
        .map(|point| point.density_b)
        .collect::<Vec<_>>();
    let initial_state = state(&positions, &density_a, &density_b);
    let initial_mass_a = trapezoidal_mass(&density_a, spacing);
    let initial_mass_b = trapezoidal_mass(&density_b, spacing);
    let threshold_a = spec.extinction_threshold_fraction * spec.carrying_a;
    let threshold_b = spec.extinction_threshold_fraction * spec.carrying_b;
    let mut a_was_above = maximum(&density_a) > threshold_a;
    let mut b_was_above = maximum(&density_b) > threshold_b;
    let mut a_event = false;
    let mut b_event = false;
    let mut extinction_events = Vec::new();
    let mut trajectory = vec![snapshot(0.0, &density_a, &density_b, spacing)];
    let mut time = 0.0;
    let mut completed_steps = 0_u32;
    let mut positivity_violations = 0_u64;
    while time < spec.final_time {
        let dt = spec.time_step.min(spec.final_time - time);
        competition_reaction_step(&mut density_a, &mut density_b, &spec, dt / 2.0)?;
        diffusion_step(&mut density_a, spec.diffusion_a_um2_per_time, spacing, dt)?;
        diffusion_step(&mut density_b, spec.diffusion_b_um2_per_time, spacing, dt)?;
        competition_reaction_step(&mut density_a, &mut density_b, &spec, dt / 2.0)?;
        check_state(&mut density_a, spec.carrying_a, &mut positivity_violations)?;
        check_state(&mut density_b, spec.carrying_b, &mut positivity_violations)?;
        time += dt;
        completed_steps = completed_steps
            .checked_add(1)
            .ok_or_else(|| SimulationError::Numerical("step count overflowed".into()))?;
        let max_a = maximum(&density_a);
        let max_b = maximum(&density_b);
        if a_was_above && !a_event && max_a <= threshold_a {
            extinction_events.push(CompetitionExtinctionEvent {
                species: "a",
                time,
                threshold: threshold_a,
            });
            a_event = true;
        }
        if b_was_above && !b_event && max_b <= threshold_b {
            extinction_events.push(CompetitionExtinctionEvent {
                species: "b",
                time,
                threshold: threshold_b,
            });
            b_event = true;
        }
        a_was_above |= max_a > threshold_a;
        b_was_above |= max_b > threshold_b;
        if completed_steps.is_multiple_of(spec.record_every_steps) || time == spec.final_time {
            trajectory.push(snapshot(time, &density_a, &density_b, spacing));
        }
    }
    let final_max_a = maximum(&density_a);
    let final_max_b = maximum(&density_b);
    let a_extinct = final_max_a <= threshold_a;
    let b_extinct = final_max_b <= threshold_b;
    let outcome = match (a_extinct, b_extinct) {
        (false, false) => "coexistence",
        (true, false) => "species_a_excluded",
        (false, true) => "species_b_excluded",
        (true, true) => "both_species_below_threshold",
    };
    let minimum_density = density_a
        .iter()
        .chain(&density_b)
        .copied()
        .fold(f64::INFINITY, f64::min);
    let maximum_density = density_a
        .iter()
        .chain(&density_b)
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let cell_species_steps = u64::from(completed_steps) * density_a.len() as u64 * 2;
    Ok(SpatialCompetitionResult {
        format: "marklab.spatial_competition",
        version: 1,
        simulator_id: "two_species_lotka_volterra_1d_no_flux",
        simulator_version: 1,
        dimensionality: 1,
        species: ["a", "b"],
        position_unit: "micrometre",
        density_unit: "relative_density",
        time_unit: "declared_time_unit",
        diffusion_a_um2_per_time: spec.diffusion_a_um2_per_time,
        diffusion_b_um2_per_time: spec.diffusion_b_um2_per_time,
        growth_a_per_time: spec.growth_a_per_time,
        growth_b_per_time: spec.growth_b_per_time,
        carrying_a: spec.carrying_a,
        carrying_b: spec.carrying_b,
        competition_a_from_b: spec.competition_a_from_b,
        competition_b_from_a: spec.competition_b_from_a,
        treatment_a_per_time: spec.treatment_a_per_time,
        treatment_b_per_time: spec.treatment_b_per_time,
        final_time: spec.final_time,
        requested_time_step: spec.time_step,
        grid_spacing_um: spacing,
        extinction_threshold_fraction: spec.extinction_threshold_fraction,
        initial_total_mass_a: initial_mass_a,
        initial_total_mass_b: initial_mass_b,
        final_total_mass_a: trapezoidal_mass(&density_a, spacing),
        final_total_mass_b: trapezoidal_mass(&density_b, spacing),
        final_maximum_density_a: final_max_a,
        final_maximum_density_b: final_max_b,
        outcome,
        extinction_events,
        initial_state,
        final_state: state(&positions, &density_a, &density_b),
        trajectory,
        solver: CompetitionSolverDiagnostics {
            method: "strang_logistic_competition_treatment_plus_explicit_diffusion",
            boundary_condition: "no_flux_reflected_ghost_cells",
            completed_steps,
            cell_species_steps,
            maximum_cfl_a: spec.diffusion_a_um2_per_time * spec.time_step / spacing.powi(2),
            maximum_cfl_b: spec.diffusion_b_um2_per_time * spec.time_step / spacing.powi(2),
            cfl_limit: 0.5,
            positivity_violations,
            minimum_density,
            maximum_density,
        },
        treatment_interpretation: "declared_external_mortality_rate_not_estimated_causal_effect",
        claim_status: "experimental_ecological_simulation_not_causal_treatment_effect",
    })
}

fn competition_reaction_step(
    density_a: &mut [f64],
    density_b: &mut [f64],
    spec: &CompetitionSpec,
    dt: f64,
) -> Result<(), SimulationError> {
    reaction_step(density_a, spec.growth_a_per_time, spec.carrying_a, dt / 2.0)?;
    reaction_step(density_b, spec.growth_b_per_time, spec.carrying_b, dt / 2.0)?;
    for (a, b) in density_a.iter_mut().zip(density_b.iter_mut()) {
        let previous_a = *a;
        let previous_b = *b;
        let loss_a = spec.treatment_a_per_time
            + spec.growth_a_per_time * spec.competition_a_from_b * previous_b / spec.carrying_a;
        let loss_b = spec.treatment_b_per_time
            + spec.growth_b_per_time * spec.competition_b_from_a * previous_a / spec.carrying_b;
        *a *= (-loss_a * dt).exp();
        *b *= (-loss_b * dt).exp();
    }
    reaction_step(density_a, spec.growth_a_per_time, spec.carrying_a, dt / 2.0)?;
    reaction_step(density_b, spec.growth_b_per_time, spec.carrying_b, dt / 2.0)
}

fn validate(spec: &CompetitionSpec) -> Result<f64, SimulationError> {
    let parameters = [
        spec.diffusion_a_um2_per_time,
        spec.diffusion_b_um2_per_time,
        spec.growth_a_per_time,
        spec.growth_b_per_time,
        spec.competition_a_from_b,
        spec.competition_b_from_a,
        spec.treatment_a_per_time,
        spec.treatment_b_per_time,
    ];
    let controls_valid = (3..=100_000).contains(&spec.initial.len())
        && parameters
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        && spec.carrying_a.is_finite()
        && spec.carrying_a > 0.0
        && spec.carrying_b.is_finite()
        && spec.carrying_b > 0.0
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && spec.time_step.is_finite()
        && spec.time_step > 0.0
        && spec.extinction_threshold_fraction.is_finite()
        && spec.extinction_threshold_fraction > 0.0
        && spec.extinction_threshold_fraction < 1.0
        && spec.record_every_steps > 0
        && (1..=250_000_000).contains(&spec.maximum_cell_species_steps);
    if !controls_valid {
        return Err(SimulationError::Invalid(
            "competition dimensions, parameters, or controls are invalid".into(),
        ));
    }
    if spec.initial.iter().any(|point| {
        !point.position_um.is_finite()
            || !point.density_a.is_finite()
            || !point.density_b.is_finite()
            || !(0.0..=spec.carrying_a).contains(&point.density_a)
            || !(0.0..=spec.carrying_b).contains(&point.density_b)
    }) {
        return Err(SimulationError::Invalid(
            "competition grid or initial densities are invalid".into(),
        ));
    }
    let spacing = spec.initial[1].position_um - spec.initial[0].position_um;
    let tolerance = 1e-10 * (1.0 + spacing.abs());
    if !spacing.is_finite()
        || spacing <= 0.0
        || spec.initial.windows(2).any(|pair| {
            let value = pair[1].position_um - pair[0].position_um;
            value <= 0.0 || (value - spacing).abs() > tolerance
        })
    {
        return Err(SimulationError::Invalid(
            "competition grid must be strictly increasing and equally spaced".into(),
        ));
    }
    let maximum_steps = (spec.final_time / spec.time_step).ceil();
    if !maximum_steps.is_finite() || maximum_steps > f64::from(u32::MAX) {
        return Err(SimulationError::Invalid(
            "competition step count is invalid".into(),
        ));
    }
    let work = maximum_steps as u64 * spec.initial.len() as u64 * 2;
    if work > spec.maximum_cell_species_steps {
        return Err(SimulationError::Invalid(format!(
            "{work} cell-species-steps exceed the declared resource bound"
        )));
    }
    for (label, diffusion) in [
        ("a", spec.diffusion_a_um2_per_time),
        ("b", spec.diffusion_b_um2_per_time),
    ] {
        let cfl = diffusion * spec.time_step / spacing.powi(2);
        if !cfl.is_finite() || cfl > 0.5 + 1e-14 {
            return Err(SimulationError::Invalid(format!(
                "species {label} diffusion CFL {cfl} exceeds 0.5"
            )));
        }
    }
    Ok(spacing)
}

fn check_state(
    density: &mut [f64],
    carrying: f64,
    violations: &mut u64,
) -> Result<(), SimulationError> {
    for value in density {
        if !value.is_finite() {
            return Err(SimulationError::Numerical(
                "competition density became non-finite".into(),
            ));
        }
        if *value < -1e-12 || *value > carrying * (1.0 + 1e-12) {
            *violations += 1;
        }
        *value = value.clamp(0.0, carrying);
    }
    if *violations != 0 {
        return Err(SimulationError::Numerical(
            "competition density left its invariant interval".into(),
        ));
    }
    Ok(())
}

fn state(positions: &[f64], density_a: &[f64], density_b: &[f64]) -> Vec<CompetitionStatePoint> {
    positions
        .iter()
        .zip(density_a)
        .zip(density_b)
        .map(|((position, a), b)| CompetitionStatePoint {
            position_um: *position,
            density_a: *a,
            density_b: *b,
        })
        .collect()
}

fn snapshot(time: f64, density_a: &[f64], density_b: &[f64], spacing: f64) -> CompetitionSnapshot {
    CompetitionSnapshot {
        time,
        total_mass_a: trapezoidal_mass(density_a, spacing),
        total_mass_b: trapezoidal_mass(density_b, spacing),
        maximum_density_a: maximum(density_a),
        maximum_density_b: maximum(density_b),
    }
}

fn maximum(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}
