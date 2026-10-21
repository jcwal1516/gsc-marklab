pub(crate) use marklab_bayes::protocol::arbitrary_window_ipp_fit::{
    divide_summary, ArbitraryWindowIppFitInputIdentity, ArbitraryWindowIppFitModel,
    ArbitraryWindowIppFitNodeSummary, ArbitraryWindowIppFitPosterior,
    ArbitraryWindowIppFitPredictive, ArbitraryWindowIppFitResult,
};

use std::fs;
use std::path::PathBuf;

use marklab::spatial::ArbitraryWindowIppWindowSummary;
use marklab::ObservationWindow2D;
use marklab_bayes::sha256_hex;
use marklab_bayes::FitState;
use marklab_bayes::InhomogeneousPoissonEvent;
use marklab_bayes::InhomogeneousPoissonFitInputIdentity;
use marklab_bayes::InhomogeneousPoissonFitSpec;
use marklab_bayes::InhomogeneousPoissonFitWorkerRequest;
use marklab_bayes::InhomogeneousPoissonFitWorkerResult;
use marklab_bayes::MidpointQuadratureValue;
use marklab_bayes::NutsSamplingSpec;
use marklab_bayes::RectangularWindow;

use super::arbitrary_window_ipp::prepare as prepare_inputs;
use super::arbitrary_window_ipp::PreparedArbitraryWindowIpp;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct FitArguments {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    events: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    window: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    coefficient_prior_sd: f64,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    seed: u64,
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    maximum_draw_node_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: FitArguments) -> Result<(), CliError> {
    let prepared = prepare(crate::bayes::ArbitraryWindowIppFitParameters {
        events_path: arguments.events,
        quadrature_path: arguments.quadrature,
        window_path: arguments.window,
        intercept_prior_mean: arguments.intercept_prior_mean,
        intercept_prior_sd: arguments.intercept_prior_sd,
        coefficient_prior_mean: arguments.coefficient_prior_mean,
        coefficient_prior_sd: arguments.coefficient_prior_sd,
        sampling: NutsSamplingSpec {
            chains: arguments.chains,
            tune_per_chain: arguments.tune,
            draws_per_chain: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
        },
        maximum_events: arguments.maximum_events,
        maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
        maximum_draw_node_work: arguments.maximum_draw_node_work,
        timeout_seconds: arguments.timeout_seconds,
    })?;
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

pub(crate) struct Parameters {
    pub(crate) events_path: PathBuf,
    pub(crate) quadrature_path: PathBuf,
    pub(crate) window_path: PathBuf,
    pub(crate) intercept_prior_mean: f64,
    pub(crate) intercept_prior_sd: f64,
    pub(crate) coefficient_prior_mean: f64,
    pub(crate) coefficient_prior_sd: f64,
    pub(crate) sampling: NutsSamplingSpec,
    pub(crate) maximum_events: usize,
    pub(crate) maximum_quadrature_nodes: usize,
    pub(crate) maximum_draw_node_work: u64,
    pub(crate) timeout_seconds: u64,
}

pub(crate) fn prepare(parameters: Parameters) -> Result<PreparedArbitraryWindowIppFit, CliError> {
    let Parameters {
        events_path,
        quadrature_path,
        window_path,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        sampling,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        timeout_seconds,
    } = parameters;
    if maximum_quadrature_nodes > 1_024 || maximum_draw_node_work == 0 {
        return Err(CliError::Input(
            "arbitrary-window IPP fit requires at most 1024 quadrature nodes and positive draw-node work"
                .into(),
        ));
    }
    let maximum_work = maximum_events
        .checked_add(maximum_quadrature_nodes)
        .ok_or_else(|| CliError::Input("arbitrary-window fit work overflows".into()))?;
    let mut input = prepare_inputs(
        events_path,
        quadrature_path,
        window_path,
        0.0,
        0.0,
        marklab::spatial::ArbitraryWindowIppLimits {
            maximum_events,
            maximum_quadrature_nodes,
            maximum_work,
            maximum_retained_bytes: 16 * 1024 * 1024,
        },
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
        .ok_or_else(|| CliError::Input("arbitrary-window draw-node work overflows".into()))?;
    if draw_node_work > maximum_draw_node_work {
        return Err(CliError::Input(format!(
            "arbitrary-window draw-node work exceeds maximum: {draw_node_work} > {maximum_draw_node_work}"
        )));
    }
    let repository = &marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_pymc_inhomogeneous_poisson_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| CliError::Io {
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
) -> Result<ArbitraryWindowIppFitResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
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
        statistical_unit: StatisticalUnit::OneObservedPointPattern,
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if fit.fit_state == FitState::Complete {
            ClaimStatus::ExperimentalSinglePatternAssociation
        } else {
            ClaimStatus::DiagnosticOnlyNonconverged
        },
    };
    prepared.validate_result(&result)?;
    Ok(result)
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

impl PreparedArbitraryWindowIppFit {
    pub(crate) fn validate_result(
        &self,
        result: &ArbitraryWindowIppFitResult,
    ) -> Result<(), CliError> {
        let descriptor = self.input.window.descriptor();
        result
            .validate(
                &marklab_bayes::protocol::arbitrary_window_ipp_fit::ValidationContext {
                    request: &self.request,
                    request_sha256: &self.request_sha256,
                    input_identity: &self.input_identity,
                    window_logical_digest: descriptor.logical_digest.to_string(),
                    window_area_um2: descriptor.area_um2,
                    observed_event_count: self.input.spec.events.len(),
                    quadrature: &self.input.spec.quadrature,
                    maximum_draw_node_work: self.maximum_draw_node_work,
                },
            )
            .map_err(super::protocol_error)
    }
}
