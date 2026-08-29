use std::collections::BTreeSet;

use crate::{ObservationWindow2D, Pattern};

use super::{
    analysis::{canonical_zero, compensated_add, Counters},
    types::{
        InhomogeneousIntensityGridPoint, InhomogeneousIntensityPoint, InhomogeneousSpatialConfig,
        InhomogeneousSpatialError,
    },
};

#[derive(Clone, Copy)]
pub(super) struct Probe {
    x_um: f64,
    y_um: f64,
    min_x_um: f64,
    min_y_um: f64,
    max_x_um: f64,
    max_y_um: f64,
}

pub(super) struct Grid {
    pub(super) probes: Vec<Probe>,
    pub(super) spacing_um: [f64; 2],
    pub(super) cell_area_um2: f64,
}

pub(super) struct FittedIntensity {
    pub(super) grid: Grid,
    pub(super) point_values: Vec<InhomogeneousIntensityPoint>,
    pub(super) observed_intensities: Vec<f64>,
    pub(super) probe_cdf: Vec<f64>,
    pub(super) fixed_grid_total_mass: f64,
    pub(super) fixed_grid: Vec<InhomogeneousIntensityGridPoint>,
}

pub(super) struct FittedEventIntensity {
    pub(super) point_values: Vec<InhomogeneousIntensityPoint>,
    pub(super) observed_intensities: Vec<f64>,
}

pub(super) fn fit_intensity(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<FittedIntensity, InhomogeneousSpatialError> {
    let grid = build_grid(window, config)?;
    let events = fit_event_intensity(pattern, &grid, config, counters)?;
    let (probe_cdf, fixed_grid_total_mass, fixed_grid) =
        fixed_probe_cdf(pattern, &grid, config, counters)?;
    Ok(FittedIntensity {
        grid,
        point_values: events.point_values,
        observed_intensities: events.observed_intensities,
        probe_cdf,
        fixed_grid_total_mass,
        fixed_grid,
    })
}

pub(super) fn fit_event_intensity(
    pattern: &Pattern,
    grid: &Grid,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<FittedEventIntensity, InhomogeneousSpatialError> {
    let mut point_values = Vec::new();
    let mut observed_intensities = Vec::new();
    point_values
        .try_reserve_exact(pattern.len())
        .and_then(|()| observed_intensities.try_reserve_exact(pattern.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let finite_scale = pattern.len() as f64 / (pattern.len() - 1) as f64;
    for row in 0..pattern.len() {
        let location = (pattern.x_um[row], pattern.y_um[row]);
        let correction = boundary_mass(location, grid, config, counters)?;
        let raw = kernel_sum(
            location,
            &pattern.x_um,
            &pattern.y_um,
            Some(row),
            config,
            counters,
        )?;
        let intensity = finite_scale * raw / correction;
        validate_intensity(row, intensity, config.minimum_intensity_per_um2)?;
        point_values.push(InhomogeneousIntensityPoint {
            row,
            intensity_per_um2: canonical_zero(intensity),
            boundary_mass: correction,
            training_point_count: pattern.len() - 1,
        });
        observed_intensities.push(intensity);
    }
    Ok(FittedEventIntensity {
        point_values,
        observed_intensities,
    })
}

pub(super) fn evaluate_fixed_intensities(
    x: &[f64],
    y: &[f64],
    pattern: &Pattern,
    fitted: &FittedIntensity,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<Vec<f64>, InhomogeneousSpatialError> {
    let mut intensities = Vec::new();
    intensities
        .try_reserve_exact(x.len())
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    for row in 0..x.len() {
        let location = (x[row], y[row]);
        let correction = boundary_mass(location, &fitted.grid, config, counters)?;
        let intensity = kernel_sum(
            location,
            &pattern.x_um,
            &pattern.y_um,
            None,
            config,
            counters,
        )? / correction;
        validate_intensity(row, intensity, config.minimum_intensity_per_um2)?;
        intensities.push(intensity);
    }
    Ok(intensities)
}

pub(super) fn build_grid(
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
) -> Result<Grid, InhomogeneousSpatialError> {
    let [min_x, min_y, max_x, max_y] = window.bounds_um();
    let spacing = [
        (max_x - min_x) / config.integration_grid[0] as f64,
        (max_y - min_y) / config.integration_grid[1] as f64,
    ];
    if spacing
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "window bounds do not define a positive grid".into(),
        ));
    }
    let capacity = config.integration_grid[0]
        .checked_mul(config.integration_grid[1])
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let mut probes = Vec::new();
    probes
        .try_reserve_exact(capacity)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    for y in 0..config.integration_grid[1] {
        for x in 0..config.integration_grid[0] {
            let cell_min_x = min_x + x as f64 * spacing[0];
            let cell_min_y = min_y + y as f64 * spacing[1];
            let center_x = cell_min_x + 0.5 * spacing[0];
            let center_y = cell_min_y + 0.5 * spacing[1];
            if window.contains(center_x, center_y) {
                probes.push(Probe {
                    x_um: center_x,
                    y_um: center_y,
                    min_x_um: cell_min_x,
                    min_y_um: cell_min_y,
                    max_x_um: cell_min_x + spacing[0],
                    max_y_um: cell_min_y + spacing[1],
                });
            }
        }
    }
    if probes.is_empty() {
        return Err(InhomogeneousSpatialError::Dependency(
            "integration grid retains no in-window probes".into(),
        ));
    }
    Ok(Grid {
        probes,
        spacing_um: spacing,
        cell_area_um2: spacing[0] * spacing[1],
    })
}

pub(super) fn boundary_mass(
    location: (f64, f64),
    grid: &Grid,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<f64, InhomogeneousSpatialError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for probe in &grid.probes {
        counters.charge_intensity(config)?;
        compensated_add(
            &mut sum,
            &mut correction,
            gaussian_kernel(location, (probe.x_um, probe.y_um), config.bandwidth_um)
                * grid.cell_area_um2,
        );
    }
    let value = sum + correction;
    if !value.is_finite() || value <= 0.0 {
        return Err(InhomogeneousSpatialError::Dependency(
            "kernel boundary mass is nonpositive or non-finite".into(),
        ));
    }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn kernel_sum(
    location: (f64, f64),
    x: &[f64],
    y: &[f64],
    skip: Option<usize>,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<f64, InhomogeneousSpatialError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for row in 0..x.len() {
        if skip == Some(row) {
            continue;
        }
        counters.charge_intensity(config)?;
        compensated_add(
            &mut sum,
            &mut correction,
            gaussian_kernel(location, (x[row], y[row]), config.bandwidth_um),
        );
    }
    let value = sum + correction;
    if !value.is_finite() || value <= 0.0 {
        return Err(InhomogeneousSpatialError::Dependency(
            "kernel intensity sum is nonpositive or non-finite".into(),
        ));
    }
    Ok(value)
}

fn gaussian_kernel(left: (f64, f64), right: (f64, f64), bandwidth_um: f64) -> f64 {
    let dx = left.0 - right.0;
    let dy = left.1 - right.1;
    let squared = dx * dx + dy * dy;
    (-squared / (2.0 * bandwidth_um * bandwidth_um)).exp()
        / (2.0 * std::f64::consts::PI * bandwidth_um * bandwidth_um)
}

pub(super) fn fixed_probe_cdf(
    pattern: &Pattern,
    grid: &Grid,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<(Vec<f64>, f64, Vec<InhomogeneousIntensityGridPoint>), InhomogeneousSpatialError> {
    let mut cdf = Vec::new();
    let mut fixed_grid = Vec::new();
    cdf.try_reserve_exact(grid.probes.len())
        .and_then(|()| fixed_grid.try_reserve_exact(grid.probes.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut total = 0.0;
    let mut correction = 0.0;
    for (probe_index, probe) in grid.probes.iter().enumerate() {
        let location = (probe.x_um, probe.y_um);
        let intensity = kernel_sum(
            location,
            &pattern.x_um,
            &pattern.y_um,
            None,
            config,
            counters,
        )? / boundary_mass(location, grid, config, counters)?;
        let mass = intensity * grid.cell_area_um2;
        compensated_add(&mut total, &mut correction, mass);
        cdf.push(total + correction);
        fixed_grid.push(InhomogeneousIntensityGridPoint {
            probe_index,
            x_um: probe.x_um,
            y_um: probe.y_um,
            intensity_per_um2: intensity,
            cell_mass: mass,
        });
    }
    let total = total + correction;
    if !total.is_finite() || total <= 0.0 {
        return Err(InhomogeneousSpatialError::Dependency(
            "fixed intensity grid has invalid total mass".into(),
        ));
    }
    Ok((cdf, total, fixed_grid))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sample_fixed_grid_pattern(
    window: &ObservationWindow2D,
    point_count: usize,
    grid: &Grid,
    cdf: &[f64],
    total: f64,
    seed: u64,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<(Vec<f64>, Vec<f64>), InhomogeneousSpatialError> {
    let mut x = Vec::new();
    let mut y = Vec::new();
    x.try_reserve_exact(point_count)
        .and_then(|()| y.try_reserve_exact(point_count))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut unique = BTreeSet::new();
    let mut state = seed;
    while x.len() < point_count {
        counters.charge_null_draw(config)?;
        state = crate::common::seeds::splitmix64(state);
        let mass = unit_interval(state) * total;
        let cell = cdf
            .partition_point(|value| *value <= mass)
            .min(cdf.len() - 1);
        let probe = grid.probes[cell];
        state = crate::common::seeds::splitmix64(state);
        let x_um = probe.min_x_um + unit_interval(state) * (probe.max_x_um - probe.min_x_um);
        state = crate::common::seeds::splitmix64(state);
        let y_um = probe.min_y_um + unit_interval(state) * (probe.max_y_um - probe.min_y_um);
        let key = (canonical_bits(x_um), canonical_bits(y_um));
        if window.contains(x_um, y_um) && unique.insert(key) {
            x.push(x_um);
            y.push(y_um);
        }
    }
    Ok((x, y))
}

pub(super) fn validate_intensity(
    row: usize,
    value: f64,
    minimum: f64,
) -> Result<(), InhomogeneousSpatialError> {
    if !value.is_finite() || value < minimum {
        return Err(InhomogeneousSpatialError::NearZeroIntensity {
            row,
            observed: value,
            minimum,
        });
    }
    Ok(())
}

pub(super) fn extrema(values: &[f64]) -> Result<(f64, f64), InhomogeneousSpatialError> {
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !minimum.is_finite() || !maximum.is_finite() {
        return Err(InhomogeneousSpatialError::Dependency(
            "intensity extrema are non-finite".into(),
        ));
    }
    Ok((minimum, maximum))
}

pub(super) fn probe_storage_bytes(probe_count: usize) -> Option<usize> {
    probe_count.checked_mul(
        std::mem::size_of::<Probe>()
            + std::mem::size_of::<f64>()
            + std::mem::size_of::<InhomogeneousIntensityGridPoint>(),
    )
}

fn unit_interval(value: u64) -> f64 {
    (value >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}
