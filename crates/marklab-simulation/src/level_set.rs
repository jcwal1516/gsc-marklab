use serde::Serialize;

use super::SimulationError;

#[derive(Clone, Debug)]
pub struct LevelSetSpec {
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub initial_phi_row_major: Vec<f64>,
    pub normal_speed_um_per_time_row_major: Vec<f64>,
    pub curvature_weight_um2_per_time: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub reinitialize_every_steps: u32,
    pub record_every_steps: u32,
    pub maximum_cell_steps: u64,
    pub maximum_reinitialization_distance_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LevelSetStateCell {
    pub ix: u32,
    pub iy: u32,
    pub x_um: f64,
    pub y_um: f64,
    pub phi: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LevelSetInterfaceDiagnostics {
    pub exact_zero_nodes: u32,
    pub horizontal_crossings: u32,
    pub vertical_crossings: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct LevelSetSnapshot {
    pub time: f64,
    pub minimum_phi: f64,
    pub maximum_phi: f64,
    pub interface: LevelSetInterfaceDiagnostics,
}

#[derive(Clone, Debug, Serialize)]
pub struct LevelSetSolverDiagnostics {
    pub method: &'static str,
    pub boundary_condition: &'static str,
    pub completed_steps: u32,
    pub cell_steps: u64,
    pub reinitializations: u32,
    pub reinitialization_distance_visits: u64,
    pub advective_cfl_number: f64,
    pub curvature_cfl_number: f64,
    pub combined_cfl_number: f64,
    pub cfl_limit: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LevelSetResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub curvature_weight_um2_per_time: f64,
    pub final_time: f64,
    pub requested_time_step: f64,
    pub reinitialize_every_steps: u32,
    pub initial_state: Vec<LevelSetStateCell>,
    pub final_state: Vec<LevelSetStateCell>,
    pub initial_interface: LevelSetInterfaceDiagnostics,
    pub final_interface: LevelSetInterfaceDiagnostics,
    pub trajectory: Vec<LevelSetSnapshot>,
    pub solver: LevelSetSolverDiagnostics,
    pub observation_model: &'static str,
    pub claim_status: &'static str,
}

pub fn evolve_interface_level_set(spec: LevelSetSpec) -> Result<LevelSetResult, SimulationError> {
    let (cells, planned_steps, advective_cfl, curvature_cfl) = validate(&spec)?;
    let initial_interface = interface_diagnostics(&spec.initial_phi_row_major, &spec);
    if interface_point_samples(&spec.initial_phi_row_major, &spec).is_empty() {
        return Err(SimulationError::Invalid(
            "level-set field contains no zero contour".into(),
        ));
    }
    let initial_state = state(&spec.initial_phi_row_major, &spec);
    let mut phi = spec.initial_phi_row_major.clone();
    let mut trajectory = vec![snapshot(0.0, &phi, &spec)];
    let mut completed_steps = 0_u32;
    let mut reinitializations = 0_u32;
    let mut distance_visits = 0_u64;
    let mut time = 0.0;

    while completed_steps < planned_steps {
        let next_time = if completed_steps + 1 == planned_steps {
            spec.final_time
        } else {
            f64::from(completed_steps + 1) * spec.time_step
        };
        advance(&mut phi, &spec, next_time - time)?;
        time = next_time;
        completed_steps += 1;
        if spec.reinitialize_every_steps != 0
            && completed_steps.is_multiple_of(spec.reinitialize_every_steps)
        {
            reinitialize(&mut phi, &spec, &mut distance_visits)?;
            reinitializations += 1;
        }
        if completed_steps.is_multiple_of(spec.record_every_steps)
            || completed_steps == planned_steps
        {
            trajectory.push(snapshot(time, &phi, &spec));
        }
    }

    Ok(LevelSetResult {
        format: "marklab.level_set_interface",
        version: 1,
        simulator_id: "regular_2d_upwind_level_set",
        simulator_version: 1,
        dimensionality: 2,
        grid_x: spec.grid_x,
        grid_y: spec.grid_y,
        spacing_x_um: spec.spacing_x_um,
        spacing_y_um: spec.spacing_y_um,
        curvature_weight_um2_per_time: spec.curvature_weight_um2_per_time,
        final_time: spec.final_time,
        requested_time_step: spec.time_step,
        reinitialize_every_steps: spec.reinitialize_every_steps,
        initial_state,
        final_state: state(&phi, &spec),
        initial_interface,
        final_interface: interface_diagnostics(&phi, &spec),
        trajectory,
        solver: LevelSetSolverDiagnostics {
            method: "first_order_godunov_upwind_with_centered_curvature",
            boundary_condition: "linear_extrapolation_ghost",
            completed_steps,
            cell_steps: u64::from(completed_steps) * cells as u64,
            reinitializations,
            reinitialization_distance_visits: distance_visits,
            advective_cfl_number: advective_cfl,
            curvature_cfl_number: curvature_cfl,
            combined_cfl_number: advective_cfl + curvature_cfl,
            cfl_limit: 0.5,
        },
        observation_model: "identity_zero_contour_observation",
        claim_status: "experimental_interface_not_tumour_forecast",
    })
}

fn validate(spec: &LevelSetSpec) -> Result<(usize, u32, f64, f64), SimulationError> {
    let cells = (spec.grid_x as usize)
        .checked_mul(spec.grid_y as usize)
        .ok_or_else(|| SimulationError::Invalid("level-set grid size overflowed".into()))?;
    let valid = (3..=256).contains(&spec.grid_x)
        && (3..=256).contains(&spec.grid_y)
        && cells <= 65_536
        && spec.initial_phi_row_major.len() == cells
        && spec.normal_speed_um_per_time_row_major.len() == cells
        && spec.spacing_x_um.is_finite()
        && spec.spacing_x_um > 0.0
        && spec.spacing_y_um.is_finite()
        && spec.spacing_y_um > 0.0
        && spec
            .initial_phi_row_major
            .iter()
            .all(|value| value.is_finite())
        && spec
            .normal_speed_um_per_time_row_major
            .iter()
            .all(|value| value.is_finite())
        && spec.curvature_weight_um2_per_time.is_finite()
        && spec.curvature_weight_um2_per_time >= 0.0
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && spec.time_step.is_finite()
        && spec.time_step > 0.0
        && spec.record_every_steps > 0
        && (1..=250_000_000).contains(&spec.maximum_cell_steps)
        && spec.maximum_reinitialization_distance_visits <= 250_000_000;
    if !valid {
        return Err(SimulationError::Invalid(
            "level-set grid, fields, or controls are invalid".into(),
        ));
    }
    let steps = (spec.final_time / spec.time_step).ceil();
    if !steps.is_finite() || steps > f64::from(u32::MAX) {
        return Err(SimulationError::Invalid(
            "level-set step count is invalid".into(),
        ));
    }
    let work = steps as u64 * cells as u64;
    if work > spec.maximum_cell_steps {
        return Err(SimulationError::Invalid(format!(
            "{work} cell-steps exceed the declared resource bound"
        )));
    }
    let maximum_speed = spec
        .normal_speed_um_per_time_row_major
        .iter()
        .copied()
        .map(f64::abs)
        .fold(0.0, f64::max);
    let advective_cfl =
        spec.time_step * maximum_speed * (spec.spacing_x_um.recip() + spec.spacing_y_um.recip());
    let curvature_cfl = 2.0
        * spec.curvature_weight_um2_per_time
        * spec.time_step
        * (spec.spacing_x_um.recip().powi(2) + spec.spacing_y_um.recip().powi(2));
    if !advective_cfl.is_finite()
        || !curvature_cfl.is_finite()
        || advective_cfl + curvature_cfl > 0.5 + 1e-14
    {
        return Err(SimulationError::Invalid(format!(
            "level-set combined CFL {} exceeds 0.5",
            advective_cfl + curvature_cfl
        )));
    }
    Ok((cells, steps as u32, advective_cfl, curvature_cfl))
}

fn advance(phi: &mut [f64], spec: &LevelSetSpec, dt: f64) -> Result<(), SimulationError> {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let previous = phi.to_vec();
    for y in 0..ny {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(ny - 1);
        for x in 0..nx {
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(nx - 1);
            let index = y * nx + x;
            let mut backward_x = (previous[index] - previous[y * nx + xm]) / spec.spacing_x_um;
            let mut forward_x = (previous[y * nx + xp] - previous[index]) / spec.spacing_x_um;
            let mut backward_y = (previous[index] - previous[ym * nx + x]) / spec.spacing_y_um;
            let mut forward_y = (previous[yp * nx + x] - previous[index]) / spec.spacing_y_um;
            if x == 0 {
                backward_x = forward_x;
            } else if x + 1 == nx {
                forward_x = backward_x;
            }
            if y == 0 {
                backward_y = forward_y;
            } else if y + 1 == ny {
                forward_y = backward_y;
            }
            let curvature = curvature(&previous, x, y, spec);
            let normal_speed = spec.normal_speed_um_per_time_row_major[index]
                + spec.curvature_weight_um2_per_time * curvature;
            let gradient =
                godunov_gradient(normal_speed, backward_x, forward_x, backward_y, forward_y);
            phi[index] = previous[index] - dt * normal_speed * gradient;
        }
    }
    if phi.iter().any(|value| !value.is_finite()) {
        return Err(SimulationError::Numerical(
            "level-set step became non-finite".into(),
        ));
    }
    Ok(())
}

fn godunov_gradient(speed: f64, bx: f64, fx: f64, by: f64, fy: f64) -> f64 {
    let squared = if speed >= 0.0 {
        bx.max(0.0).powi(2) + fx.min(0.0).powi(2) + by.max(0.0).powi(2) + fy.min(0.0).powi(2)
    } else {
        bx.min(0.0).powi(2) + fx.max(0.0).powi(2) + by.min(0.0).powi(2) + fy.max(0.0).powi(2)
    };
    squared.sqrt()
}

fn curvature(phi: &[f64], x: usize, y: usize, spec: &LevelSetSpec) -> f64 {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let xm = x.saturating_sub(1);
    let xp = (x + 1).min(nx - 1);
    let ym = y.saturating_sub(1);
    let yp = (y + 1).min(ny - 1);
    let center = phi[y * nx + x];
    let dx_span = (xp - xm) as f64 * spec.spacing_x_um;
    let dy_span = (yp - ym) as f64 * spec.spacing_y_um;
    let gx = (phi[y * nx + xp] - phi[y * nx + xm]) / dx_span;
    let gy = (phi[yp * nx + x] - phi[ym * nx + x]) / dy_span;
    let gxx = (phi[y * nx + xp] - 2.0 * center + phi[y * nx + xm]) / spec.spacing_x_um.powi(2);
    let gyy = (phi[yp * nx + x] - 2.0 * center + phi[ym * nx + x]) / spec.spacing_y_um.powi(2);
    let gxy = (phi[yp * nx + xp] - phi[yp * nx + xm] - phi[ym * nx + xp] + phi[ym * nx + xm])
        / (dx_span * dy_span);
    let gradient_squared = gx * gx + gy * gy;
    if gradient_squared <= 1e-24 {
        0.0
    } else {
        (gxx * gy * gy - 2.0 * gx * gy * gxy + gyy * gx * gx) / gradient_squared.powf(1.5)
    }
}

fn reinitialize(
    phi: &mut [f64],
    spec: &LevelSetSpec,
    distance_visits: &mut u64,
) -> Result<(), SimulationError> {
    let samples = interface_point_samples(phi, spec);
    if samples.is_empty() {
        return Err(SimulationError::Numerical(
            "level-set zero contour vanished before reinitialization".into(),
        ));
    }
    let visits = (phi.len() as u64)
        .checked_mul(samples.len() as u64)
        .and_then(|value| distance_visits.checked_add(value))
        .ok_or_else(|| SimulationError::Numerical("reinitialization work overflowed".into()))?;
    if visits > spec.maximum_reinitialization_distance_visits {
        return Err(SimulationError::Invalid(format!(
            "{visits} reinitialization distance visits exceed the declared resource bound"
        )));
    }
    let previous = phi.to_vec();
    for (index, value) in phi.iter_mut().enumerate() {
        if previous[index] == 0.0 {
            *value = 0.0;
            continue;
        }
        let x = (index % spec.grid_x as usize) as f64 * spec.spacing_x_um;
        let y = (index / spec.grid_x as usize) as f64 * spec.spacing_y_um;
        let distance = samples
            .iter()
            .map(|(sample_x, sample_y)| (x - sample_x).hypot(y - sample_y))
            .fold(f64::INFINITY, f64::min);
        *value = previous[index].signum() * distance;
    }
    *distance_visits = visits;
    Ok(())
}

fn interface_point_samples(phi: &[f64], spec: &LevelSetSpec) -> Vec<(f64, f64)> {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let mut samples = Vec::new();
    for y in 0..ny {
        for x in 0..nx {
            let index = y * nx + x;
            if phi[index] == 0.0 {
                samples.push((x as f64 * spec.spacing_x_um, y as f64 * spec.spacing_y_um));
            }
            if x + 1 < nx && strict_crossing(phi[index], phi[index + 1]) {
                let fraction = phi[index] / (phi[index] - phi[index + 1]);
                samples.push((
                    (x as f64 + fraction) * spec.spacing_x_um,
                    y as f64 * spec.spacing_y_um,
                ));
            }
            if y + 1 < ny && strict_crossing(phi[index], phi[index + nx]) {
                let fraction = phi[index] / (phi[index] - phi[index + nx]);
                samples.push((
                    x as f64 * spec.spacing_x_um,
                    (y as f64 + fraction) * spec.spacing_y_um,
                ));
            }
        }
    }
    samples
}

fn strict_crossing(left: f64, right: f64) -> bool {
    (left < 0.0 && right > 0.0) || (left > 0.0 && right < 0.0)
}

fn interface_diagnostics(phi: &[f64], spec: &LevelSetSpec) -> LevelSetInterfaceDiagnostics {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let exact_zero_nodes = phi.iter().filter(|value| **value == 0.0).count() as u32;
    let mut horizontal_crossings = 0_u32;
    for y in 0..ny {
        horizontal_crossings += phi[y * nx..(y + 1) * nx]
            .iter()
            .filter(|value| **value == 0.0)
            .count() as u32;
        horizontal_crossings += (0..nx - 1)
            .filter(|x| strict_crossing(phi[y * nx + *x], phi[y * nx + *x + 1]))
            .count() as u32;
    }
    let mut vertical_crossings = 0_u32;
    for x in 0..nx {
        vertical_crossings += (0..ny).filter(|y| phi[*y * nx + x] == 0.0).count() as u32;
        vertical_crossings += (0..ny - 1)
            .filter(|y| strict_crossing(phi[*y * nx + x], phi[(*y + 1) * nx + x]))
            .count() as u32;
    }
    LevelSetInterfaceDiagnostics {
        exact_zero_nodes,
        horizontal_crossings,
        vertical_crossings,
    }
}

fn state(phi: &[f64], spec: &LevelSetSpec) -> Vec<LevelSetStateCell> {
    phi.iter()
        .enumerate()
        .map(|(index, value)| {
            let ix = index % spec.grid_x as usize;
            let iy = index / spec.grid_x as usize;
            LevelSetStateCell {
                ix: ix as u32,
                iy: iy as u32,
                x_um: ix as f64 * spec.spacing_x_um,
                y_um: iy as f64 * spec.spacing_y_um,
                phi: *value,
            }
        })
        .collect()
}

fn snapshot(time: f64, phi: &[f64], spec: &LevelSetSpec) -> LevelSetSnapshot {
    LevelSetSnapshot {
        time,
        minimum_phi: phi.iter().copied().fold(f64::INFINITY, f64::min),
        maximum_phi: phi.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        interface: interface_diagnostics(phi, spec),
    }
}
