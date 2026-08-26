use rustfft::{num_complex::Complex, FftPlanner};
use serde::{Deserialize, Serialize};

use super::SimulationError;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "model", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReactionModel {
    Linear {
        rate_per_time: f64,
    },
    Logistic {
        rate_per_time: f64,
        carrying_capacity: f64,
    },
}

impl ReactionModel {
    fn validate(self) -> bool {
        match self {
            Self::Linear { rate_per_time } => rate_per_time.is_finite(),
            Self::Logistic {
                rate_per_time,
                carrying_capacity,
            } => {
                rate_per_time.is_finite()
                    && rate_per_time >= 0.0
                    && carrying_capacity.is_finite()
                    && carrying_capacity > 0.0
            }
        }
    }

    fn apply(self, values: &mut [f64], dt: f64) -> Result<(), SimulationError> {
        match self {
            Self::Linear { rate_per_time } => {
                let multiplier = (rate_per_time * dt).exp();
                for value in values.iter_mut() {
                    *value *= multiplier;
                }
            }
            Self::Logistic {
                rate_per_time,
                carrying_capacity,
            } => {
                let decay = (-rate_per_time * dt).exp();
                for value in values.iter_mut() {
                    if *value > 0.0 {
                        *value = carrying_capacity * *value
                            / (*value + (carrying_capacity - *value) * decay);
                    }
                }
            }
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(SimulationError::Numerical(
                "reaction flow became non-finite".into(),
            ));
        }
        Ok(())
    }

    fn value(self, state: f64) -> f64 {
        match self {
            Self::Linear { rate_per_time } => rate_per_time * state,
            Self::Logistic {
                rate_per_time,
                carrying_capacity,
            } => rate_per_time * state * (1.0 - state / carrying_capacity),
        }
    }

    fn derivative(self, state: f64) -> f64 {
        match self {
            Self::Linear { rate_per_time } => rate_per_time,
            Self::Logistic {
                rate_per_time,
                carrying_capacity,
            } => rate_per_time * (1.0 - 2.0 * state / carrying_capacity),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReactionDiffusionSpec {
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub initial_row_major: Vec<f64>,
    pub diffusion_um2_per_time: f64,
    pub reaction: ReactionModel,
    pub final_time: f64,
    pub time_step: f64,
    pub record_every_steps: u32,
    pub maximum_cell_steps: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReactionDiffusionStateCell {
    pub ix: u32,
    pub iy: u32,
    pub x_um: f64,
    pub y_um: f64,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReactionDiffusionSnapshot {
    pub time: f64,
    pub mean: f64,
    pub population_variance: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub spatial_integral: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReactionDiffusionPatternDiagnostics {
    pub spectrum_basis: &'static str,
    pub dominant_wavelength_um: Option<f64>,
    pub dominant_frequency_x_cycles_per_um: Option<f64>,
    pub dominant_frequency_y_cycles_per_um: Option<f64>,
    pub dominant_power_fraction: Option<f64>,
    pub reference_homogeneous_state: f64,
    pub homogeneous_reaction_residual: f64,
    pub linearized_reaction_slope: f64,
    pub unstable_wavelength_min_um: Option<f64>,
    pub unstable_wavelength_max_um: Option<f64>,
    pub linear_instability_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReactionDiffusionSolverDiagnostics {
    pub method: &'static str,
    pub boundary_condition: &'static str,
    pub completed_steps: u32,
    pub cell_steps: u64,
    pub maximum_cfl_number: f64,
    pub cfl_limit: f64,
    pub positivity_violations: u64,
    pub minimum_value: f64,
    pub maximum_value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReactionDiffusionResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub diffusion_um2_per_time: f64,
    pub reaction: ReactionModel,
    pub final_time: f64,
    pub requested_time_step: f64,
    pub initial_state: Vec<ReactionDiffusionStateCell>,
    pub final_state: Vec<ReactionDiffusionStateCell>,
    pub trajectory: Vec<ReactionDiffusionSnapshot>,
    pub pattern: ReactionDiffusionPatternDiagnostics,
    pub solver: ReactionDiffusionSolverDiagnostics,
    pub observation_model: &'static str,
    pub claim_status: &'static str,
}

pub fn simulate_reaction_diffusion(
    spec: ReactionDiffusionSpec,
) -> Result<ReactionDiffusionResult, SimulationError> {
    let (cells, planned_steps) = validate(&spec)?;
    let initial_state = state(&spec, &spec.initial_row_major);
    let reference_state = mean(&spec.initial_row_major);
    let mut values = spec.initial_row_major.clone();
    let mut trajectory = vec![snapshot(0.0, &values, &spec)];
    let mut time = 0.0;
    let mut completed_steps = 0_u32;
    let mut positivity_violations = 0_u64;
    while completed_steps < planned_steps {
        let next_time = if completed_steps + 1 == planned_steps {
            spec.final_time
        } else {
            f64::from(completed_steps + 1) * spec.time_step
        };
        let dt = next_time - time;
        spec.reaction.apply(&mut values, dt / 2.0)?;
        diffusion_step(&mut values, &spec, dt)?;
        spec.reaction.apply(&mut values, dt / 2.0)?;
        for value in &values {
            if !value.is_finite() {
                return Err(SimulationError::Numerical(
                    "reaction-diffusion state became non-finite".into(),
                ));
            }
            if *value < -1e-12 {
                positivity_violations += 1;
            }
        }
        if positivity_violations != 0 {
            return Err(SimulationError::Numerical(
                "reaction-diffusion state became negative".into(),
            ));
        }
        time = next_time;
        completed_steps = completed_steps
            .checked_add(1)
            .ok_or_else(|| SimulationError::Numerical("step count overflowed".into()))?;
        if completed_steps.is_multiple_of(spec.record_every_steps) || time == spec.final_time {
            trajectory.push(snapshot(time, &values, &spec));
        }
    }
    let pattern = pattern_diagnostics(&values, &spec, reference_state);
    let minimum_value = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum_value = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let cfl = cfl(&spec);
    Ok(ReactionDiffusionResult {
        format: "marklab.reaction_diffusion",
        version: 1,
        simulator_id: "scalar_periodic_2d_reaction_diffusion",
        simulator_version: 1,
        dimensionality: 2,
        grid_x: spec.grid_x,
        grid_y: spec.grid_y,
        spacing_x_um: spec.spacing_x_um,
        spacing_y_um: spec.spacing_y_um,
        diffusion_um2_per_time: spec.diffusion_um2_per_time,
        reaction: spec.reaction,
        final_time: spec.final_time,
        requested_time_step: spec.time_step,
        initial_state,
        final_state: state(&spec, &values),
        trajectory,
        pattern,
        solver: ReactionDiffusionSolverDiagnostics {
            method: "strang_exact_reaction_plus_explicit_five_point_diffusion",
            boundary_condition: "periodic",
            completed_steps,
            cell_steps: u64::from(completed_steps) * cells as u64,
            maximum_cfl_number: cfl,
            cfl_limit: 0.5,
            positivity_violations,
            minimum_value,
            maximum_value,
        },
        observation_model: "identity_field_observation",
        claim_status: "experimental_reaction_diffusion_not_biological_mechanism_proof",
    })
}

fn validate(spec: &ReactionDiffusionSpec) -> Result<(usize, u32), SimulationError> {
    let cells = (spec.grid_x as usize)
        .checked_mul(spec.grid_y as usize)
        .ok_or_else(|| SimulationError::Invalid("grid size overflowed".into()))?;
    let controls_valid = (2..=256).contains(&spec.grid_x)
        && (2..=256).contains(&spec.grid_y)
        && cells <= 65_536
        && spec.initial_row_major.len() == cells
        && spec.spacing_x_um.is_finite()
        && spec.spacing_x_um > 0.0
        && spec.spacing_y_um.is_finite()
        && spec.spacing_y_um > 0.0
        && spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.reaction.validate()
        && spec.final_time.is_finite()
        && spec.final_time > 0.0
        && spec.time_step.is_finite()
        && spec.time_step > 0.0
        && spec.record_every_steps > 0
        && (1..=250_000_000).contains(&spec.maximum_cell_steps)
        && spec
            .initial_row_major
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0);
    if !controls_valid {
        return Err(SimulationError::Invalid(
            "reaction-diffusion grid, state, model, or controls are invalid".into(),
        ));
    }
    if let ReactionModel::Logistic {
        carrying_capacity, ..
    } = spec.reaction
    {
        if spec
            .initial_row_major
            .iter()
            .any(|value| *value > carrying_capacity)
        {
            return Err(SimulationError::Invalid(
                "logistic initial state exceeds carrying capacity".into(),
            ));
        }
    }
    let maximum_steps = (spec.final_time / spec.time_step).ceil();
    if !maximum_steps.is_finite() || maximum_steps > f64::from(u32::MAX) {
        return Err(SimulationError::Invalid(
            "reaction-diffusion step count is invalid".into(),
        ));
    }
    let work = maximum_steps as u64 * cells as u64;
    if work > spec.maximum_cell_steps {
        return Err(SimulationError::Invalid(format!(
            "{work} cell-steps exceed the declared resource bound"
        )));
    }
    let cfl = cfl(spec);
    if !cfl.is_finite() || cfl > 0.5 + 1e-14 {
        return Err(SimulationError::Invalid(format!(
            "reaction-diffusion CFL {cfl} exceeds 0.5"
        )));
    }
    Ok((cells, maximum_steps as u32))
}

fn cfl(spec: &ReactionDiffusionSpec) -> f64 {
    spec.diffusion_um2_per_time
        * spec.time_step
        * (spec.spacing_x_um.recip().powi(2) + spec.spacing_y_um.recip().powi(2))
}

fn diffusion_step(
    values: &mut [f64],
    spec: &ReactionDiffusionSpec,
    dt: f64,
) -> Result<(), SimulationError> {
    if spec.diffusion_um2_per_time == 0.0 || dt == 0.0 {
        return Ok(());
    }
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let cx = spec.diffusion_um2_per_time * dt / spec.spacing_x_um.powi(2);
    let cy = spec.diffusion_um2_per_time * dt / spec.spacing_y_um.powi(2);
    let previous = values.to_vec();
    for y in 0..ny {
        let below = (y + ny - 1) % ny;
        let above = (y + 1) % ny;
        for x in 0..nx {
            let left = (x + nx - 1) % nx;
            let right = (x + 1) % nx;
            let index = y * nx + x;
            values[index] = (1.0 - 2.0 * cx - 2.0 * cy) * previous[index]
                + cx * (previous[y * nx + left] + previous[y * nx + right])
                + cy * (previous[below * nx + x] + previous[above * nx + x]);
        }
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(SimulationError::Numerical(
            "diffusion step became non-finite".into(),
        ));
    }
    Ok(())
}

fn pattern_diagnostics(
    values: &[f64],
    spec: &ReactionDiffusionSpec,
    reference_state: f64,
) -> ReactionDiffusionPatternDiagnostics {
    let nx = spec.grid_x as usize;
    let ny = spec.grid_y as usize;
    let center = mean(values);
    let mut transformed = values
        .iter()
        .map(|value| Complex::new(value - center, 0.0))
        .collect::<Vec<_>>();
    let mut planner = FftPlanner::new();
    let row_fft = planner.plan_fft_forward(nx);
    for row in transformed.chunks_exact_mut(nx) {
        row_fft.process(row);
    }
    let column_fft = planner.plan_fft_forward(ny);
    let mut column = vec![Complex::new(0.0, 0.0); ny];
    for x in 0..nx {
        for y in 0..ny {
            column[y] = transformed[y * nx + x];
        }
        column_fft.process(&mut column);
        for y in 0..ny {
            transformed[y * nx + x] = column[y];
        }
    }
    let total_power = transformed
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 0)
        .map(|(_, value)| value.norm_sqr())
        .sum::<f64>();
    let dominant = transformed
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 0)
        .max_by(|left, right| left.1.norm_sqr().total_cmp(&right.1.norm_sqr()));
    let (dominant_wavelength, dominant_fx, dominant_fy, dominant_fraction) = if total_power <= 1e-24
    {
        (None, None, None, None)
    } else {
        let (index, coefficient) = dominant.expect("a nonzero Fourier mode exists");
        let x = index % nx;
        let y = index / nx;
        let fx = signed_frequency(x, nx, spec.spacing_x_um);
        let fy = signed_frequency(y, ny, spec.spacing_y_um);
        let radial = fx.hypot(fy);
        (
            Some(radial.recip()),
            Some(fx),
            Some(fy),
            Some(coefficient.norm_sqr() / total_power),
        )
    };
    let slope = spec.reaction.derivative(reference_state);
    let reaction_residual = spec.reaction.value(reference_state);
    let reference_is_equilibrium = reaction_residual.abs() <= 1e-10 * (1.0 + reference_state.abs());
    let mut unstable_wavelengths = Vec::new();
    if reference_is_equilibrium {
        for y in 0..ny {
            for x in 0..nx {
                if x == 0 && y == 0 {
                    continue;
                }
                let fx = signed_frequency(x, nx, spec.spacing_x_um);
                let fy = signed_frequency(y, ny, spec.spacing_y_um);
                let kx_index = x.min(nx - x) as f64;
                let ky_index = y.min(ny - y) as f64;
                let eigenvalue = 4.0
                    * ((std::f64::consts::PI * kx_index / nx as f64).sin().powi(2)
                        / spec.spacing_x_um.powi(2)
                        + (std::f64::consts::PI * ky_index / ny as f64).sin().powi(2)
                            / spec.spacing_y_um.powi(2));
                if slope - spec.diffusion_um2_per_time * eigenvalue > 1e-12 {
                    unstable_wavelengths.push(fx.hypot(fy).recip());
                }
            }
        }
    }
    unstable_wavelengths.sort_by(f64::total_cmp);
    ReactionDiffusionPatternDiagnostics {
        spectrum_basis: "periodic_2d_discrete_fourier",
        dominant_wavelength_um: dominant_wavelength,
        dominant_frequency_x_cycles_per_um: dominant_fx,
        dominant_frequency_y_cycles_per_um: dominant_fy,
        dominant_power_fraction: dominant_fraction,
        reference_homogeneous_state: reference_state,
        homogeneous_reaction_residual: reaction_residual,
        linearized_reaction_slope: slope,
        unstable_wavelength_min_um: unstable_wavelengths.first().copied(),
        unstable_wavelength_max_um: unstable_wavelengths.last().copied(),
        linear_instability_status: if !reference_is_equilibrium {
            "reference_not_homogeneous_equilibrium"
        } else if unstable_wavelengths.is_empty() {
            "nonzero_modes_stable"
        } else {
            "unstable_nonzero_modes"
        },
    }
}

fn signed_frequency(index: usize, length: usize, spacing: f64) -> f64 {
    let signed = if index <= length / 2 {
        index as f64
    } else {
        index as f64 - length as f64
    };
    signed / (length as f64 * spacing)
}

fn state(spec: &ReactionDiffusionSpec, values: &[f64]) -> Vec<ReactionDiffusionStateCell> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let ix = index % spec.grid_x as usize;
            let iy = index / spec.grid_x as usize;
            ReactionDiffusionStateCell {
                ix: ix as u32,
                iy: iy as u32,
                x_um: ix as f64 * spec.spacing_x_um,
                y_um: iy as f64 * spec.spacing_y_um,
                value: *value,
            }
        })
        .collect()
}

fn snapshot(time: f64, values: &[f64], spec: &ReactionDiffusionSpec) -> ReactionDiffusionSnapshot {
    let mean = mean(values);
    ReactionDiffusionSnapshot {
        time,
        mean,
        population_variance: values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64,
        minimum: values.iter().copied().fold(f64::INFINITY, f64::min),
        maximum: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        spatial_integral: values.iter().sum::<f64>() * spec.spacing_x_um * spec.spacing_y_um,
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
