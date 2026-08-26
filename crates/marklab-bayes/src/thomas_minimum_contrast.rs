use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::{
    model::{BackendContract, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, FitState, WorkerBackend,
};

const SCIPY_BACKEND_VERSION: &str = "scipy-1.18.1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasKCurveRow {
    pub radius_um: f64,
    pub observed_k_um2: f64,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct ThomasMinimumContrastSpec {
    pub curve: Vec<ThomasKCurveRow>,
    pub observed_intensity_per_um2: f64,
    pub kappa_min_per_um2: f64,
    pub kappa_max_per_um2: f64,
    pub sigma_min_um: f64,
    pub sigma_max_um: f64,
    pub maximum_iterations: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasMinimumContrastModelIr {
    pub family: &'static str,
    pub summary: &'static str,
    pub theoretical_curve: &'static str,
    pub contrast_transform: &'static str,
    pub primary_weights: &'static str,
    pub sensitivity_fits: [&'static str; 2],
    pub offspring_mean_identification: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasMinimumContrastBounds {
    pub kappa_min_per_um2: f64,
    pub kappa_max_per_um2: f64,
    pub sigma_min_um: f64,
    pub sigma_max_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasMinimumContrastResources {
    pub maximum_curve_rows: u32,
    pub maximum_iterations: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThomasMinimumContrastWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: ThomasMinimumContrastModelIr,
    pub curve: Vec<ThomasKCurveRow>,
    pub observed_intensity_per_um2: f64,
    pub bounds: ThomasMinimumContrastBounds,
    pub maximum_iterations: u32,
    pub resources: ThomasMinimumContrastResources,
}

impl ThomasMinimumContrastWorkerRequest {
    pub fn new(
        spec: ThomasMinimumContrastSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !(8..=1_000).contains(&spec.curve.len())
            || !spec.observed_intensity_per_um2.is_finite()
            || spec.observed_intensity_per_um2 <= 0.0
            || ![
                spec.kappa_min_per_um2,
                spec.kappa_max_per_um2,
                spec.sigma_min_um,
                spec.sigma_max_um,
            ]
            .into_iter()
            .all(f64::is_finite)
            || spec.kappa_min_per_um2 <= 0.0
            || spec.kappa_min_per_um2 >= spec.kappa_max_per_um2
            || spec.sigma_min_um <= 0.0
            || spec.sigma_min_um >= spec.sigma_max_um
            || !(10..=100_000).contains(&spec.maximum_iterations)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "Thomas minimum-contrast curve, bounds, intensity, or controls are invalid".into(),
            ));
        }
        for (index, row) in spec.curve.iter().enumerate() {
            if ![row.radius_um, row.observed_k_um2, row.weight]
                .into_iter()
                .all(f64::is_finite)
                || row.radius_um <= 0.0
                || row.observed_k_um2 < 0.0
                || row.weight <= 0.0
                || (index > 0 && row.radius_um <= spec.curve[index - 1].radius_um)
            {
                return Err(BayesError::InvalidSpec(
                    "Thomas K curve requires increasing positive radii, nonnegative K, and positive weights"
                        .into(),
                ));
            }
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: ThomasMinimumContrastModelIr {
                family: "thomas_cluster_process",
                summary: "observed_isotropic_K_curve",
                theoretical_curve: "pi_r_squared_plus_inverse_kappa_times_one_minus_exp_negative_r_squared_over_four_sigma_squared",
                contrast_transform: "fourth_root",
                primary_weights: "caller_supplied_positive",
                sensitivity_fits: ["unit_weights_full_range", "caller_weights_interior_range"],
                offspring_mean_identification: "observed_intensity_divided_by_parent_intensity",
                backend_capability: "bounded_nonlinear_least_squares",
                maturity: "experimental_minimum_contrast",
            },
            curve: spec.curve,
            observed_intensity_per_um2: spec.observed_intensity_per_um2,
            bounds: ThomasMinimumContrastBounds {
                kappa_min_per_um2: spec.kappa_min_per_um2,
                kappa_max_per_um2: spec.kappa_max_per_um2,
                sigma_min_um: spec.sigma_min_um,
                sigma_max_um: spec.sigma_max_um,
            },
            maximum_iterations: spec.maximum_iterations,
            resources: ThomasMinimumContrastResources {
                maximum_curve_rows: 1_000,
                maximum_iterations: 100_000,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasMinimumContrastFitSummary {
    pub fit_name: String,
    pub weight_rule: String,
    pub first_row: u32,
    pub last_row_exclusive: u32,
    pub row_count: u32,
    pub kappa_parent_per_um2: f64,
    pub sigma_um: f64,
    pub mu_offspring: f64,
    pub objective: f64,
    pub converged: bool,
    pub status: i32,
    pub message: String,
    pub evaluations: u64,
    pub optimality: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasMinimumContrastCurveFitRow {
    pub radius_um: f64,
    pub observed_k_um2: f64,
    pub weight: f64,
    pub fitted_k_um2: f64,
    pub transformed_residual: f64,
    pub weighted_squared_contribution: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThomasMinimumContrastWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub primary_fit: ThomasMinimumContrastFitSummary,
    pub sensitivity_fits: Vec<ThomasMinimumContrastFitSummary>,
    pub curve: Vec<ThomasMinimumContrastCurveFitRow>,
}

impl ThomasMinimumContrastWorkerResult {
    pub fn validate(
        &self,
        request: &ThomasMinimumContrastWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.scipy_thomas_minimum_contrast_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.sensitivity_fits.len() != 2
            || self.curve.len() != request.curve.len()
        {
            return Err(BayesError::WorkerContract(
                "Thomas minimum-contrast result identity or dimensions mismatch".into(),
            ));
        }
        validate_fit(
            &self.primary_fit,
            request,
            "primary_weighted_full_range",
            0,
            request.curve.len(),
        )?;
        validate_fit(
            &self.sensitivity_fits[0],
            request,
            "sensitivity_unweighted_full_range",
            0,
            request.curve.len(),
        )?;
        validate_fit(
            &self.sensitivity_fits[1],
            request,
            "sensitivity_weighted_interior_range",
            1,
            request.curve.len() - 1,
        )?;
        let mut objective = 0.0;
        for (actual, observed) in self.curve.iter().zip(&request.curve) {
            let fitted = thomas_k(
                observed.radius_um,
                self.primary_fit.kappa_parent_per_um2,
                self.primary_fit.sigma_um,
            );
            let residual = observed.observed_k_um2.sqrt().sqrt() - fitted.sqrt().sqrt();
            let contribution = observed.weight * residual * residual;
            if !close(actual.radius_um, observed.radius_um)
                || !close(actual.observed_k_um2, observed.observed_k_um2)
                || !close(actual.weight, observed.weight)
                || !close(actual.fitted_k_um2, fitted)
                || !close(actual.transformed_residual, residual)
                || !close(actual.weighted_squared_contribution, contribution)
            {
                return Err(BayesError::WorkerContract(
                    "Thomas minimum-contrast curve arithmetic mismatch".into(),
                ));
            }
            objective += contribution;
        }
        if !close(self.primary_fit.objective, objective) {
            return Err(BayesError::WorkerContract(
                "Thomas minimum-contrast primary objective mismatch".into(),
            ));
        }
        let complete = std::iter::once(&self.primary_fit)
            .chain(self.sensitivity_fits.iter())
            .all(|fit| fit.converged && fit.optimality <= 1e-6);
        if (self.fit_state == FitState::Complete) != complete {
            return Err(BayesError::WorkerContract(
                "Thomas minimum-contrast fit state disagrees with optimizer diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: ThomasMinimumContrastWorkerRequest,
        input: ThomasMinimumContrastInputIdentity,
    ) -> ThomasMinimumContrastResult {
        ThomasMinimumContrastResult {
            format: "marklab.thomas_minimum_contrast_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            observed_intensity_per_um2: request.observed_intensity_per_um2,
            bounds: request.bounds,
            fit_state: self.fit_state,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_minimum_contrast"
            } else {
                "diagnostic_only_nonconverged"
            },
            primary_fit: self.primary_fit,
            sensitivity_fits: self.sensitivity_fits,
            curve: self.curve,
            likelihood_comparison: ThomasLikelihoodComparison {
                state: "unavailable",
                reason: "latent_parent_likelihood_backend_not_admitted",
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_fit(
    fit: &ThomasMinimumContrastFitSummary,
    request: &ThomasMinimumContrastWorkerRequest,
    name: &str,
    first: usize,
    last: usize,
) -> Result<(), BayesError> {
    if fit.fit_name != name
        || fit.first_row != first as u32
        || fit.last_row_exclusive != last as u32
        || fit.row_count != (last - first) as u32
        || fit.weight_rule.is_empty()
        || fit.message.is_empty()
        || fit.evaluations == 0
        || ![
            fit.kappa_parent_per_um2,
            fit.sigma_um,
            fit.mu_offspring,
            fit.objective,
            fit.optimality,
        ]
        .into_iter()
        .all(f64::is_finite)
        || !(request.bounds.kappa_min_per_um2..=request.bounds.kappa_max_per_um2)
            .contains(&fit.kappa_parent_per_um2)
        || !(request.bounds.sigma_min_um..=request.bounds.sigma_max_um).contains(&fit.sigma_um)
        || fit.mu_offspring <= 0.0
        || fit.objective < 0.0
        || fit.optimality < 0.0
        || !close(
            fit.mu_offspring,
            request.observed_intensity_per_um2 / fit.kappa_parent_per_um2,
        )
    {
        return Err(BayesError::WorkerContract(
            "Thomas minimum-contrast optimizer summary is invalid".into(),
        ));
    }
    let objective = request.curve[first..last]
        .iter()
        .map(|row| {
            let fitted = thomas_k(row.radius_um, fit.kappa_parent_per_um2, fit.sigma_um);
            let residual = row.observed_k_um2.sqrt().sqrt() - fitted.sqrt().sqrt();
            let weight = if name == "sensitivity_unweighted_full_range" {
                1.0
            } else {
                row.weight
            };
            weight * residual * residual
        })
        .sum::<f64>();
    if !close(fit.objective, objective) {
        return Err(BayesError::WorkerContract(
            "Thomas minimum-contrast sensitivity objective mismatch".into(),
        ));
    }
    Ok(())
}

fn thomas_k(radius: f64, kappa: f64, sigma: f64) -> f64 {
    PI * radius * radius + (1.0 / kappa) * (1.0 - (-radius * radius / (4.0 * sigma * sigma)).exp())
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-8 * actual.abs().max(expected.abs()).max(1.0)
}

#[derive(Debug, Serialize)]
pub struct ThomasMinimumContrastInputIdentity {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ThomasLikelihoodComparison {
    pub state: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ThomasMinimumContrastResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: ThomasMinimumContrastModelIr,
    pub input: ThomasMinimumContrastInputIdentity,
    pub observed_intensity_per_um2: f64,
    pub bounds: ThomasMinimumContrastBounds,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub primary_fit: ThomasMinimumContrastFitSummary,
    pub sensitivity_fits: Vec<ThomasMinimumContrastFitSummary>,
    pub curve: Vec<ThomasMinimumContrastCurveFitRow>,
    pub likelihood_comparison: ThomasLikelihoodComparison,
    pub request_sha256: String,
}
