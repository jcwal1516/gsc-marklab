use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab::{ArbitraryWindowIppWindowSummary, ObservationWindow2D};
use marklab_bayes::{
    sha256_hex, FitState, InhomogeneousPoissonEvent, InhomogeneousPoissonFitInputIdentity,
    InhomogeneousPoissonFitSpec, InhomogeneousPoissonFitWorkerRequest,
    InhomogeneousPoissonFitWorkerResult, MidpointQuadratureValue, NormalMeanDiagnostics,
    NutsSamplingSpec, RectangularWindow, SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_ipp::{prepare as prepare_inputs, PreparedArbitraryWindowIpp},
    publish_json, run_worker, BayesCliError,
};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct FitCli {
    #[command(subcommand)]
    command: FitTopLevel,
}

#[derive(Debug, Subcommand)]
enum FitTopLevel {
    Bayes {
        #[command(subcommand)]
        command: FitCommand,
    },
}

#[derive(Debug, Subcommand)]
enum FitCommand {
    FitArbitraryWindowIpp(Box<FitArguments>),
}

#[derive(Debug, clap::Args)]
struct FitArguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let FitTopLevel::Bayes { command } = FitCli::parse_from(std::env::args_os()).command;
    let FitCommand::FitArbitraryWindowIpp(arguments) = command;
    let prepared = prepare(
        arguments.events,
        arguments.quadrature,
        arguments.window,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.coefficient_prior_mean,
        arguments.coefficient_prior_sd,
        NutsSamplingSpec {
            chains: arguments.chains,
            tune_per_chain: arguments.tune,
            draws_per_chain: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
        },
        arguments.maximum_events,
        arguments.maximum_quadrature_nodes,
        arguments.maximum_draw_node_work,
        arguments.timeout_seconds,
    )?;
    let result = execute(&prepared)?;
    publish_json(&arguments.out, &result)
}

pub(crate) struct PreparedArbitraryWindowIppFit {
    pub(crate) input: PreparedArbitraryWindowIpp,
    pub(crate) request: InhomogeneousPoissonFitWorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) input_identity: ArbitraryWindowIppFitInputIdentity,
    pub(crate) maximum_draw_node_work: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArbitraryWindowIppFitInputIdentity {
    pub events_digest: String,
    pub quadrature_digest: String,
    pub window_logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArbitraryWindowIppFitModel {
    pub family: String,
    pub coordinate_unit: String,
    pub window: String,
    pub quadrature: String,
    pub covariates: String,
    pub likelihood: String,
    pub backend_adapter: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArbitraryWindowIppFitNodeSummary {
    pub node_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub weight_um2: f64,
    pub covariate: f64,
    pub offset: f64,
    pub intensity_per_um2: SarScalarSummary,
    pub expected_count: SarScalarSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArbitraryWindowIppFitPredictive {
    pub observed_total_count: u64,
    pub total_expected_count_mean: f64,
    pub replicated_total_mean: f64,
    pub replicated_total_sd: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArbitraryWindowIppFitPosterior {
    pub intercept: SarScalarSummary,
    pub coefficient: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArbitraryWindowIppFitResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: ArbitraryWindowIppFitModel,
    pub input: ArbitraryWindowIppFitInputIdentity,
    pub window: ArbitraryWindowIppWindowSummary,
    pub observed_event_count: usize,
    pub quadrature_node_count: usize,
    pub quadrature_weight_um2: f64,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: ArbitraryWindowIppFitPosterior,
    pub nodes: Vec<ArbitraryWindowIppFitNodeSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: ArbitraryWindowIppFitPredictive,
    pub seed: u64,
    pub request_sha256: String,
    pub maximum_draw_node_work: u64,
    pub statistical_unit: String,
    pub finite_result_policy: String,
    pub claim_status: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    events_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    sampling: NutsSamplingSpec,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_draw_node_work: u64,
    timeout_seconds: u64,
) -> Result<PreparedArbitraryWindowIppFit, BayesCliError> {
    if maximum_quadrature_nodes > 1_024 || maximum_draw_node_work == 0 {
        return Err(BayesCliError::Input(
            "arbitrary-window IPP fit requires at most 1024 quadrature nodes and positive draw-node work"
                .into(),
        ));
    }
    let maximum_work = maximum_events
        .checked_add(maximum_quadrature_nodes)
        .ok_or_else(|| BayesCliError::Input("arbitrary-window fit work overflows".into()))?;
    let mut input = prepare_inputs(
        events_path,
        quadrature_path,
        window_path,
        0.0,
        0.0,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        16 * 1024 * 1024,
    )?;
    input
        .spec
        .events
        .sort_by(|left, right| left.event_id.cmp(&right.event_id));
    input
        .spec
        .quadrature
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let identity = super::arbitrary_window_ipp::execute(&input)?;
    let nodes = input.spec.quadrature.len();
    let draw_node_work = u64::from(sampling.chains)
        .checked_mul(u64::from(sampling.draws_per_chain))
        .and_then(|draws| draws.checked_mul(nodes as u64))
        .ok_or_else(|| BayesCliError::Input("arbitrary-window draw-node work overflows".into()))?;
    if draw_node_work > maximum_draw_node_work {
        return Err(BayesCliError::Input(format!(
            "arbitrary-window draw-node work exceeds maximum: {draw_node_work} > {maximum_draw_node_work}"
        )));
    }
    let repository = &marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_pymc_inhomogeneous_poisson_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = InhomogeneousPoissonFitWorkerRequest::new(
        InhomogeneousPoissonFitSpec {
            window: RectangularWindow {
                xmin_um: 0.0,
                ymin_um: 0.0,
                xmax_um: nodes as f64,
                ymax_um: 1.0,
            },
            grid_x: nodes as u32,
            grid_y: 1,
            events: input
                .spec
                .events
                .iter()
                .map(|event| InhomogeneousPoissonEvent {
                    event_id: event.event_id.clone(),
                    x_um: 0.5,
                    y_um: 0.5,
                    covariate: event.covariate,
                    offset: event.offset,
                })
                .collect(),
            quadrature: input
                .spec
                .quadrature
                .iter()
                .enumerate()
                .map(|(index, node)| MidpointQuadratureValue {
                    ix: index as u32,
                    iy: 0,
                    covariate: node.covariate,
                    offset: node.offset + node.weight_um2.ln(),
                })
                .collect(),
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
        },
        sampling,
        sha256_hex(&lock),
        sha256_hex(&worker),
        timeout_seconds,
    )?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedArbitraryWindowIppFit {
        input,
        request,
        request_bytes,
        request_sha256,
        input_identity: ArbitraryWindowIppFitInputIdentity {
            events_digest: identity.events_digest,
            quadrature_digest: identity.quadrature_digest,
            window_logical_digest: identity.window.logical_digest,
        },
        maximum_draw_node_work,
    })
}

pub(crate) fn execute(
    prepared: &PreparedArbitraryWindowIppFit,
) -> Result<ArbitraryWindowIppFitResult, BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
    let bytes = run_worker(
        repository,
        "marklab_pymc_inhomogeneous_poisson_worker.py",
        &prepared.request_bytes,
        prepared.request.resources.timeout_seconds,
    )?;
    let worker: InhomogeneousPoissonFitWorkerResult = serde_json::from_slice(&bytes)?;
    worker.validate(&prepared.request, &prepared.request_sha256)?;
    let fit = worker.into_fit(
        prepared.request.clone(),
        InhomogeneousPoissonFitInputIdentity {
            events_path: "algebraic_weighted_quadrature_adapter".into(),
            quadrature_path: "algebraic_weighted_quadrature_adapter".into(),
            events_sha256: prepared.input_identity.events_digest.clone(),
            quadrature_sha256: prepared.input_identity.quadrature_digest.clone(),
        },
    );
    let nodes = fit
        .cells
        .into_iter()
        .zip(&prepared.input.spec.quadrature)
        .map(|(cell, node)| ArbitraryWindowIppFitNodeSummary {
            node_id: node.node_id.clone(),
            x_um: node.x_um,
            y_um: node.y_um,
            weight_um2: node.weight_um2,
            covariate: node.covariate,
            offset: node.offset,
            intensity_per_um2: divide_summary(cell.intensity, node.weight_um2),
            expected_count: cell.expected_count,
        })
        .collect::<Vec<_>>();
    let total_expected_count_mean = nodes
        .iter()
        .map(|node| node.expected_count.mean)
        .sum::<f64>();
    let descriptor = prepared.input.window.descriptor();
    let result = ArbitraryWindowIppFitResult {
        format: "marklab.bayesian_arbitrary_window_ipp_fit".into(),
        version: 1,
        backend: fit.backend,
        model: ArbitraryWindowIppFitModel {
            family: "log_linear_inhomogeneous_poisson_process".into(),
            coordinate_unit: "micrometer".into(),
            window: "exact_multipolygon".into(),
            quadrature: "positive_weighted_area_partition".into(),
            covariates: "one_fixed_event_and_quadrature_covariate_plus_offset".into(),
            likelihood: "event_linear_predictor_sum_minus_weighted_quadrature_integral".into(),
            backend_adapter: "equal_area_synthetic_grid_with_log_weight_offsets".into(),
        },
        input: prepared.input_identity.clone(),
        window: window_summary(&prepared.input.window),
        observed_event_count: prepared.input.spec.events.len(),
        quadrature_node_count: prepared.input.spec.quadrature.len(),
        quadrature_weight_um2: descriptor.area_um2,
        sampling: fit.sampling,
        fit_state: fit.fit_state,
        posterior: ArbitraryWindowIppFitPosterior {
            intercept: fit.posterior.intercept,
            coefficient: fit.posterior.coefficient,
        },
        nodes,
        diagnostics: fit.diagnostics,
        posterior_predictive: ArbitraryWindowIppFitPredictive {
            observed_total_count: fit.posterior_predictive.observed_total_count,
            total_expected_count_mean,
            replicated_total_mean: fit.posterior_predictive.replicated_total_mean,
            replicated_total_sd: fit.posterior_predictive.replicated_total_sd,
        },
        seed: prepared.request.sampling.seed,
        request_sha256: fit.request_sha256,
        maximum_draw_node_work: prepared.maximum_draw_node_work,
        statistical_unit: "one_observed_point_pattern".into(),
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if fit.fit_state == FitState::Complete {
            "experimental_single_pattern_association".into()
        } else {
            "diagnostic_only_nonconverged".into()
        },
    };
    result.validate(prepared)?;
    Ok(result)
}

impl ArbitraryWindowIppFitResult {
    pub(crate) fn validate(
        &self,
        prepared: &PreparedArbitraryWindowIppFit,
    ) -> Result<(), BayesCliError> {
        let descriptor = prepared.input.window.descriptor();
        let expected_draws = u64::from(prepared.request.sampling.chains)
            * u64::from(prepared.request.sampling.draws_per_chain);
        let node_identity_matches =
            self.nodes
                .iter()
                .zip(&prepared.input.spec.quadrature)
                .all(|(result, input)| {
                    result.node_id == input.node_id
                        && equal(result.x_um, input.x_um)
                        && equal(result.y_um, input.y_um)
                        && equal(result.weight_um2, input.weight_um2)
                        && equal(result.covariate, input.covariate)
                        && equal(result.offset, input.offset)
                        && scalar_valid(&result.intensity_per_um2, true)
                        && scalar_valid(&result.expected_count, true)
                });
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= prepared.request.diagnostic_policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= prepared.request.diagnostic_policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= prepared.request.diagnostic_policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= prepared.request.diagnostic_policy.minimum_ebfmi
            && self.diagnostics.divergences
                <= prepared.request.diagnostic_policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits
                <= prepared.request.diagnostic_policy.maximum_tree_depth_hits;
        if self.format != "marklab.bayesian_arbitrary_window_ipp_fit"
            || self.version != 1
            || self.backend.name != prepared.request.backend.name
            || self.backend.version != prepared.request.backend.version
            || self.backend.python_version != prepared.request.backend.python_version
            || self.backend.environment_lock_sha256
                != prepared.request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != prepared.request.backend.worker_sha256
            || self.request_sha256 != prepared.request_sha256
            || self.input.events_digest != prepared.input_identity.events_digest
            || self.input.quadrature_digest != prepared.input_identity.quadrature_digest
            || self.input.window_logical_digest != descriptor.logical_digest.to_string()
            || self.window.logical_digest != descriptor.logical_digest.to_string()
            || self.observed_event_count != prepared.input.spec.events.len()
            || self.quadrature_node_count != prepared.input.spec.quadrature.len()
            || self.nodes.len() != prepared.input.spec.quadrature.len()
            || !equal(self.quadrature_weight_um2, descriptor.area_um2)
            || self.sampling.chains != prepared.request.sampling.chains
            || self.sampling.tune_per_chain != prepared.request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != prepared.request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.seed != prepared.request.sampling.seed
            || self.maximum_draw_node_work != prepared.maximum_draw_node_work
            || !scalar_valid(&self.posterior.intercept, false)
            || !scalar_valid(&self.posterior.coefficient, false)
            || !node_identity_matches
            || ![
                self.posterior_predictive.total_expected_count_mean,
                self.posterior_predictive.replicated_total_mean,
                self.posterior_predictive.replicated_total_sd,
            ]
            .into_iter()
            .all(f64::is_finite)
            || self.posterior_predictive.total_expected_count_mean <= 0.0
            || self.posterior_predictive.replicated_total_mean < 0.0
            || self.posterior_predictive.replicated_total_sd < 0.0
            || (self.fit_state == FitState::Complete) != diagnostics_pass
            || self.statistical_unit != "one_observed_point_pattern"
            || self.finite_result_policy != "nonconverged_is_diagnostic_only_reject_non_finite"
            || (self.fit_state == FitState::Complete
                && self.claim_status != "experimental_single_pattern_association")
            || (self.fit_state != FitState::Complete
                && self.claim_status != "diagnostic_only_nonconverged")
        {
            return Err(BayesCliError::Backend(
                "arbitrary-window IPP fit result identity or diagnostics differ".into(),
            ));
        }
        Ok(())
    }
}

fn window_summary(window: &ObservationWindow2D) -> ArbitraryWindowIppWindowSummary {
    let descriptor = window.descriptor();
    ArbitraryWindowIppWindowSummary {
        area_um2: descriptor.area_um2,
        perimeter_um: descriptor.perimeter_um,
        bounds_um: descriptor.bounds_um,
        component_count: descriptor.component_count,
        hole_count: descriptor.hole_count,
        ring_count: descriptor.ring_count,
        vertex_count: descriptor.vertex_count,
        logical_digest: descriptor.logical_digest.to_string(),
    }
}

fn divide_summary(value: SarScalarSummary, divisor: f64) -> SarScalarSummary {
    SarScalarSummary {
        mean: value.mean / divisor,
        sd: value.sd / divisor,
        interval_lower: value.interval_lower / divisor,
        interval_upper: value.interval_upper / divisor,
    }
}

fn scalar_valid(value: &SarScalarSummary, positive: bool) -> bool {
    [
        value.mean,
        value.sd,
        value.interval_lower,
        value.interval_upper,
    ]
    .into_iter()
    .all(f64::is_finite)
        && value.sd > 0.0
        && value.interval_lower <= value.interval_upper
        && (!positive || (value.mean > 0.0 && value.interval_lower >= 0.0))
}

fn equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<FitCli>(|| run_cli().map_err(super::into_marklab_error))
}
