use marklab_core::backend::ClaimStatus;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use serde::Serialize;

use super::SimulationError;

const MAXIMUM_CELLS: usize = 65_536;
const MAXIMUM_CELL_STEPS: u64 = 50_000_000;
const MAXIMUM_RETAINED_SNAPSHOTS: u64 = 10_001;
const MAXIMUM_INITIAL_ABSOLUTE_PHASE: f64 = 2.0;
const MAXIMUM_SOLVER_ABSOLUTE_PHASE: f64 = 4.0;

/// A periodic two-dimensional Cahn-Hilliard phase-field simulation.
#[derive(Clone, Debug)]
pub struct PhaseFieldSpec {
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub initial_phase_row_major: Vec<f64>,
    pub mobility_um2_per_time: f64,
    pub bulk_energy_scale: f64,
    pub gradient_energy_coefficient_um2: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub record_every_steps: u32,
    pub maximum_cell_steps: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseFieldStateCell {
    pub ix: u32,
    pub iy: u32,
    pub x_um: f64,
    pub y_um: f64,
    pub phase: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseFieldSnapshot {
    pub time: f64,
    pub mean_phase: f64,
    pub population_variance: f64,
    pub minimum_phase: f64,
    pub maximum_phase: f64,
    pub mass: f64,
    pub bulk_free_energy: f64,
    pub gradient_free_energy: f64,
    pub total_free_energy: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseFieldConservationSummary {
    pub initial_mean: f64,
    pub final_mean: f64,
    pub absolute_mean_drift: f64,
    pub mean_tolerance: f64,
    pub mean_conserved: bool,
    pub initial_mass: f64,
    pub final_mass: f64,
    pub absolute_mass_drift: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseFieldEnergySummary {
    pub discrete_functional: &'static str,
    pub initial_free_energy: f64,
    pub final_free_energy: f64,
    pub absolute_change: f64,
    pub largest_step_increase: f64,
    pub maximum_step_tolerance: f64,
    pub nonincreasing: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseFieldSolverDiagnostics {
    pub method: &'static str,
    pub boundary_condition: &'static str,
    pub laplacian: &'static str,
    pub completed_steps: u32,
    pub cell_steps: u64,
    pub retained_snapshots: u32,
    pub maximum_stabilization: f64,
    pub maximum_stabilized_update_number: f64,
    pub stabilized_update_limit: f64,
    pub maximum_absolute_mean_drift: f64,
    pub energy_rejections: u32,
    pub minimum_phase: f64,
    pub maximum_phase: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseFieldResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub mobility_um2_per_time: f64,
    pub bulk_energy_scale: f64,
    pub gradient_energy_coefficient_um2: f64,
    pub final_time: f64,
    pub requested_time_step: f64,
    pub initial_state: Vec<PhaseFieldStateCell>,
    pub final_state: Vec<PhaseFieldStateCell>,
    pub trajectory: Vec<PhaseFieldSnapshot>,
    pub conservation: PhaseFieldConservationSummary,
    pub energy: PhaseFieldEnergySummary,
    pub solver: PhaseFieldSolverDiagnostics,
    pub observation_model: &'static str,
    pub claim_status: ClaimStatus,
}

/// Evolve `phi_t = M Laplacian(mu)` with
/// `mu = A (phi^3 - phi) - kappa Laplacian(phi)` on a periodic grid.
pub fn simulate_phase_field(spec: PhaseFieldSpec) -> Result<PhaseFieldResult, SimulationError> {
    let (cells, planned_steps) = validate(&spec)?;
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let initial_mean = mean(&spec.initial_phase_row_major);
    let initial_state = state(&spec.initial_phase_row_major, &spec);
    let initial_snapshot = snapshot(0.0, &spec.initial_phase_row_major, &spec);
    validate_snapshot(&initial_snapshot, "initial")?;
    let initial_energy = initial_snapshot.total_free_energy;
    let initial_mass = initial_snapshot.mass;
    let mean_tolerance = 1e-11 * (1.0 + initial_mean.abs());

    let mut planner = FftPlanner::new();
    let row_forward = planner.plan_fft_forward(nx);
    let column_forward = planner.plan_fft_forward(ny);
    let row_inverse = planner.plan_fft_inverse(nx);
    let column_inverse = planner.plan_fft_inverse(ny);

    let mut phase = spec.initial_phase_row_major.clone();
    let mut phase_spectrum = phase
        .iter()
        .copied()
        .map(|value| Complex::new(value, 0.0))
        .collect::<Vec<_>>();
    transform_2d(
        &mut phase_spectrum,
        nx,
        ny,
        row_forward.as_ref(),
        column_forward.as_ref(),
        false,
    );
    let laplacian_eigenvalues = laplacian_eigenvalues(&spec)?;
    let mut trajectory = vec![initial_snapshot];
    let mut completed_steps = 0_u32;
    let mut time = 0.0;
    let mut previous_energy = initial_energy;
    let mut largest_step_increase = 0.0_f64;
    let mut maximum_step_tolerance = 0.0_f64;
    let mut maximum_stabilization = 0.0_f64;
    let mut maximum_update_number = 0.0_f64;
    let mut maximum_mean_drift = 0.0_f64;

    while completed_steps < planned_steps {
        let next_time = if completed_steps + 1 == planned_steps {
            spec.final_time
        } else {
            f64::from(completed_steps + 1) * spec.time_step
        };
        let dt = next_time - time;
        let maximum_absolute_phase = phase.iter().copied().map(f64::abs).fold(0.0, f64::max);
        if maximum_absolute_phase > MAXIMUM_SOLVER_ABSOLUTE_PHASE {
            return Err(SimulationError::Numerical(format!(
                "phase-field absolute phase {maximum_absolute_phase} exceeds the solver bound {MAXIMUM_SOLVER_ABSOLUTE_PHASE}"
            )));
        }
        let nonlinear_lipschitz =
            spec.bulk_energy_scale * (3.0 * maximum_absolute_phase.powi(2) + 1.0);
        let stabilization = nonlinear_lipschitz;
        maximum_stabilization = maximum_stabilization.max(stabilization);

        let mut nonlinear_spectrum = phase
            .iter()
            .map(|value| Complex::new(value.powi(3) - value, 0.0))
            .collect::<Vec<_>>();
        transform_2d(
            &mut nonlinear_spectrum,
            nx,
            ny,
            row_forward.as_ref(),
            column_forward.as_ref(),
            false,
        );
        for ((coefficient, nonlinear), q) in phase_spectrum
            .iter_mut()
            .zip(&nonlinear_spectrum)
            .zip(&laplacian_eigenvalues)
        {
            let mobility_wave = dt * spec.mobility_um2_per_time * q;
            let denominator =
                1.0 + mobility_wave * (stabilization + spec.gradient_energy_coefficient_um2 * q);
            let update_number = mobility_wave * nonlinear_lipschitz / denominator;
            maximum_update_number = maximum_update_number.max(update_number);
            *coefficient = (*coefficient * (1.0 + mobility_wave * stabilization)
                - *nonlinear * (mobility_wave * spec.bulk_energy_scale))
                / denominator;
        }
        if !maximum_update_number.is_finite() || maximum_update_number > 1.0 + 1e-12 {
            return Err(SimulationError::Numerical(format!(
                "phase-field stabilized update number {maximum_update_number} exceeds 1"
            )));
        }

        let mut next_phase = phase_spectrum.clone();
        transform_2d(
            &mut next_phase,
            nx,
            ny,
            row_inverse.as_ref(),
            column_inverse.as_ref(),
            true,
        );
        phase = next_phase.into_iter().map(|value| value.re).collect();
        if phase.iter().any(|value| !value.is_finite()) {
            return Err(SimulationError::Numerical(
                "phase-field state became non-finite".into(),
            ));
        }
        let next_maximum_absolute_phase = phase.iter().copied().map(f64::abs).fold(0.0, f64::max);
        if next_maximum_absolute_phase > MAXIMUM_SOLVER_ABSOLUTE_PHASE {
            return Err(SimulationError::Numerical(format!(
                "phase-field absolute phase {next_maximum_absolute_phase} exceeds the solver bound {MAXIMUM_SOLVER_ABSOLUTE_PHASE}"
            )));
        }
        let mean_drift = (mean(&phase) - initial_mean).abs();
        maximum_mean_drift = maximum_mean_drift.max(mean_drift);
        if mean_drift > mean_tolerance {
            return Err(SimulationError::Numerical(format!(
                "phase-field mean drift {mean_drift} exceeds tolerance {mean_tolerance}"
            )));
        }
        let current = snapshot(next_time, &phase, &spec);
        validate_snapshot(&current, "evolved")?;
        let energy_increase = (current.total_free_energy - previous_energy).max(0.0);
        let energy_tolerance = 1e-10 * (1.0 + previous_energy.abs());
        largest_step_increase = largest_step_increase.max(energy_increase);
        maximum_step_tolerance = maximum_step_tolerance.max(energy_tolerance);
        if energy_increase > energy_tolerance {
            return Err(SimulationError::Numerical(format!(
                "phase-field free energy increased by {energy_increase}, exceeding tolerance {energy_tolerance}; reduce --time-step"
            )));
        }
        previous_energy = current.total_free_energy;
        time = next_time;
        completed_steps += 1;
        if completed_steps.is_multiple_of(spec.record_every_steps)
            || completed_steps == planned_steps
        {
            trajectory.push(current);
        }
    }

    let final_snapshot = trajectory
        .last()
        .expect("the final step retains a snapshot");
    let final_energy = final_snapshot.total_free_energy;
    let final_mean = final_snapshot.mean_phase;
    let final_mass = final_snapshot.mass;
    let minimum_phase = final_snapshot.minimum_phase;
    let maximum_phase = final_snapshot.maximum_phase;
    let absolute_mean_drift = (final_mean - initial_mean).abs();
    let absolute_mass_drift = (final_mass - initial_mass).abs();
    let energy_change = final_energy - initial_energy;
    if [
        minimum_phase,
        maximum_phase,
        absolute_mean_drift,
        absolute_mass_drift,
        energy_change,
        largest_step_increase,
        maximum_step_tolerance,
        maximum_stabilization,
        maximum_update_number,
        maximum_mean_drift,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(SimulationError::Numerical(
            "phase-field result summary became non-finite".into(),
        ));
    }
    Ok(PhaseFieldResult {
        format: "marklab.cahn_hilliard_phase_field",
        version: 1,
        simulator_id: "periodic_2d_cahn_hilliard_double_well",
        simulator_version: 1,
        dimensionality: 2,
        grid_x: spec.grid_x,
        grid_y: spec.grid_y,
        spacing_x_um: spec.spacing_x_um,
        spacing_y_um: spec.spacing_y_um,
        mobility_um2_per_time: spec.mobility_um2_per_time,
        bulk_energy_scale: spec.bulk_energy_scale,
        gradient_energy_coefficient_um2: spec.gradient_energy_coefficient_um2,
        final_time: spec.final_time,
        requested_time_step: spec.time_step,
        initial_state,
        final_state: state(&phase, &spec),
        trajectory,
        conservation: PhaseFieldConservationSummary {
            initial_mean,
            final_mean,
            absolute_mean_drift,
            mean_tolerance,
            mean_conserved: absolute_mean_drift <= mean_tolerance,
            initial_mass,
            final_mass,
            absolute_mass_drift,
        },
        energy: PhaseFieldEnergySummary {
            discrete_functional: "periodic_forward_difference_double_well_plus_squared_gradient",
            initial_free_energy: initial_energy,
            final_free_energy: final_energy,
            absolute_change: energy_change,
            largest_step_increase,
            maximum_step_tolerance,
            nonincreasing: largest_step_increase <= maximum_step_tolerance,
        },
        solver: PhaseFieldSolverDiagnostics {
            method: "first_order_stabilized_semi_implicit_discrete_fourier",
            boundary_condition: "periodic",
            laplacian: "periodic_five_point_finite_difference_spectrum",
            completed_steps,
            cell_steps: u64::from(completed_steps) * cells as u64,
            retained_snapshots: u32::try_from(trajectory_len(
                planned_steps,
                spec.record_every_steps,
            ))
            .expect("validated snapshot count fits u32"),
            maximum_stabilization,
            maximum_stabilized_update_number: maximum_update_number,
            stabilized_update_limit: 1.0,
            maximum_absolute_mean_drift: maximum_mean_drift,
            energy_rejections: 0,
            minimum_phase,
            maximum_phase,
        },
        observation_model: "identity_phase_field_observation",
        claim_status: ClaimStatus::ExperimentalSimulation,
    })
}

fn validate(spec: &PhaseFieldSpec) -> Result<(usize, u32), SimulationError> {
    let cells = (spec.grid_x as usize)
        .checked_mul(spec.grid_y as usize)
        .ok_or_else(|| SimulationError::Invalid("phase-field grid size overflowed".into()))?;
    let valid = (2..=256).contains(&spec.grid_x)
        && (2..=256).contains(&spec.grid_y)
        && cells <= MAXIMUM_CELLS
        && spec.initial_phase_row_major.len() == cells
        && spec.spacing_x_um.is_finite()
        && spec.spacing_x_um > 0.0
        && spec.spacing_y_um.is_finite()
        && spec.spacing_y_um > 0.0
        && spec.mobility_um2_per_time.is_finite()
        && spec.mobility_um2_per_time > 0.0
        && spec.bulk_energy_scale.is_finite()
        && spec.bulk_energy_scale > 0.0
        && spec.gradient_energy_coefficient_um2.is_finite()
        && spec.gradient_energy_coefficient_um2 > 0.0
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && spec.time_step.is_finite()
        && spec.time_step > 0.0
        && spec.time_step <= spec.final_time
        && spec.record_every_steps > 0
        && (1..=MAXIMUM_CELL_STEPS).contains(&spec.maximum_cell_steps)
        && spec
            .initial_phase_row_major
            .iter()
            .all(|value| value.is_finite() && value.abs() <= MAXIMUM_INITIAL_ABSOLUTE_PHASE);
    if !valid {
        return Err(SimulationError::Invalid(
            "phase-field grid, initial phase, energy parameters, or controls are invalid".into(),
        ));
    }
    validate_derived_geometry_and_operator(spec, cells)?;
    let steps = (spec.final_time / spec.time_step).ceil();
    if !steps.is_finite() || steps > f64::from(u32::MAX) {
        return Err(SimulationError::Invalid(
            "phase-field step count is invalid".into(),
        ));
    }
    let planned_steps = steps as u32;
    let work = u64::from(planned_steps)
        .checked_mul(cells as u64)
        .ok_or_else(|| SimulationError::Invalid("phase-field work overflowed".into()))?;
    if work > spec.maximum_cell_steps {
        return Err(SimulationError::Invalid(format!(
            "{work} cell-steps exceed the declared resource bound"
        )));
    }
    let snapshots = trajectory_len(planned_steps, spec.record_every_steps);
    if snapshots > MAXIMUM_RETAINED_SNAPSHOTS {
        return Err(SimulationError::Invalid(format!(
            "{snapshots} retained phase-field snapshots exceed the {MAXIMUM_RETAINED_SNAPSHOTS} limit"
        )));
    }
    Ok((cells, planned_steps))
}

fn validate_derived_geometry_and_operator(
    spec: &PhaseFieldSpec,
    cells: usize,
) -> Result<(), SimulationError> {
    let maximum_x_um = f64::from(spec.grid_x - 1) * spec.spacing_x_um;
    let maximum_y_um = f64::from(spec.grid_y - 1) * spec.spacing_y_um;
    let area_per_cell_um2 = spec.spacing_x_um * spec.spacing_y_um;
    let domain_area_um2 = area_per_cell_um2 * cells as f64;
    let inverse_spacing_x_squared = (1.0 / spec.spacing_x_um) / spec.spacing_x_um;
    let inverse_spacing_y_squared = (1.0 / spec.spacing_y_um) / spec.spacing_y_um;
    let maximum_laplacian_eigenvalue =
        4.0 * (inverse_spacing_x_squared + inverse_spacing_y_squared);
    let maximum_nonlinear_lipschitz =
        spec.bulk_energy_scale * (3.0 * MAXIMUM_SOLVER_ABSOLUTE_PHASE.powi(2) + 1.0);
    let maximum_mobility_wave =
        spec.time_step * spec.mobility_um2_per_time * maximum_laplacian_eigenvalue;
    let maximum_gradient_wave = spec.gradient_energy_coefficient_um2 * maximum_laplacian_eigenvalue;
    let maximum_denominator_increment =
        maximum_mobility_wave * (maximum_nonlinear_lipschitz + maximum_gradient_wave);
    let maximum_bulk_update = maximum_mobility_wave * spec.bulk_energy_scale;
    let maximum_mass_magnitude = MAXIMUM_SOLVER_ABSOLUTE_PHASE * domain_area_um2;
    let finite = [
        maximum_x_um,
        maximum_y_um,
        area_per_cell_um2,
        domain_area_um2,
        inverse_spacing_x_squared,
        inverse_spacing_y_squared,
        maximum_laplacian_eigenvalue,
        maximum_nonlinear_lipschitz,
        maximum_mobility_wave,
        maximum_gradient_wave,
        maximum_denominator_increment,
        maximum_bulk_update,
        maximum_mass_magnitude,
    ]
    .iter()
    .all(|value| value.is_finite());
    if !finite
        || area_per_cell_um2 <= 0.0
        || domain_area_um2 <= 0.0
        || inverse_spacing_x_squared <= 0.0
        || inverse_spacing_y_squared <= 0.0
        || maximum_laplacian_eigenvalue <= 0.0
        || maximum_nonlinear_lipschitz <= 0.0
        || maximum_mobility_wave <= 0.0
    {
        return Err(SimulationError::Invalid(
            "phase-field derived geometry or operator is non-finite, underflowed, or degenerate"
                .into(),
        ));
    }
    Ok(())
}

fn trajectory_len(steps: u32, record_every_steps: u32) -> u64 {
    1 + u64::from(steps / record_every_steps) + u64::from(!steps.is_multiple_of(record_every_steps))
}

fn transform_2d(
    values: &mut [Complex<f64>],
    nx: usize,
    ny: usize,
    row_transform: &dyn Fft<f64>,
    column_transform: &dyn Fft<f64>,
    normalize: bool,
) {
    for row in values.chunks_exact_mut(nx) {
        row_transform.process(row);
    }
    let mut column = vec![Complex::new(0.0, 0.0); ny];
    for x in 0..nx {
        for y in 0..ny {
            column[y] = values[y * nx + x];
        }
        column_transform.process(&mut column);
        for y in 0..ny {
            values[y * nx + x] = column[y];
        }
    }
    if normalize {
        let scale = (nx * ny) as f64;
        for value in values {
            *value /= scale;
        }
    }
}

fn laplacian_eigenvalues(spec: &PhaseFieldSpec) -> Result<Vec<f64>, SimulationError> {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let inverse_spacing_x_squared = (1.0 / spec.spacing_x_um) / spec.spacing_x_um;
    let inverse_spacing_y_squared = (1.0 / spec.spacing_y_um) / spec.spacing_y_um;
    let values = (0..ny)
        .flat_map(|y| {
            (0..nx).map(move |x| {
                4.0 * (std::f64::consts::PI * x as f64 / nx as f64).sin().powi(2)
                    * inverse_spacing_x_squared
                    + 4.0
                        * (std::f64::consts::PI * y as f64 / ny as f64).sin().powi(2)
                        * inverse_spacing_y_squared
            })
        })
        .collect::<Vec<_>>();
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(SimulationError::Numerical(
            "phase-field Laplacian spectrum is not finite and nonnegative".into(),
        ));
    }
    Ok(values)
}

fn snapshot(time: f64, phase: &[f64], spec: &PhaseFieldSpec) -> PhaseFieldSnapshot {
    let mean_phase = mean(phase);
    let (bulk_free_energy, gradient_free_energy) = free_energy(phase, spec);
    let area_per_cell = spec.spacing_x_um * spec.spacing_y_um;
    PhaseFieldSnapshot {
        time,
        mean_phase,
        population_variance: phase
            .iter()
            .map(|value| (value - mean_phase).powi(2))
            .sum::<f64>()
            / phase.len() as f64,
        minimum_phase: phase.iter().copied().fold(f64::INFINITY, f64::min),
        maximum_phase: phase.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        mass: phase.iter().sum::<f64>() * area_per_cell,
        bulk_free_energy,
        gradient_free_energy,
        total_free_energy: bulk_free_energy + gradient_free_energy,
    }
}

fn validate_snapshot(snapshot: &PhaseFieldSnapshot, stage: &str) -> Result<(), SimulationError> {
    let finite = [
        snapshot.time,
        snapshot.mean_phase,
        snapshot.population_variance,
        snapshot.minimum_phase,
        snapshot.maximum_phase,
        snapshot.mass,
        snapshot.bulk_free_energy,
        snapshot.gradient_free_energy,
        snapshot.total_free_energy,
    ]
    .iter()
    .all(|value| value.is_finite());
    if !finite
        || snapshot.population_variance < 0.0
        || snapshot.minimum_phase > snapshot.maximum_phase
        || snapshot.bulk_free_energy < 0.0
        || snapshot.gradient_free_energy < 0.0
        || snapshot.total_free_energy < 0.0
    {
        return Err(SimulationError::Numerical(format!(
            "phase-field {stage} snapshot is non-finite or incoherent"
        )));
    }
    Ok(())
}

fn free_energy(phase: &[f64], spec: &PhaseFieldSpec) -> (f64, f64) {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let area = spec.spacing_x_um * spec.spacing_y_um;
    let bulk = phase
        .iter()
        .map(|value| 0.25 * spec.bulk_energy_scale * (value.powi(2) - 1.0).powi(2))
        .sum::<f64>()
        * area;
    let mut squared_gradient = 0.0;
    for y in 0..ny {
        let above = (y + 1) % ny;
        for x in 0..nx {
            let right = (x + 1) % nx;
            let value = phase[y * nx + x];
            squared_gradient += ((phase[y * nx + right] - value) / spec.spacing_x_um).powi(2);
            squared_gradient += ((phase[above * nx + x] - value) / spec.spacing_y_um).powi(2);
        }
    }
    let gradient = 0.5 * spec.gradient_energy_coefficient_um2 * squared_gradient * area;
    (bulk, gradient)
}

fn state(phase: &[f64], spec: &PhaseFieldSpec) -> Vec<PhaseFieldStateCell> {
    phase
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let ix = index % spec.grid_x as usize;
            let iy = index / spec.grid_x as usize;
            PhaseFieldStateCell {
                ix: ix as u32,
                iy: iy as u32,
                x_um: ix as f64 * spec.spacing_x_um,
                y_um: iy as f64 * spec.spacing_y_um,
                phase: *value,
            }
        })
        .collect()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
