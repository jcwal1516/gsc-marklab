use std::collections::VecDeque;

use super::SimulationError;

mod mapping;
mod types;

use mapping::{map_uptake_cells, map_vessels, validate_entities};
pub use types::*;

pub fn simulate_vascular_transport(
    mut spec: VascularTransportSpec,
) -> Result<VascularTransportResult, SimulationError> {
    spec.vessel_sources
        .sort_by(|left, right| left.vessel_id.cmp(&right.vessel_id));
    spec.uptake_cells
        .sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    let (cells, planned_steps, diffusive_cfl, advective_cfl) = validate(&spec)?;
    let (mapped_vessels, sources) = map_vessels(&spec)?;
    let (mapped_cells, uptake) = map_uptake_cells(&spec)?;
    let initial_mass = mass(&spec.initial_concentration_row_major, &spec);
    let initial_state = state(
        &spec.initial_concentration_row_major,
        &sources,
        &uptake,
        &spec,
    );
    let mut concentration = spec.initial_concentration_row_major.clone();
    let mut trajectory = vec![snapshot(0.0, &concentration, &spec)];
    let mut completed_steps = 0_u32;
    let mut time = 0.0;
    let mut source_mass = 0.0;
    let mut uptake_mass = 0.0;
    let mut maximum_transport_residual = 0.0_f64;
    let positivity_violations = 0_u64;

    while completed_steps < planned_steps {
        let next_time = if completed_steps + 1 == planned_steps {
            spec.final_time
        } else {
            f64::from(completed_steps + 1) * spec.time_step
        };
        let dt = next_time - time;
        let (added, removed) = local_flow(&mut concentration, &sources, &uptake, &spec, dt / 2.0)?;
        source_mass += added;
        uptake_mass += removed;
        let before_transport = mass(&concentration, &spec);
        transport_step(&mut concentration, &spec, dt)?;
        let transport_residual = (mass(&concentration, &spec) - before_transport).abs();
        maximum_transport_residual = maximum_transport_residual.max(transport_residual);
        let (added, removed) = local_flow(&mut concentration, &sources, &uptake, &spec, dt / 2.0)?;
        source_mass += added;
        uptake_mass += removed;
        if concentration
            .iter()
            .any(|value| !value.is_finite() || *value < -1e-12)
        {
            return Err(SimulationError::Numerical(
                "vascular concentration became non-finite or negative".into(),
            ));
        }
        for value in &mut concentration {
            *value = value.max(0.0);
        }
        time = next_time;
        completed_steps += 1;
        if completed_steps.is_multiple_of(spec.record_every_steps)
            || completed_steps == planned_steps
        {
            trajectory.push(snapshot(time, &concentration, &spec));
        }
    }
    let final_mass = mass(&concentration, &spec);
    Ok(VascularTransportResult {
        format: "marklab.vascular_transport",
        version: 1,
        simulator_id: "regular_2d_static_flow_vascular_transport",
        simulator_version: 1,
        dimensionality: 2,
        grid_x: spec.grid_x,
        grid_y: spec.grid_y,
        spacing_x_um: spec.spacing_x_um,
        spacing_y_um: spec.spacing_y_um,
        final_time: spec.final_time,
        requested_time_step: spec.time_step,
        hypoxia_threshold: spec.hypoxia_threshold,
        mapped_vessel_sources: mapped_vessels,
        mapped_uptake_cells: mapped_cells,
        initial_state,
        final_state: state(&concentration, &sources, &uptake, &spec),
        hypoxic_regions: hypoxic_regions(&concentration, &spec),
        trajectory,
        solver: VascularTransportSolverDiagnostics {
            method: "strang_exact_source_uptake_plus_conservative_explicit_transport",
            boundary_condition: "no_flux",
            flow_approximation: "caller_declared_static_velocity",
            completed_steps,
            cell_steps: u64::from(completed_steps) * cells as u64,
            diffusive_cfl_number: diffusive_cfl,
            advective_cfl_number: advective_cfl,
            combined_cfl_number: diffusive_cfl + advective_cfl,
            cfl_limit: 1.0,
            cumulative_vessel_source_mass: source_mass,
            cumulative_cell_uptake_mass: uptake_mass,
            maximum_transport_mass_residual: maximum_transport_residual,
            mass_balance_residual: final_mass - initial_mass - source_mass + uptake_mass,
            positivity_violations,
        },
        observation_model: "identity_concentration_field",
        claim_status: "advanced_transport_not_causal_or_hemodynamic_truth",
    })
}

fn validate(spec: &VascularTransportSpec) -> Result<(usize, u32, f64, f64), SimulationError> {
    let cells = (spec.grid_x as usize)
        .checked_mul(spec.grid_y as usize)
        .ok_or_else(|| SimulationError::Invalid("vascular grid size overflowed".into()))?;
    let fields = [
        &spec.initial_concentration_row_major,
        &spec.diffusion_um2_per_time_row_major,
        &spec.velocity_x_um_per_time_row_major,
        &spec.velocity_y_um_per_time_row_major,
    ];
    let valid = (3..=256).contains(&spec.grid_x)
        && (3..=256).contains(&spec.grid_y)
        && cells <= 65_536
        && fields.iter().all(|field| field.len() == cells)
        && spec.spacing_x_um.is_finite()
        && spec.spacing_x_um > 0.0
        && spec.spacing_y_um.is_finite()
        && spec.spacing_y_um > 0.0
        && spec
            .initial_concentration_row_major
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        && spec
            .diffusion_um2_per_time_row_major
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        && spec
            .velocity_x_um_per_time_row_major
            .iter()
            .chain(&spec.velocity_y_um_per_time_row_major)
            .all(|value| value.is_finite())
        && spec.flow_approximation == "caller_declared_static_velocity"
        && spec.vessel_sources.len() <= 100_000
        && spec.uptake_cells.len() <= 1_000_000
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && spec.time_step.is_finite()
        && spec.time_step > 0.0
        && spec.hypoxia_threshold.is_finite()
        && spec.hypoxia_threshold >= 0.0
        && spec.record_every_steps > 0
        && (1..=250_000_000).contains(&spec.maximum_cell_steps);
    if !valid {
        return Err(SimulationError::Invalid(
            "vascular grid, fields, flow declaration, or controls are invalid".into(),
        ));
    }
    validate_entities(&spec.vessel_sources, &spec.uptake_cells)?;
    let steps = (spec.final_time / spec.time_step).ceil();
    if !steps.is_finite() || steps > f64::from(u32::MAX) {
        return Err(SimulationError::Invalid(
            "vascular step count is invalid".into(),
        ));
    }
    let work = steps as u64 * cells as u64;
    if work > spec.maximum_cell_steps {
        return Err(SimulationError::Invalid(format!(
            "{work} cell-steps exceed the declared resource bound"
        )));
    }
    let (diffusive, advective) = transport_cfl(spec);
    if !diffusive.is_finite() || !advective.is_finite() || diffusive + advective > 1.0 + 1e-14 {
        return Err(SimulationError::Invalid(format!(
            "vascular combined transport CFL {} exceeds 1",
            diffusive + advective
        )));
    }
    Ok((cells, steps as u32, diffusive, advective))
}

fn transport_cfl(spec: &VascularTransportSpec) -> (f64, f64) {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let mut maximum_diffusion = 0.0_f64;
    let mut maximum_advection = 0.0_f64;
    for y in 0..ny {
        for x in 0..nx {
            let index = y * nx + x;
            let mut diffusion = 0.0;
            let mut advection = 0.0;
            if x + 1 < nx {
                diffusion += face(
                    spec.diffusion_um2_per_time_row_major[index],
                    spec.diffusion_um2_per_time_row_major[index + 1],
                ) / spec.spacing_x_um.powi(2);
                advection += face(
                    spec.velocity_x_um_per_time_row_major[index],
                    spec.velocity_x_um_per_time_row_major[index + 1],
                )
                .max(0.0)
                    / spec.spacing_x_um;
            }
            if x > 0 {
                diffusion += face(
                    spec.diffusion_um2_per_time_row_major[index],
                    spec.diffusion_um2_per_time_row_major[index - 1],
                ) / spec.spacing_x_um.powi(2);
                advection += (-face(
                    spec.velocity_x_um_per_time_row_major[index],
                    spec.velocity_x_um_per_time_row_major[index - 1],
                ))
                .max(0.0)
                    / spec.spacing_x_um;
            }
            if y + 1 < ny {
                diffusion += face(
                    spec.diffusion_um2_per_time_row_major[index],
                    spec.diffusion_um2_per_time_row_major[index + nx],
                ) / spec.spacing_y_um.powi(2);
                advection += face(
                    spec.velocity_y_um_per_time_row_major[index],
                    spec.velocity_y_um_per_time_row_major[index + nx],
                )
                .max(0.0)
                    / spec.spacing_y_um;
            }
            if y > 0 {
                diffusion += face(
                    spec.diffusion_um2_per_time_row_major[index],
                    spec.diffusion_um2_per_time_row_major[index - nx],
                ) / spec.spacing_y_um.powi(2);
                advection += (-face(
                    spec.velocity_y_um_per_time_row_major[index],
                    spec.velocity_y_um_per_time_row_major[index - nx],
                ))
                .max(0.0)
                    / spec.spacing_y_um;
            }
            maximum_diffusion = maximum_diffusion.max(diffusion * spec.time_step);
            maximum_advection = maximum_advection.max(advection * spec.time_step);
        }
    }
    (maximum_diffusion, maximum_advection)
}

fn face(left: f64, right: f64) -> f64 {
    (left + right) / 2.0
}

fn local_flow(
    concentration: &mut [f64],
    source: &[f64],
    uptake: &[f64],
    spec: &VascularTransportSpec,
    dt: f64,
) -> Result<(f64, f64), SimulationError> {
    let area = spec.spacing_x_um * spec.spacing_y_um;
    let mut source_mass = 0.0;
    let mut uptake_mass = 0.0;
    for index in 0..concentration.len() {
        let before = concentration[index];
        concentration[index] = if uptake[index] == 0.0 {
            before + source[index] * dt
        } else {
            let decay = (-uptake[index] * dt).exp();
            before * decay + source[index] / uptake[index] * (1.0 - decay)
        };
        if !concentration[index].is_finite() {
            return Err(SimulationError::Numerical(
                "vascular source/uptake flow became non-finite".into(),
            ));
        }
        let gross_source = source[index] * dt * area;
        source_mass += gross_source;
        uptake_mass += gross_source - (concentration[index] - before) * area;
    }
    Ok((source_mass, uptake_mass.max(0.0)))
}

fn transport_step(
    concentration: &mut [f64],
    spec: &VascularTransportSpec,
    dt: f64,
) -> Result<(), SimulationError> {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let previous = concentration.to_vec();
    let mut delta = vec![0.0; concentration.len()];
    for y in 0..ny {
        for x in 0..nx - 1 {
            let left = y * nx + x;
            let right = left + 1;
            let diffusion = face(
                spec.diffusion_um2_per_time_row_major[left],
                spec.diffusion_um2_per_time_row_major[right],
            );
            let velocity = face(
                spec.velocity_x_um_per_time_row_major[left],
                spec.velocity_x_um_per_time_row_major[right],
            );
            let flux = diffusion * (previous[left] - previous[right]) / spec.spacing_x_um
                + velocity
                    * if velocity >= 0.0 {
                        previous[left]
                    } else {
                        previous[right]
                    };
            delta[left] -= dt * flux / spec.spacing_x_um;
            delta[right] += dt * flux / spec.spacing_x_um;
        }
    }
    for y in 0..ny - 1 {
        for x in 0..nx {
            let lower = y * nx + x;
            let upper = lower + nx;
            let diffusion = face(
                spec.diffusion_um2_per_time_row_major[lower],
                spec.diffusion_um2_per_time_row_major[upper],
            );
            let velocity = face(
                spec.velocity_y_um_per_time_row_major[lower],
                spec.velocity_y_um_per_time_row_major[upper],
            );
            let flux = diffusion * (previous[lower] - previous[upper]) / spec.spacing_y_um
                + velocity
                    * if velocity >= 0.0 {
                        previous[lower]
                    } else {
                        previous[upper]
                    };
            delta[lower] -= dt * flux / spec.spacing_y_um;
            delta[upper] += dt * flux / spec.spacing_y_um;
        }
    }
    for (value, change) in concentration.iter_mut().zip(delta) {
        *value += change;
    }
    if concentration.iter().any(|value| !value.is_finite()) {
        return Err(SimulationError::Numerical(
            "vascular transport step became non-finite".into(),
        ));
    }
    Ok(())
}

fn state(
    concentration: &[f64],
    source: &[f64],
    uptake: &[f64],
    spec: &VascularTransportSpec,
) -> Vec<VascularTransportStateCell> {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    concentration
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = index % nx;
            let y = index / nx;
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(nx - 1);
            let ym = y.saturating_sub(1);
            let yp = (y + 1).min(ny - 1);
            let gx = (concentration[y * nx + xp] - concentration[y * nx + xm])
                / ((xp - xm) as f64 * spec.spacing_x_um);
            let gy = (concentration[yp * nx + x] - concentration[ym * nx + x])
                / ((yp - ym) as f64 * spec.spacing_y_um);
            VascularTransportStateCell {
                ix: x as u32,
                iy: y as u32,
                x_um: x as f64 * spec.spacing_x_um,
                y_um: y as f64 * spec.spacing_y_um,
                concentration: *value,
                gradient_magnitude_per_um: gx.hypot(gy),
                vessel_source_concentration_per_time: source[index],
                linear_uptake_per_time: uptake[index],
                hypoxic: *value < spec.hypoxia_threshold,
            }
        })
        .collect()
}

fn snapshot(
    time: f64,
    concentration: &[f64],
    spec: &VascularTransportSpec,
) -> VascularTransportSnapshot {
    VascularTransportSnapshot {
        time,
        total_concentration_mass: mass(concentration, spec),
        minimum_concentration: concentration.iter().copied().fold(f64::INFINITY, f64::min),
        maximum_concentration: concentration
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max),
        hypoxic_cells: concentration
            .iter()
            .filter(|value| **value < spec.hypoxia_threshold)
            .count() as u32,
    }
}

fn hypoxic_regions(
    concentration: &[f64],
    spec: &VascularTransportSpec,
) -> Vec<VascularHypoxicRegion> {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let mut visited = vec![false; concentration.len()];
    let mut regions = Vec::new();
    for start in 0..concentration.len() {
        if visited[start] || concentration[start] >= spec.hypoxia_threshold {
            continue;
        }
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut indices = Vec::new();
        let mut minimum = f64::INFINITY;
        while let Some(index) = queue.pop_front() {
            indices.push(index);
            minimum = minimum.min(concentration[index]);
            let x = index % nx;
            let y = index / nx;
            let neighbors = [
                x.checked_sub(1).map(|value| y * nx + value),
                (x + 1 < nx).then_some(y * nx + x + 1),
                y.checked_sub(1).map(|value| value * nx + x),
                (y + 1 < ny).then_some((y + 1) * nx + x),
            ];
            for neighbor in neighbors.into_iter().flatten() {
                if !visited[neighbor] && concentration[neighbor] < spec.hypoxia_threshold {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        indices.sort_unstable();
        regions.push(VascularHypoxicRegion {
            region_id: regions.len() as u32 + 1,
            cell_count: indices.len() as u32,
            area_um2: indices.len() as f64 * spec.spacing_x_um * spec.spacing_y_um,
            minimum_concentration: minimum,
            grid_indices: indices,
        });
    }
    regions
}

fn mass(concentration: &[f64], spec: &VascularTransportSpec) -> f64 {
    concentration.iter().sum::<f64>() * spec.spacing_x_um * spec.spacing_y_um
}
