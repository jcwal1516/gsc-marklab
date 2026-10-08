use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    validate_spatial_weights, DiagonalPolicy, NormalizationPolicy, SpatialEdge,
    SpatialWeightsPolicy, SymmetryPolicy,
};

const Z_975: f64 = 1.959_963_984_540_054;
const FORMAT: &str = "marklab.sparse_car_fit";
const CLAIM: &str = "experimental_conditional_gaussian_field";
const INFERENCE: &str = "independent_gaussian_precision_perturbation_with_verified_sparse_solves";

/// One declared region; `None` is an unobserved outcome, never an imputed input.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseCarObservation {
    pub region_id: String,
    pub prior_mean: f64,
    pub value: Option<f64>,
    pub noise_sd: f64,
}

/// Conditional Gaussian CAR fit. Hyperparameters and measurement errors are fixed inputs.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SparseCarSpec {
    pub graph_definition: String,
    pub coordinate_frame: String,
    pub observations: Vec<SparseCarObservation>,
    pub edges: Vec<SpatialEdge>,
    pub rho: f64,
    pub tau: f64,
    pub draws: usize,
    pub seed: u64,
    pub solve_tolerance: f64,
    pub maximum_iterations: usize,
    pub maximum_work: u64,
    pub memory_budget_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SparseCarRegionSummary {
    pub region_id: String,
    pub observed: bool,
    pub posterior_mean: f64,
    pub posterior_sd: f64,
    pub lower_95: f64,
    pub upper_95: f64,
    pub predictive_sd: f64,
    pub sd_mcse: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SparseCarPredictiveCheck {
    pub observed: f64,
    pub replicate_mean: f64,
    pub replicate_sd: f64,
    pub upper_tail_probability: f64,
    pub probability_mcse: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SparseCarPredictive {
    pub residual_energy: SparseCarPredictiveCheck,
    pub edge_roughness: Option<SparseCarPredictiveCheck>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SparseCarDiagnostics {
    pub inference: String,
    pub mcmc_diagnostics: String,
    pub independent_draws: usize,
    pub linear_solves: usize,
    pub maximum_iterations_used: usize,
    pub maximum_relative_residual: f64,
    pub maximum_relative_solution_error_bound: f64,
    pub work_used: u64,
    pub estimated_peak_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SparseCarResult {
    pub format: String,
    pub version: u32,
    pub fit_state: String,
    pub graph_definition: String,
    pub coordinate_frame: String,
    pub statistical_unit: String,
    pub rho: f64,
    pub tau: f64,
    pub seed: u64,
    pub components: usize,
    pub components_without_observations: usize,
    pub undirected_edges: usize,
    pub regions: Vec<SparseCarRegionSummary>,
    pub prior_predictive: SparseCarPredictive,
    pub posterior_predictive: SparseCarPredictive,
    pub diagnostics: SparseCarDiagnostics,
    pub claim_status: String,
}

#[derive(Debug, Error)]
pub enum SparseCarError {
    #[error("invalid sparse CAR input: {0}")]
    InvalidInput(String),
    #[error("sparse CAR numerical failure: {0}")]
    Numerical(String),
    #[error("sparse CAR resource limit: {0}")]
    ResourceLimit(String),
}

impl SparseCarSpec {
    /// Validate model and retained-memory admission without fitting or creating durable state.
    pub fn validate(&self) -> Result<(), SparseCarError> {
        Prepared::new(self).map(|_| ())
    }
}

impl SparseCarResult {
    /// Validate typed finite replay against the exact cache-bound source specification.
    pub fn validate_for(&self, spec: &SparseCarSpec) -> Result<(), SparseCarError> {
        self.validate_prepared(spec, &Prepared::new(spec)?)
    }

    fn validate_prepared(
        &self,
        spec: &SparseCarSpec,
        prepared: &Prepared<'_>,
    ) -> Result<(), SparseCarError> {
        let d = &self.diagnostics;
        let metadata_valid = self.format == FORMAT
            && self.version == 1
            && self.fit_state == "complete"
            && self.claim_status == CLAIM
            && self.graph_definition == spec.graph_definition
            && self.coordinate_frame == spec.coordinate_frame
            && self.statistical_unit == "region_conditional_on_declared_graph"
            && self.rho == spec.rho
            && self.tau == spec.tau
            && self.seed == spec.seed
            && self.components == prepared.components
            && self.components_without_observations == prepared.components_without_observations
            && self.undirected_edges == prepared.edges.len()
            && self.regions.len() == prepared.rows.len()
            && d.inference == INFERENCE
            && d.mcmc_diagnostics == "not_applicable_independent_draws"
            && d.independent_draws == spec.draws
            && d.linear_solves == 2 * spec.draws + 1
            && d.maximum_iterations_used <= spec.maximum_iterations
            && d.work_used > 0
            && d.work_used <= spec.maximum_work
            && d.estimated_peak_bytes == prepared.estimated_peak_bytes
            && d.maximum_relative_residual.is_finite()
            && d.maximum_relative_residual >= 0.0
            && d.maximum_relative_solution_error_bound.is_finite()
            && (0.0..=spec.solve_tolerance).contains(&d.maximum_relative_solution_error_bound);
        let rows_valid = self
            .regions
            .iter()
            .zip(&prepared.rows)
            .all(|(summary, row)| {
                summary.region_id == row.region_id
                    && summary.observed == row.value.is_some()
                    && [
                        summary.posterior_mean,
                        summary.posterior_sd,
                        summary.lower_95,
                        summary.upper_95,
                        summary.predictive_sd,
                        summary.sd_mcse,
                    ]
                    .iter()
                    .all(|x| x.is_finite())
                    && summary.posterior_sd > 0.0
                    && summary.predictive_sd >= summary.posterior_sd
                    && summary.sd_mcse > 0.0
                    && close(
                        summary.lower_95,
                        summary.posterior_mean - Z_975 * summary.posterior_sd,
                    )
                    && close(
                        summary.upper_95,
                        summary.posterior_mean + Z_975 * summary.posterior_sd,
                    )
                    && close(
                        summary.predictive_sd,
                        summary.posterior_sd.hypot(row.noise_sd),
                    )
                    && close(
                        summary.sd_mcse,
                        summary.posterior_sd / (2.0 * (spec.draws - 1) as f64).sqrt(),
                    )
            });
        let observed = prepared.discrepancies(&prepared.observed_residual);
        let predictive_valid = [&self.prior_predictive, &self.posterior_predictive]
            .iter()
            .all(|p| {
                p.residual_energy.valid(observed.0, spec.draws)
                    && match (&p.edge_roughness, observed.1) {
                        (Some(check), Some(value)) => check.valid(value, spec.draws),
                        (None, None) => true,
                        _ => false,
                    }
            });
        if !metadata_valid || !rows_valid || !predictive_valid {
            return Err(SparseCarError::Numerical(
                "result violates finite conditional-fit contract".into(),
            ));
        }
        Ok(())
    }
}

/// Fit the proper CAR Gaussian posterior with bounded sparse solves and independent draws.
pub fn fit_sparse_car(spec: &SparseCarSpec) -> Result<SparseCarResult, SparseCarError> {
    let prepared = Prepared::new(spec)?;
    let n = prepared.rows.len();
    let mut diagnostics = SparseCarDiagnostics {
        inference: INFERENCE.into(),
        mcmc_diagnostics: "not_applicable_independent_draws".into(),
        independent_draws: spec.draws,
        linear_solves: 0,
        maximum_iterations_used: 0,
        maximum_relative_residual: 0.0,
        maximum_relative_solution_error_bound: 0.0,
        work_used: 0,
        estimated_peak_bytes: prepared.estimated_peak_bytes,
    };
    let right: Vec<_> = prepared
        .observed_residual
        .iter()
        .zip(&prepared.observation_precision)
        .map(|(y, w)| y * w)
        .collect();
    let mean = solve(&prepared, &right, true, spec, &mut diagnostics)?;
    let mut posterior_moments = vec![Moments::default(); n];
    let observed = prepared.discrepancies(&prepared.observed_residual);
    let mut prior_ppc = PredictiveAccumulator::new(observed);
    let mut posterior_ppc = PredictiveAccumulator::new(observed);
    // Separate streams keep field draws independent of predictive-noise consumption.
    let mut prior_rng = ChaCha8Rng::seed_from_u64(spec.seed ^ 0x6361_725f_7072_696f);
    let mut posterior_rng = ChaCha8Rng::seed_from_u64(spec.seed ^ 0x6361_725f_706f_7374);
    let mut predictive_rng = ChaCha8Rng::seed_from_u64(spec.seed ^ 0x6361_725f_7070_635f);
    for _ in 0..spec.draws {
        charge(
            &mut diagnostics,
            spec,
            (8 * n + 4 * prepared.edges.len()) as u64,
        )?;
        let prior_right = prepared.perturb(false, &mut prior_rng);
        let mut prior = solve(&prepared, &prior_right, false, spec, &mut diagnostics)?;
        let posterior_right = prepared.perturb(true, &mut posterior_rng);
        let mut posterior = solve(&prepared, &posterior_right, true, spec, &mut diagnostics)?;
        for i in 0..n {
            posterior_moments[i].add(posterior[i]);
            prior[i] += prepared.rows[i].noise_sd * predictive_rng.sample::<f64, _>(StandardNormal);
            posterior[i] += mean[i]
                + prepared.rows[i].noise_sd * predictive_rng.sample::<f64, _>(StandardNormal);
        }
        prior_ppc.add(prepared.discrepancies(&prior));
        posterior_ppc.add(prepared.discrepancies(&posterior));
    }
    let regions = prepared
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let posterior_mean = row.prior_mean + mean[i];
            let posterior_sd = posterior_moments[i].sd();
            SparseCarRegionSummary {
                region_id: row.region_id.clone(),
                observed: row.value.is_some(),
                posterior_mean,
                posterior_sd,
                lower_95: posterior_mean - Z_975 * posterior_sd,
                upper_95: posterior_mean + Z_975 * posterior_sd,
                predictive_sd: posterior_sd.hypot(row.noise_sd),
                sd_mcse: posterior_sd / (2.0 * (spec.draws - 1) as f64).sqrt(),
            }
        })
        .collect();
    let result = SparseCarResult {
        format: FORMAT.into(),
        version: 1,
        fit_state: "complete".into(),
        graph_definition: spec.graph_definition.clone(),
        coordinate_frame: spec.coordinate_frame.clone(),
        statistical_unit: "region_conditional_on_declared_graph".into(),
        rho: spec.rho,
        tau: spec.tau,
        seed: spec.seed,
        components: prepared.components,
        components_without_observations: prepared.components_without_observations,
        undirected_edges: prepared.edges.len(),
        regions,
        prior_predictive: prior_ppc.finish(),
        posterior_predictive: posterior_ppc.finish(),
        diagnostics,
        claim_status: CLAIM.into(),
    };
    result.validate_prepared(spec, &prepared)?;
    Ok(result)
}

struct Edge {
    i: usize,
    j: usize,
    weight: f64,
    coupling: f64,
}

struct Prepared<'a> {
    rows: Vec<&'a SparseCarObservation>,
    edges: Vec<Edge>,
    base: Vec<f64>,
    prior_diagonal: Vec<f64>,
    posterior_diagonal: Vec<f64>,
    observation_precision: Vec<f64>,
    observed_residual: Vec<f64>,
    prior_eigenvalue_lower_bound: f64,
    posterior_eigenvalue_lower_bound: f64,
    components: usize,
    components_without_observations: usize,
    estimated_peak_bytes: usize,
}

impl<'a> Prepared<'a> {
    fn new(spec: &'a SparseCarSpec) -> Result<Self, SparseCarError> {
        let valid_text =
            |text: &str, max: usize| !text.is_empty() && text.trim() == text && text.len() <= max;
        if !valid_text(&spec.graph_definition, 4096)
            || !valid_text(&spec.coordinate_frame, 256)
            || !(2..=100_000).contains(&spec.observations.len())
            || spec.edges.len() > 2_000_000
            || !(256..=16_384).contains(&spec.draws)
            || !(1..=10_000).contains(&spec.maximum_iterations)
            || !(1e-12..=1e-8).contains(&spec.solve_tolerance)
            || !(0.0..1.0).contains(&spec.rho)
            || !spec.tau.is_finite()
            || spec.tau <= 0.0
            || !(1..=500_000_000).contains(&spec.maximum_work)
            || !(1..=1_073_741_824).contains(&spec.memory_budget_bytes)
        {
            return Err(SparseCarError::InvalidInput("require explicit graph/frame, 2–100000 regions, at most 2000000 directed edges, 256–16384 draws, rho in [0,1), positive tau, tolerance 1e-12–1e-8 and bounded solver resources".into()));
        }
        if spec.observations.iter().any(|o| {
            !valid_text(&o.region_id, 256)
                || !o.prior_mean.is_finite()
                || o.value.is_some_and(|v| !v.is_finite())
                || !o.noise_sd.is_finite()
                || o.noise_sd <= 0.0
        }) || !spec.observations.iter().any(|o| o.value.is_some())
            || spec
                .edges
                .iter()
                .any(|e| !valid_text(&e.source_region, 256) || !valid_text(&e.target_region, 256))
        {
            return Err(SparseCarError::InvalidInput("exact bounded region IDs, finite observations/means, positive noise SDs and at least one observed region are required".into()));
        }
        // Includes input/canonicalization maps and their strings, work vectors, summaries,
        // serialization, and a fixed allowance. No draws-by-regions or n-by-n allocation.
        let strings: usize = spec
            .observations
            .iter()
            .map(|r| r.region_id.len())
            .sum::<usize>()
            + spec
                .edges
                .iter()
                .map(|e| e.source_region.len() + e.target_region.len())
                .sum::<usize>();
        let estimated_peak_bytes =
            1_048_576 + spec.observations.len() * 2048 + spec.edges.len() * 512 + 3 * strings;
        if estimated_peak_bytes > spec.memory_budget_bytes {
            return Err(SparseCarError::ResourceLimit(format!(
                "estimated peak {estimated_peak_bytes} bytes exceeds memory budget {}",
                spec.memory_budget_bytes
            )));
        }
        let weights = validate_spatial_weights(
            spec.observations
                .iter()
                .map(|o| o.region_id.clone())
                .collect(),
            spec.edges.clone(),
            SpatialWeightsPolicy {
                symmetry: SymmetryPolicy::Required,
                diagonal: DiagonalPolicy::Zero,
                normalization: NormalizationPolicy::Preserve,
            },
        )
        .map_err(|e| SparseCarError::InvalidInput(e.to_string()))?;
        if !weights.islands.is_empty() {
            return Err(SparseCarError::InvalidInput(
                "proper CAR requires no isolated regions; no island prior is supplied".into(),
            ));
        }
        let mut rows: Vec<_> = spec.observations.iter().collect();
        rows.sort_by(|a, b| a.region_id.cmp(&b.region_id));
        let degrees = weights.row_sums();
        let base: Vec<_> = degrees
            .iter()
            .map(|d| spec.tau * (1.0 - spec.rho) * d)
            .collect();
        let edges: Vec<_> = weights
            .weights
            .iter()
            .filter(|e| e.source_index < e.target_index)
            .map(|e| Edge {
                i: e.source_index,
                j: e.target_index,
                weight: e.weight,
                coupling: spec.tau * spec.rho * e.weight,
            })
            .collect();
        let mut prior_diagonal = base.clone();
        for e in &edges {
            prior_diagonal[e.i] += e.coupling;
            prior_diagonal[e.j] += e.coupling;
        }
        let observation_precision: Vec<_> = rows
            .iter()
            .map(|r| {
                if r.value.is_some() {
                    r.noise_sd.powi(-2)
                } else {
                    0.0
                }
            })
            .collect();
        let posterior_diagonal: Vec<_> = prior_diagonal
            .iter()
            .zip(&observation_precision)
            .map(|(q, w)| q + w)
            .collect();
        let observed_residual: Vec<_> = rows
            .iter()
            .map(|r| r.value.map_or(0.0, |y| y - r.prior_mean))
            .collect();
        if base
            .iter()
            .chain(&prior_diagonal)
            .chain(&posterior_diagonal)
            .any(|v| !v.is_finite() || *v <= 0.0)
            || edges.iter().any(|e| !e.coupling.is_finite())
            || observation_precision
                .iter()
                .zip(&rows)
                .any(|(w, r)| !w.is_finite() || (r.value.is_some() && *w <= 0.0))
            || observed_residual.iter().any(|v| !v.is_finite())
        {
            return Err(SparseCarError::Numerical("precision or centered observations overflowed/underflowed; no numerical repair applied".into()));
        }
        let prior_eigenvalue_lower_bound = base.iter().copied().fold(f64::INFINITY, f64::min);
        let posterior_eigenvalue_lower_bound = base
            .iter()
            .zip(&observation_precision)
            .map(|(b, w)| b + w)
            .fold(f64::INFINITY, f64::min);
        let components_without_observations = weights
            .components
            .iter()
            .filter(|c| c.iter().all(|i| rows[*i].value.is_none()))
            .count();
        let prepared = Self {
            rows,
            edges,
            base,
            prior_diagonal,
            posterior_diagonal,
            observation_precision,
            observed_residual,
            prior_eigenvalue_lower_bound,
            posterior_eigenvalue_lower_bound,
            components: weights.components.len(),
            components_without_observations,
            estimated_peak_bytes,
        };
        let observed = prepared.discrepancies(&prepared.observed_residual);
        if !observed.0.is_finite() || observed.1.is_some_and(|x| !x.is_finite()) {
            return Err(SparseCarError::Numerical(
                "observed predictive discrepancy is not finite".into(),
            ));
        }
        Ok(prepared)
    }

    fn perturb(&self, posterior: bool, rng: &mut ChaCha8Rng) -> Vec<f64> {
        let mut right: Vec<_> = self
            .base
            .iter()
            .zip(&self.observation_precision)
            .map(|(b, w)| {
                (b + if posterior { *w } else { 0.0 }).sqrt() * rng.sample::<f64, _>(StandardNormal)
            })
            .collect();
        for edge in &self.edges {
            let value = edge.coupling.sqrt() * rng.sample::<f64, _>(StandardNormal);
            right[edge.i] += value;
            right[edge.j] -= value;
        }
        right
    }

    fn discrepancies(&self, residual: &[f64]) -> (f64, Option<f64>) {
        let energy = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.value.is_some())
            .map(|(i, r)| (residual[i] / r.noise_sd).powi(2))
            .sum();
        let mut roughness = None;
        for e in &self.edges {
            if self.rows[e.i].value.is_some() && self.rows[e.j].value.is_some() {
                *roughness.get_or_insert(0.0) += e.weight * (residual[e.i] - residual[e.j]).powi(2);
            }
        }
        (energy, roughness)
    }
}

fn charge(
    d: &mut SparseCarDiagnostics,
    spec: &SparseCarSpec,
    work: u64,
) -> Result<(), SparseCarError> {
    d.work_used = d
        .work_used
        .checked_add(work)
        .ok_or_else(|| SparseCarError::ResourceLimit("work counter overflow".into()))?;
    if d.work_used > spec.maximum_work {
        return Err(SparseCarError::ResourceLimit(format!(
            "sparse vector/edge work exceeds {}",
            spec.maximum_work
        )));
    }
    Ok(())
}

fn multiply(prepared: &Prepared<'_>, posterior: bool, x: &[f64], result: &mut [f64]) {
    for i in 0..x.len() {
        result[i] = (prepared.base[i]
            + if posterior {
                prepared.observation_precision[i]
            } else {
                0.0
            })
            * x[i];
    }
    // Incidence form avoids subtracting two large nearly equal diagonal/adjacency terms.
    for e in &prepared.edges {
        let difference = e.coupling * (x[e.i] - x[e.j]);
        result[e.i] += difference;
        result[e.j] -= difference;
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn solve(
    prepared: &Prepared<'_>,
    right: &[f64],
    posterior: bool,
    spec: &SparseCarSpec,
    diagnostics: &mut SparseCarDiagnostics,
) -> Result<Vec<f64>, SparseCarError> {
    let (diagonal, eigen_lower) = if posterior {
        (
            &prepared.posterior_diagonal,
            prepared.posterior_eigenvalue_lower_bound,
        )
    } else {
        (
            &prepared.prior_diagonal,
            prepared.prior_eigenvalue_lower_bound,
        )
    };
    let n = right.len();
    let mut x = vec![0.0; n];
    let right_norm = dot(right, right).sqrt();
    if !right_norm.is_finite() {
        return Err(SparseCarError::Numerical(
            "nonfinite linear right-hand side".into(),
        ));
    }
    diagnostics.linear_solves += 1;
    if right_norm == 0.0 {
        if right.iter().any(|v| *v != 0.0) {
            return Err(SparseCarError::Numerical(
                "right-hand norm underflow".into(),
            ));
        }
        return Ok(x);
    }
    let mut residual = right.to_vec();
    let mut z: Vec<_> = residual.iter().zip(diagonal).map(|(r, d)| r / d).collect();
    let mut direction = z.clone();
    let mut product = vec![0.0; n];
    let mut rz = dot(&residual, &z);
    for iteration in 1..=spec.maximum_iterations {
        charge(diagnostics, spec, (8 * n + 2 * prepared.edges.len()) as u64)?;
        multiply(prepared, posterior, &direction, &mut product);
        let curvature = dot(&direction, &product);
        if !curvature.is_finite() || curvature <= 0.0 || !rz.is_finite() || rz <= 0.0 {
            return Err(SparseCarError::Numerical(
                "sparse solve lost positive finite curvature".into(),
            ));
        }
        let step = rz / curvature;
        for i in 0..n {
            x[i] += step * direction[i];
            residual[i] -= step * product[i];
        }
        let x_norm = dot(&x, &x).sqrt();
        let mut residual_norm = dot(&residual, &residual).sqrt();
        if !x_norm.is_finite() || !residual_norm.is_finite() {
            return Err(SparseCarError::Numerical(
                "sparse solve is not finite".into(),
            ));
        }
        let candidate =
            x_norm > 0.0 && (residual_norm / eigen_lower) / x_norm <= spec.solve_tolerance;
        if candidate {
            // Recursive CG residuals alone cannot establish numerical acceptance.
            charge(diagnostics, spec, (3 * n + 2 * prepared.edges.len()) as u64)?;
            multiply(prepared, posterior, &x, &mut product);
            for i in 0..n {
                residual[i] = right[i] - product[i];
            }
            residual_norm = dot(&residual, &residual).sqrt();
            let absolute_error_bound = residual_norm / eigen_lower;
            // ||x_exact|| >= ||x_returned|| - ||error|| gives a bound relative
            // to the exact solution, not merely the current iterate.
            let relative_error = if absolute_error_bound < x_norm {
                absolute_error_bound / (x_norm - absolute_error_bound)
            } else {
                f64::INFINITY
            };
            if relative_error.is_finite() && relative_error <= spec.solve_tolerance {
                diagnostics.maximum_iterations_used =
                    diagnostics.maximum_iterations_used.max(iteration);
                diagnostics.maximum_relative_residual = diagnostics
                    .maximum_relative_residual
                    .max(residual_norm / right_norm);
                diagnostics.maximum_relative_solution_error_bound = diagnostics
                    .maximum_relative_solution_error_bound
                    .max(relative_error);
                return Ok(x);
            }
        }
        for i in 0..n {
            z[i] = residual[i] / diagonal[i];
        }
        let next_rz = dot(&residual, &z);
        let beta = if candidate { 0.0 } else { next_rz / rz };
        for i in 0..n {
            direction[i] = z[i] + beta * direction[i];
        }
        rz = next_rz;
    }
    Err(SparseCarError::Numerical(format!(
        "verified sparse solve exhausted {} iterations",
        spec.maximum_iterations
    )))
}

#[derive(Clone, Default)]
struct Moments {
    count: usize,
    mean: f64,
    m2: f64,
}

impl Moments {
    fn add(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (value - self.mean);
    }
    fn sd(&self) -> f64 {
        (self.m2 / (self.count - 1) as f64).sqrt()
    }
}

struct CheckAccumulator {
    observed: f64,
    moments: Moments,
    upper: usize,
}

impl CheckAccumulator {
    fn new(observed: f64) -> Self {
        Self {
            observed,
            moments: Moments::default(),
            upper: 0,
        }
    }
    fn add(&mut self, value: f64) {
        self.moments.add(value);
        self.upper += usize::from(value >= self.observed);
    }
    fn finish(self) -> SparseCarPredictiveCheck {
        let n = self.moments.count as f64;
        // Jeffreys-smoothed binomial MCSE remains nonzero for zero/all tail counts.
        let adjusted = (self.upper as f64 + 0.5) / (n + 1.0);
        SparseCarPredictiveCheck {
            observed: self.observed,
            replicate_mean: self.moments.mean,
            replicate_sd: self.moments.sd(),
            upper_tail_probability: self.upper as f64 / n,
            probability_mcse: (adjusted * (1.0 - adjusted) / (n + 2.0)).sqrt(),
        }
    }
}

struct PredictiveAccumulator {
    energy: CheckAccumulator,
    roughness: Option<CheckAccumulator>,
}

impl PredictiveAccumulator {
    fn new(observed: (f64, Option<f64>)) -> Self {
        Self {
            energy: CheckAccumulator::new(observed.0),
            roughness: observed.1.map(CheckAccumulator::new),
        }
    }
    fn add(&mut self, values: (f64, Option<f64>)) {
        self.energy.add(values.0);
        if let (Some(check), Some(value)) = (&mut self.roughness, values.1) {
            check.add(value);
        }
    }
    fn finish(self) -> SparseCarPredictive {
        SparseCarPredictive {
            residual_energy: self.energy.finish(),
            edge_roughness: self.roughness.map(CheckAccumulator::finish),
        }
    }
}

impl SparseCarPredictiveCheck {
    fn valid(&self, observed: f64, draws: usize) -> bool {
        let adjusted = (self.upper_tail_probability * draws as f64 + 0.5) / (draws as f64 + 1.0);
        self.observed == observed
            && [
                self.observed,
                self.replicate_mean,
                self.replicate_sd,
                self.upper_tail_probability,
                self.probability_mcse,
            ]
            .iter()
            .all(|v| v.is_finite())
            && self.observed >= 0.0
            && self.replicate_mean >= 0.0
            && self.replicate_sd >= 0.0
            && (0.0..=1.0).contains(&self.upper_tail_probability)
            && close(
                self.probability_mcse,
                (adjusted * (1.0 - adjusted) / (draws as f64 + 2.0)).sqrt(),
            )
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * (1.0 + a.abs().max(b.abs()))
}

#[cfg(test)]
mod tests;
