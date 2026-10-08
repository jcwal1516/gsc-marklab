use std::collections::BTreeSet;

use crate::common::{finite::canonical_zero, summation::kahan_add};

use crate::{geom::spatial_index::SpatialIndex2D, ObservationWindow2D, Pattern};

use super::{
    analysis::{dependency, Counters},
    types::{
        InhomogeneousIntensityGridPoint, InhomogeneousIntensityPoint, InhomogeneousSpatialConfig,
        InhomogeneousSpatialError,
    },
};

/// Events farther than this many bandwidths are skipped only when their
/// combined kernel mass is provably below one unit roundoff of the sum.
const KERNEL_CUTOFF_BANDWIDTHS: f64 = 12.0;

#[derive(Clone, Copy)]
pub(super) struct Probe {
    x_um: f64,
    y_um: f64,
    min_x_um: f64,
    min_y_um: f64,
    max_x_um: f64,
    max_y_um: f64,
    column: usize,
}

/// Row-major cell-center probes retained from a regular lattice.
pub(super) struct Grid {
    pub(super) probes: Vec<Probe>,
    pub(super) spacing_um: [f64; 2],
    pub(super) cell_area_um2: f64,
    column_centers_um: Vec<f64>,
    row_centers_um: Vec<f64>,
    /// `probes[row_starts[r]..row_starts[r + 1]]` lie in lattice row `r`.
    row_starts: Vec<usize>,
}

pub(super) struct FittedIntensity {
    pub(super) grid: Grid,
    pub(super) point_values: Vec<InhomogeneousIntensityPoint>,
    pub(super) observed_intensities: Vec<f64>,
    pub(super) probe_cdf: Vec<f64>,
    pub(super) fixed_grid_total_mass: f64,
    pub(super) fixed_grid: Vec<InhomogeneousIntensityGridPoint>,
    pub(super) cross_fit: Option<CrossFitPlan>,
    /// Location index of the fitted events, reused by every null evaluation.
    event_index: SpatialIndex2D,
}

/// Per-location buffers reused across one batch of intensity evaluations.
#[derive(Default)]
struct KernelScratch {
    neighbors: Vec<usize>,
    column_factors: Vec<f64>,
    fold_sums: Vec<(f64, f64)>,
    fold_near: Vec<usize>,
}

#[derive(Clone)]
pub(super) struct CrossFitPlan {
    pub(super) assignments: Vec<usize>,
    pub(super) fold_counts: Vec<usize>,
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
    let cross_fit = cross_fit_plan(pattern, config)?;
    let event_index = event_index(pattern)?;
    let mut scratch = KernelScratch::default();
    let events = fit_events_on_index(pattern, &grid, &event_index, config, counters, &mut scratch)?;
    let (probe_cdf, fixed_grid_total_mass, fixed_grid) =
        fixed_probe_cdf(pattern, &grid, &event_index, config, counters, &mut scratch)?;
    Ok(FittedIntensity {
        grid,
        point_values: events.point_values,
        observed_intensities: events.observed_intensities,
        probe_cdf,
        fixed_grid_total_mass,
        fixed_grid,
        cross_fit,
        event_index,
    })
}

/// Leave-one-out or cross-fitted intensity at each event of `pattern`.
pub(super) fn fit_event_intensity(
    pattern: &Pattern,
    grid: &Grid,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
) -> Result<FittedEventIntensity, InhomogeneousSpatialError> {
    let index = event_index(pattern)?;
    let mut scratch = KernelScratch::default();
    fit_events_on_index(pattern, grid, &index, config, counters, &mut scratch)
}

fn event_index(pattern: &Pattern) -> Result<SpatialIndex2D, InhomogeneousSpatialError> {
    SpatialIndex2D::new(&pattern.x_um, &pattern.y_um).map_err(dependency)
}

fn fit_events_on_index(
    pattern: &Pattern,
    grid: &Grid,
    index: &SpatialIndex2D,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
    scratch: &mut KernelScratch,
) -> Result<FittedEventIntensity, InhomogeneousSpatialError> {
    let cross_fit = cross_fit_plan(pattern, config)?;
    let events = KernelEvents::new(&pattern.x_um, &pattern.y_um, index);
    let mut point_values = Vec::new();
    let mut observed_intensities = Vec::new();
    point_values
        .try_reserve_exact(pattern.len())
        .and_then(|()| observed_intensities.try_reserve_exact(pattern.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    // An event's training set excludes itself, or its whole held-out fold.
    let same_fold = |left: usize, right: usize| {
        cross_fit
            .as_ref()
            .is_some_and(|plan| plan.assignments[left] == plan.assignments[right])
    };
    let pairwise = events.pairwise_sums(same_fold, config, counters)?;
    for row in 0..pattern.len() {
        let location = (pattern.x_um[row], pattern.y_um[row]);
        let correction = boundary_mass(location, grid, config, counters, scratch)?;
        let training_point_count = cross_fit.as_ref().map_or(pattern.len() - 1, |plan| {
            pattern.len() - plan.fold_counts[plan.assignments[row]]
        });
        let raw = match &pairwise {
            Some(sums) => positive(
                sums[row],
                "kernel intensity sum is nonpositive or non-finite",
            )?,
            None => events.sum(
                location,
                |event| event != row && !same_fold(event, row),
                training_point_count,
                config,
                counters,
                scratch,
            )?,
        };
        let scale = pattern.len() as f64 / training_point_count as f64;
        let intensity = scale * raw / correction;
        validate_intensity(row, intensity, config.minimum_intensity_per_um2)?;
        point_values.push(InhomogeneousIntensityPoint {
            row,
            intensity_per_um2: canonical_zero(intensity),
            boundary_mass: correction,
            training_point_count,
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
    let events = KernelEvents::new(&pattern.x_um, &pattern.y_um, &fitted.event_index);
    let mut scratch = KernelScratch::default();
    let mut intensities = Vec::new();
    intensities
        .try_reserve_exact(x.len())
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    for row in 0..x.len() {
        let location = (x[row], y[row]);
        let correction = boundary_mass(location, &fitted.grid, config, counters, &mut scratch)?;
        let raw = events.fixed_sum(
            location,
            fitted.cross_fit.as_ref(),
            config,
            counters,
            &mut scratch,
        )?;
        let intensity = raw / correction;
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
    let mut column_centers_um = Vec::new();
    let mut row_centers_um = Vec::new();
    let mut row_starts = Vec::new();
    probes
        .try_reserve_exact(capacity)
        .and_then(|()| column_centers_um.try_reserve_exact(config.integration_grid[0]))
        .and_then(|()| row_centers_um.try_reserve_exact(config.integration_grid[1]))
        .and_then(|()| row_starts.try_reserve_exact(config.integration_grid[1] + 1))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    column_centers_um.extend(
        (0..config.integration_grid[0]).map(|x| (min_x + x as f64 * spacing[0]) + 0.5 * spacing[0]),
    );
    for y in 0..config.integration_grid[1] {
        let cell_min_y = min_y + y as f64 * spacing[1];
        let center_y = cell_min_y + 0.5 * spacing[1];
        row_centers_um.push(center_y);
        row_starts.push(probes.len());
        for (x, &center_x) in column_centers_um.iter().enumerate() {
            let cell_min_x = min_x + x as f64 * spacing[0];
            if window.contains(center_x, center_y) {
                probes.push(Probe {
                    x_um: center_x,
                    y_um: center_y,
                    min_x_um: cell_min_x,
                    min_y_um: cell_min_y,
                    max_x_um: cell_min_x + spacing[0],
                    max_y_um: cell_min_y + spacing[1],
                    column: x,
                });
            }
        }
    }
    row_starts.push(probes.len());
    if probes.is_empty() {
        return Err(InhomogeneousSpatialError::Dependency(
            "integration grid retains no in-window probes".into(),
        ));
    }
    Ok(Grid {
        probes,
        spacing_um: spacing,
        cell_area_um2: spacing[0] * spacing[1],
        column_centers_um,
        row_centers_um,
        row_starts,
    })
}

/// Cell-center quadrature of the Gaussian kernel mass inside the window.
///
/// The isotropic Gaussian factors into column and row terms, and every lattice
/// probe shares its column and row centers. One exponential per column and per
/// occupied row therefore replaces one per retained probe.
fn boundary_mass(
    location: (f64, f64),
    grid: &Grid,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
    scratch: &mut KernelScratch,
) -> Result<f64, InhomogeneousSpatialError> {
    let scale = 2.0 * config.bandwidth_um * config.bandwidth_um;
    scratch.column_factors.clear();
    for center in &grid.column_centers_um {
        counters.charge_intensity(config)?;
        let offset = location.0 - center;
        scratch
            .column_factors
            .push((-(offset * offset) / scale).exp());
    }
    let mut sum = 0.0;
    let mut correction = 0.0;
    for (row, center) in grid.row_centers_um.iter().enumerate() {
        let probes = &grid.probes[grid.row_starts[row]..grid.row_starts[row + 1]];
        if probes.is_empty() {
            continue;
        }
        counters.charge_intensity(config)?;
        let offset = location.1 - center;
        let row_factor = (-(offset * offset) / scale).exp();
        let mut row_sum = 0.0;
        let mut row_correction = 0.0;
        for probe in probes {
            kahan_add(
                &mut row_sum,
                &mut row_correction,
                scratch.column_factors[probe.column],
            );
        }
        kahan_add(
            &mut sum,
            &mut correction,
            row_factor * (row_sum + row_correction),
        );
    }
    positive(
        (sum + correction) * grid.cell_area_um2 / (std::f64::consts::PI * scale),
        "kernel boundary mass is nonpositive or non-finite",
    )
}

/// Gaussian kernel sums over one fixed event set and its location index.
struct KernelEvents<'a> {
    x: &'a [f64],
    y: &'a [f64],
    index: &'a SpatialIndex2D,
    /// Event bounding box as `[min_x, min_y, max_x, max_y]`.
    bounds: [f64; 4],
}

impl<'a> KernelEvents<'a> {
    fn new(x: &'a [f64], y: &'a [f64], index: &'a SpatialIndex2D) -> Self {
        let bounds = x.iter().zip(y).fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |[min_x, min_y, max_x, max_y], (x, y)| {
                [min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y)]
            },
        );
        Self {
            x,
            y,
            index,
            bounds,
        }
    }

    /// Whether every event lies within `radius` of `location`.
    fn within(&self, location: (f64, f64), radius: f64) -> bool {
        let [min_x, min_y, max_x, max_y] = self.bounds;
        let dx = (location.0 - min_x).abs().max((max_x - location.0).abs());
        let dy = (location.1 - min_y).abs().max((max_y - location.1).abs());
        dx.hypot(dy) <= radius
    }

    /// Leave-one-out or held-out-fold sums at every event from one kernel
    /// evaluation per pair, when every pair lies within the cutoff.
    ///
    /// Each event still accumulates its terms in row order, so the sums equal
    /// the per-event row-ordered sums exactly. Returns `None` when some pair
    /// may lie beyond the cutoff, where per-event truncated sums are cheaper.
    fn pairwise_sums(
        &self,
        excluded: impl Fn(usize, usize) -> bool,
        config: &InhomogeneousSpatialConfig,
        counters: &mut Counters,
    ) -> Result<Option<Vec<f64>>, InhomogeneousSpatialError> {
        let [min_x, min_y, max_x, max_y] = self.bounds;
        if (max_x - min_x).hypot(max_y - min_y) > KERNEL_CUTOFF_BANDWIDTHS * config.bandwidth_um {
            return Ok(None);
        }
        let mut sums = Vec::new();
        sums.try_reserve_exact(self.x.len())
            .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
        sums.resize(self.x.len(), (0.0, 0.0));
        for left in 0..self.x.len() {
            for right in left + 1..self.x.len() {
                if excluded(left, right) {
                    continue;
                }
                counters.charge_intensity(config)?;
                let value = self.kernel((self.x[left], self.y[left]), right, config);
                let (sum, correction) = &mut sums[left];
                kahan_add(sum, correction, value);
                let (sum, correction) = &mut sums[right];
                kahan_add(sum, correction, value);
            }
        }
        Ok(Some(
            sums.into_iter()
                .map(|(sum, correction)| sum + correction)
                .collect(),
        ))
    }

    /// Sum kernels from the events selected by `eligible` at `location`.
    ///
    /// Eligible events within the cutoff radius are summed; when the cutoff
    /// covers every event this is the plain row-ordered sum. Each
    /// skipped eligible event lies beyond the cutoff, so it contributes at most
    /// the kernel peak times `exp(-cutoff_bandwidths^2 / 2)`. Skipping is kept
    /// only when that bound over all skipped events is within one unit
    /// roundoff of the evaluated sum; otherwise all eligible events are summed.
    fn sum(
        &self,
        location: (f64, f64),
        eligible: impl Fn(usize) -> bool,
        eligible_count: usize,
        config: &InhomogeneousSpatialConfig,
        counters: &mut Counters,
        scratch: &mut KernelScratch,
    ) -> Result<f64, InhomogeneousSpatialError> {
        self.collect_near(location, config, scratch)?;
        let mut sum = 0.0;
        let mut correction = 0.0;
        let mut evaluated = 0_usize;
        for &event in &scratch.neighbors {
            if eligible(event) {
                counters.charge_intensity(config)?;
                evaluated += 1;
                kahan_add(
                    &mut sum,
                    &mut correction,
                    self.kernel(location, event, config),
                );
            }
        }
        let mut value = sum + correction;
        if !negligible_tail(eligible_count - evaluated, value, config) {
            value = self.full_sum(location, &eligible, config, counters)?;
        }
        positive(value, "kernel intensity sum is nonpositive or non-finite")
    }

    /// Fixed-intensity numerator at an arbitrary location.
    ///
    /// Cross-fitted sums average every fold's training-set sum. One kernel
    /// evaluation per nearby event feeds all folds that train on that event.
    fn fixed_sum(
        &self,
        location: (f64, f64),
        cross_fit: Option<&CrossFitPlan>,
        config: &InhomogeneousSpatialConfig,
        counters: &mut Counters,
        scratch: &mut KernelScratch,
    ) -> Result<f64, InhomogeneousSpatialError> {
        let Some(plan) = cross_fit else {
            return self.sum(location, |_| true, self.x.len(), config, counters, scratch);
        };
        self.collect_near(location, config, scratch)?;
        let folds = plan.fold_counts.len();
        scratch.fold_sums.clear();
        scratch.fold_sums.resize(folds, (0.0, 0.0));
        scratch.fold_near.clear();
        scratch.fold_near.resize(folds, 0);
        for &event in &scratch.neighbors {
            counters.charge_intensity(config)?;
            let value = self.kernel(location, event, config);
            let heldout = plan.assignments[event];
            scratch.fold_near[heldout] += 1;
            for (fold, (sum, correction)) in scratch.fold_sums.iter_mut().enumerate() {
                if fold != heldout {
                    kahan_add(sum, correction, value);
                }
            }
        }
        let count = self.x.len();
        let mut sum = 0.0;
        let mut correction = 0.0;
        for fold in 0..folds {
            let training = count - plan.fold_counts[fold];
            let evaluated = scratch.neighbors.len() - scratch.fold_near[fold];
            let (fold_sum, fold_correction) = scratch.fold_sums[fold];
            let mut raw = fold_sum + fold_correction;
            if !negligible_tail(training - evaluated, raw, config) {
                raw = self.full_sum(
                    location,
                    |event| plan.assignments[event] != fold,
                    config,
                    counters,
                )?;
            }
            let raw = positive(
                raw,
                "cross-fitted kernel intensity sum is nonpositive or non-finite",
            )?;
            let heldout_weight = plan.fold_counts[fold] as f64 / count as f64;
            kahan_add(
                &mut sum,
                &mut correction,
                heldout_weight * count as f64 / training as f64 * raw,
            );
        }
        positive(
            sum + correction,
            "cross-fitted fixed intensity is nonpositive or non-finite",
        )
    }

    /// Indices of events within the cutoff radius of `location`: all events
    /// in row order when the cutoff covers them, otherwise index visit order.
    fn collect_near(
        &self,
        location: (f64, f64),
        config: &InhomogeneousSpatialConfig,
        scratch: &mut KernelScratch,
    ) -> Result<(), InhomogeneousSpatialError> {
        let cutoff = KERNEL_CUTOFF_BANDWIDTHS * config.bandwidth_um;
        let neighbors = &mut scratch.neighbors;
        neighbors.clear();
        if self.within(location, cutoff) {
            neighbors.extend(0..self.x.len());
            return Ok(());
        }
        self.index
            .visit_points_within_radius(location.0, location.1, cutoff, |neighbor| {
                neighbors.push(neighbor.index)
            })
            .map_err(dependency)
    }

    fn full_sum(
        &self,
        location: (f64, f64),
        eligible: impl Fn(usize) -> bool,
        config: &InhomogeneousSpatialConfig,
        counters: &mut Counters,
    ) -> Result<f64, InhomogeneousSpatialError> {
        let mut sum = 0.0;
        let mut correction = 0.0;
        for event in (0..self.x.len()).filter(|event| eligible(*event)) {
            counters.charge_intensity(config)?;
            kahan_add(
                &mut sum,
                &mut correction,
                self.kernel(location, event, config),
            );
        }
        Ok(sum + correction)
    }

    fn kernel(
        &self,
        location: (f64, f64),
        event: usize,
        config: &InhomogeneousSpatialConfig,
    ) -> f64 {
        gaussian_kernel(
            location,
            (self.x[event], self.y[event]),
            config.bandwidth_um,
        )
    }
}

/// Whether `skipped` events beyond the cutoff cannot change `evaluated` by
/// more than one unit roundoff.
fn negligible_tail(skipped: usize, evaluated: f64, config: &InhomogeneousSpatialConfig) -> bool {
    if skipped == 0 {
        return true;
    }
    let peak = 1.0 / (2.0 * std::f64::consts::PI * config.bandwidth_um * config.bandwidth_um);
    let bound =
        skipped as f64 * peak * (-0.5 * KERNEL_CUTOFF_BANDWIDTHS * KERNEL_CUTOFF_BANDWIDTHS).exp();
    bound <= 0.5 * f64::EPSILON * evaluated
}

fn positive(value: f64, reason: &str) -> Result<f64, InhomogeneousSpatialError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(InhomogeneousSpatialError::Dependency(reason.into()))
    }
}

pub(super) fn cross_fit_plan(
    pattern: &Pattern,
    config: &InhomogeneousSpatialConfig,
) -> Result<Option<CrossFitPlan>, InhomogeneousSpatialError> {
    let Some(folds) = config.cross_fit_folds else {
        return Ok(None);
    };
    let ids = pattern.cell_ids.as_deref().ok_or_else(|| {
        InhomogeneousSpatialError::Dependency(
            "cross-fitted intensity requires complete canonical Cell IDs".into(),
        )
    })?;
    if ids.len() != pattern.len() {
        return Err(InhomogeneousSpatialError::Dependency(
            "cross-fitted Cell IDs are not row aligned".into(),
        ));
    }
    let mut unique = BTreeSet::new();
    if ids
        .iter()
        .any(|id| id.trim().is_empty() || id.trim() != id || !unique.insert(id.as_str()))
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "cross-fitted Cell IDs must be exact nonempty and unique".into(),
        ));
    }
    let mut order = (0..ids.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| ids[*left].cmp(&ids[*right]));
    let mut assignments = vec![0usize; ids.len()];
    let mut fold_counts = vec![0usize; folds];
    for (rank, row) in order.into_iter().enumerate() {
        let fold = rank % folds;
        assignments[row] = fold;
        fold_counts[fold] += 1;
    }
    if fold_counts.contains(&0)
        || fold_counts
            .iter()
            .any(|count| pattern.len().saturating_sub(*count) < 2)
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "cross-fitted intensity requires at least two training points in every fold".into(),
        ));
    }
    Ok(Some(CrossFitPlan {
        assignments,
        fold_counts,
    }))
}

fn gaussian_kernel(left: (f64, f64), right: (f64, f64), bandwidth_um: f64) -> f64 {
    let dx = left.0 - right.0;
    let dy = left.1 - right.1;
    let squared = dx * dx + dy * dy;
    (-squared / (2.0 * bandwidth_um * bandwidth_um)).exp()
        / (2.0 * std::f64::consts::PI * bandwidth_um * bandwidth_um)
}

fn fixed_probe_cdf(
    pattern: &Pattern,
    grid: &Grid,
    index: &SpatialIndex2D,
    config: &InhomogeneousSpatialConfig,
    counters: &mut Counters,
    scratch: &mut KernelScratch,
) -> Result<(Vec<f64>, f64, Vec<InhomogeneousIntensityGridPoint>), InhomogeneousSpatialError> {
    let cross_fit = cross_fit_plan(pattern, config)?;
    let events = KernelEvents::new(&pattern.x_um, &pattern.y_um, index);
    let mut cdf = Vec::new();
    let mut fixed_grid = Vec::new();
    cdf.try_reserve_exact(grid.probes.len())
        .and_then(|()| fixed_grid.try_reserve_exact(grid.probes.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut total = 0.0;
    let mut correction = 0.0;
    for (probe_index, probe) in grid.probes.iter().enumerate() {
        let location = (probe.x_um, probe.y_um);
        let raw = events.fixed_sum(location, cross_fit.as_ref(), config, counters, scratch)?;
        let intensity = raw / boundary_mass(location, grid, config, counters, scratch)?;
        let mass = intensity * grid.cell_area_um2;
        kahan_add(&mut total, &mut correction, mass);
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
