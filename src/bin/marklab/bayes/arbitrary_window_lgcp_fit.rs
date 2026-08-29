use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab::ArbitraryWindowIppWindowSummary;
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, GriddedLgcpFitSpec, GriddedLgcpFitWorkerRequest,
    GriddedLgcpPosterior, GriddedLgcpPosteriorPredictive, GriddedLgcpSpec,
    InhomogeneousPoissonEvent, MidpointQuadratureValue, NormalMeanDiagnostics, NutsSamplingSpec,
    RectangularWindow, SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_ipp, arbitrary_window_ipp_membership, publish_json, run_worker, BayesCliError,
};

const PYMC_VERSION: &str = "6.3.0";

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
    FitArbitraryWindowLgcp(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    event_membership: PathBuf,
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
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
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
    prediction_replicates: u32,
    #[arg(long)]
    prediction_seed: u64,
    #[arg(long)]
    maximum_predictive_points: u64,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InputIdentity {
    events_digest: String,
    event_membership_digest: String,
    quadrature_digest: String,
    window_logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PhysicalNode {
    node_id: String,
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    covariate: f64,
    offset: f64,
    count: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkerRequest {
    format: &'static str,
    version: u32,
    pub(crate) backend: BackendContract,
    pub(crate) source_request_sha256: String,
    pub(crate) source_request: GriddedLgcpFitWorkerRequest,
    input: InputIdentity,
    window: ArbitraryWindowIppWindowSummary,
    nodes: Vec<PhysicalNode>,
    physical_covariance: Vec<f64>,
    physical_cholesky: Vec<f64>,
    neighbor_pairs: Vec<[usize; 2]>,
    neighbor_radius_um: f64,
    maximum_neighbor_pairs: usize,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    covariance_sha256: String,
    maximum_draw_node_work: u64,
    dense_factorization_work_units: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultModel {
    family: String,
    coordinate_unit: String,
    window: String,
    quadrature: String,
    covariates: String,
    latent_field: String,
    kernel: String,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    likelihood: String,
    backend_adapter: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResultNode {
    pub(crate) node_id: String,
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    covariate: f64,
    offset: f64,
    count: u64,
    pub(crate) latent_effect: SarScalarSummary,
    intensity_per_um2: SarScalarSummary,
    pub(crate) expected_count: SarScalarSummary,
    pearson_residual: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialPpcSummary {
    observed: f64,
    replicated_mean: f64,
    replicated_sd: f64,
    probability_replicated_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpatialPpc {
    replicate_count: u32,
    prediction_seed: u64,
    neighbor_radius_um: f64,
    neighbor_pair_count: usize,
    node_density_variance: SpatialPpcSummary,
    neighbor_density_mean_absolute_difference: SpatialPpcSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FitResult {
    format: String,
    version: u32,
    pub(crate) backend: WorkerBackend,
    pub(crate) source_backend: WorkerBackend,
    model: ResultModel,
    pub(crate) input: InputIdentity,
    window: ArbitraryWindowIppWindowSummary,
    observed_event_count: usize,
    quadrature_node_count: usize,
    quadrature_weight_um2: f64,
    sampling: SamplingSummary,
    pub(crate) fit_state: FitState,
    pub(crate) posterior: GriddedLgcpPosterior,
    pub(crate) nodes: Vec<ResultNode>,
    pub(crate) diagnostics: NormalMeanDiagnostics,
    posterior_predictive: GriddedLgcpPosteriorPredictive,
    pub(crate) spatial_posterior_predictive: Option<SpatialPpc>,
    seed: u64,
    pub(crate) request_sha256: String,
    source_request_sha256: String,
    covariance_sha256: String,
    draw_node_work: u64,
    maximum_draw_node_work: u64,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_predictive_points: u64,
    maximum_neighbor_pairs: usize,
    dense_covariance_elements: usize,
    dense_factorization_work_units: u64,
    statistical_unit: String,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct PreparedArbitraryWindowLgcpFit {
    pub(crate) request: WorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) timeout_seconds: u64,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let FitTopLevel::Bayes { command } = FitCli::parse_from(std::env::args_os()).command;
    let FitCommand::FitArbitraryWindowLgcp(arguments) = command;
    let output_path = arguments.out.clone();
    let prepared = prepare_arguments(*arguments)?;
    publish_json(&output_path, &execute(&prepared)?)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    events: PathBuf,
    event_membership: PathBuf,
    quadrature: PathBuf,
    window: PathBuf,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_draw_node_work: u64,
    prediction_replicates: u32,
    prediction_seed: u64,
    maximum_predictive_points: u64,
    neighbor_radius_um: f64,
    maximum_neighbor_pairs: usize,
    timeout_seconds: u64,
) -> Result<PreparedArbitraryWindowLgcpFit, BayesCliError> {
    prepare_arguments(Arguments {
        events,
        event_membership,
        quadrature,
        window,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        chains: sampling.chains,
        tune: sampling.tune_per_chain,
        draws: sampling.draws_per_chain,
        target_accept: sampling.target_accept,
        seed: sampling.seed,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        prediction_replicates,
        prediction_seed,
        maximum_predictive_points,
        neighbor_radius_um,
        maximum_neighbor_pairs,
        timeout_seconds,
        out: PathBuf::new(),
    })
}

fn prepare_arguments(
    arguments: Arguments,
) -> Result<PreparedArbitraryWindowLgcpFit, BayesCliError> {
    if !(4..=36).contains(&arguments.maximum_quadrature_nodes)
        || arguments.maximum_draw_node_work == 0
        || arguments.maximum_draw_node_work > 1_000_000
        || !arguments.neighbor_radius_um.is_finite()
        || arguments.neighbor_radius_um <= 0.0
        || arguments.maximum_neighbor_pairs == 0
        || arguments.maximum_neighbor_pairs > 1_000_000
    {
        return Err(BayesCliError::Input(
            "arbitrary-window LGCP requires 4..36 nodes and bounded positive draw-node work".into(),
        ));
    }
    let maximum_work = arguments
        .maximum_events
        .checked_add(arguments.maximum_quadrature_nodes)
        .ok_or_else(|| BayesCliError::Input("arbitrary-window LGCP work overflows".into()))?;
    let mut prepared = arbitrary_window_ipp::prepare(
        arguments.events,
        arguments.quadrature,
        arguments.window,
        0.0,
        0.0,
        arguments.maximum_events,
        arguments.maximum_quadrature_nodes,
        maximum_work,
        16 * 1024 * 1024,
    )?;
    prepared
        .spec
        .events
        .sort_by(|left, right| left.event_id.cmp(&right.event_id));
    prepared
        .spec
        .quadrature
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let identity = arbitrary_window_ipp::execute(&prepared)?;
    let membership = arbitrary_window_ipp_membership::prepare(
        &arguments.event_membership,
        &prepared.spec.events,
        &prepared.spec.quadrature,
    )?;
    let nodes = prepared.spec.quadrature.len();
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let draw_node_work = u64::from(arguments.chains)
        .checked_mul(u64::from(arguments.draws))
        .and_then(|draws| draws.checked_mul(nodes as u64))
        .ok_or_else(|| BayesCliError::Input("arbitrary-window LGCP draw work overflows".into()))?;
    if draw_node_work > arguments.maximum_draw_node_work {
        return Err(BayesCliError::Input(format!(
            "arbitrary-window LGCP draw-node work exceeds maximum: {draw_node_work} > {}",
            arguments.maximum_draw_node_work
        )));
    }
    let (physical_covariance, physical_cholesky) = covariance_and_cholesky(
        &prepared.spec.quadrature,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
    )?;
    let neighbor_pairs = arbitrary_window_ipp_membership::physical_neighbor_pairs(
        &prepared.spec.quadrature,
        arguments.neighbor_radius_um,
    )?;
    if neighbor_pairs.is_empty() || neighbor_pairs.len() > arguments.maximum_neighbor_pairs {
        return Err(BayesCliError::Input(format!(
            "arbitrary-window LGCP neighbor pairs must be 1..={}: observed {}",
            arguments.maximum_neighbor_pairs,
            neighbor_pairs.len()
        )));
    }
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let source_worker_path = worker_directory.join("marklab_pymc_gridded_lgcp_worker.py");
    let worker_path = worker_directory.join("marklab_pymc_arbitrary_window_lgcp_worker.py");
    let lock = read(&lock_path)?;
    let source_worker = read(&source_worker_path)?;
    let worker = read(&worker_path)?;
    let source_request = source_request(
        &prepared.spec.quadrature,
        &membership.observed_node_counts,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.coefficient_prior_mean,
        arguments.coefficient_prior_sd,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
        sampling,
        arguments.prediction_replicates,
        arguments.prediction_seed,
        arguments.maximum_predictive_points,
        sha256_hex(&lock),
        sha256_hex(&source_worker),
        arguments.timeout_seconds,
    )?;
    let source_request_bytes = serde_json::to_vec(&source_request)?;
    let source_request_sha256 = sha256_hex(&source_request_bytes);
    let input = InputIdentity {
        events_digest: identity.events_digest,
        event_membership_digest: membership.digest,
        quadrature_digest: identity.quadrature_digest,
        window_logical_digest: identity.window.logical_digest.clone(),
    };
    let window = identity.window;
    let request = WorkerRequest {
        format: "marklab.pymc_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "pymc",
            version: PYMC_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        source_request_sha256,
        source_request,
        input: input.clone(),
        window,
        nodes: prepared
            .spec
            .quadrature
            .iter()
            .zip(membership.observed_node_counts)
            .map(|(node, count)| PhysicalNode {
                node_id: node.node_id.clone(),
                x_um: node.x_um,
                y_um: node.y_um,
                weight_um2: node.weight_um2,
                covariate: node.covariate,
                offset: node.offset,
                count,
            })
            .collect(),
        covariance_sha256: sha256_hex(&serde_json::to_vec(&physical_covariance)?),
        physical_covariance,
        physical_cholesky,
        neighbor_pairs,
        neighbor_radius_um: arguments.neighbor_radius_um,
        maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
        maximum_events: arguments.maximum_events,
        maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
        maximum_draw_node_work: arguments.maximum_draw_node_work,
        dense_factorization_work_units: nodes as u64 * nodes as u64 * nodes as u64,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedArbitraryWindowLgcpFit {
        request,
        request_bytes,
        request_sha256,
        timeout_seconds: arguments.timeout_seconds,
    })
}

pub(crate) fn execute(
    prepared: &PreparedArbitraryWindowLgcpFit,
) -> Result<FitResult, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes = run_worker(
        repository,
        "marklab_pymc_arbitrary_window_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: FitResult = serde_json::from_slice(&bytes)?;
    validate(&result, &prepared.request, &prepared.request_sha256)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn source_request(
    physical_nodes: &[marklab::ArbitraryWindowIppQuadratureNode],
    counts: &[u64],
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    prediction_replicates: u32,
    prediction_seed: u64,
    maximum_predictive_points: u64,
    environment_digest: String,
    worker_digest: String,
    timeout_seconds: u64,
) -> Result<GriddedLgcpFitWorkerRequest, BayesCliError> {
    let dimension = physical_nodes.len();
    let mut events = Vec::new();
    for (index, count) in counts.iter().enumerate() {
        for event in 0..*count {
            events.push(InhomogeneousPoissonEvent {
                event_id: format!("node-{index:04}-event-{event:08}"),
                x_um: index as f64 + 0.5,
                y_um: 0.5,
                covariate: physical_nodes[index].covariate,
                offset: physical_nodes[index].offset + physical_nodes[index].weight_um2.ln(),
            });
        }
    }
    GriddedLgcpFitWorkerRequest::new(
        GriddedLgcpFitSpec {
            construction: GriddedLgcpSpec {
                window: RectangularWindow {
                    xmin_um: 0.0,
                    ymin_um: 0.0,
                    xmax_um: dimension as f64,
                    ymax_um: 1.0,
                },
                grid_x: dimension as u32,
                grid_y: 1,
                events,
                grid: physical_nodes
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
                field_amplitude,
                field_length_scale_um,
                jitter,
            },
        },
        sampling,
        environment_digest,
        worker_digest,
        timeout_seconds,
    )
    .and_then(|request| {
        request.with_prediction(
            prediction_replicates,
            prediction_seed,
            maximum_predictive_points,
        )
    })
    .map_err(BayesCliError::Model)
}

fn covariance_and_cholesky(
    nodes: &[marklab::ArbitraryWindowIppQuadratureNode],
    amplitude: f64,
    length_scale_um: f64,
    jitter: f64,
) -> Result<(Vec<f64>, Vec<f64>), BayesCliError> {
    if ![amplitude, length_scale_um, jitter]
        .into_iter()
        .all(f64::is_finite)
        || amplitude <= 0.0
        || length_scale_um <= 0.0
        || jitter <= 0.0
    {
        return Err(BayesCliError::Input(
            "arbitrary-window LGCP field controls must be positive and finite".into(),
        ));
    }
    let dimension = nodes.len();
    let mut covariance = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            let distance =
                (nodes[row].x_um - nodes[column].x_um).hypot(nodes[row].y_um - nodes[column].y_um);
            let scaled = 3.0_f64.sqrt() * distance / length_scale_um;
            let mut value = amplitude.powi(2) * (1.0 + scaled) * (-scaled).exp();
            if row == column {
                value += jitter;
            }
            if !value.is_finite() {
                return Err(BayesCliError::Input(
                    "arbitrary-window LGCP covariance is non-finite".into(),
                ));
            }
            covariance[row * dimension + column] = value;
        }
    }
    let mut lower = vec![0.0; covariance.len()];
    for row in 0..dimension {
        for column in 0..=row {
            let mut value = covariance[row * dimension + column];
            for inner in 0..column {
                value -= lower[row * dimension + inner] * lower[column * dimension + inner];
            }
            if row == column {
                if !value.is_finite() || value <= 0.0 {
                    return Err(BayesCliError::Input(
                        "arbitrary-window LGCP covariance is not positive definite".into(),
                    ));
                }
                lower[row * dimension + column] = value.sqrt();
            } else {
                lower[row * dimension + column] = value / lower[column * dimension + column];
            }
        }
    }
    Ok((covariance, lower))
}

fn validate(
    result: &FitResult,
    request: &WorkerRequest,
    request_sha256: &str,
) -> Result<(), BayesCliError> {
    let draws = u64::from(request.source_request.sampling.chains)
        * u64::from(request.source_request.sampling.draws_per_chain);
    let nodes_valid = result
        .nodes
        .iter()
        .zip(&request.nodes)
        .all(|(result, input)| {
            result.node_id == input.node_id
                && equal(result.x_um, input.x_um)
                && equal(result.y_um, input.y_um)
                && equal(result.weight_um2, input.weight_um2)
                && equal(result.covariate, input.covariate)
                && equal(result.offset, input.offset)
                && result.count == input.count
                && scalar_valid(&result.latent_effect, false)
                && scalar_valid(&result.intensity_per_um2, true)
                && scalar_valid(&result.expected_count, true)
                && scalar_valid(&result.pearson_residual, false)
        });
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= request.source_request.diagnostic_policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= request.source_request.diagnostic_policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= request.source_request.diagnostic_policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi
            >= request.source_request.diagnostic_policy.minimum_ebfmi
        && result.diagnostics.divergences
            <= request.source_request.diagnostic_policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits
            <= request
                .source_request
                .diagnostic_policy
                .maximum_tree_depth_hits;
    let spatial_summary_valid = |summary: &SpatialPpcSummary| {
        [
            summary.observed,
            summary.replicated_mean,
            summary.replicated_sd,
            summary.probability_replicated_at_least_observed,
        ]
        .into_iter()
        .all(f64::is_finite)
            && summary.observed >= 0.0
            && summary.replicated_mean >= 0.0
            && summary.replicated_sd >= 0.0
            && (0.0..=1.0).contains(&summary.probability_replicated_at_least_observed)
    };
    let spatial_ppc_valid = result
        .spatial_posterior_predictive
        .as_ref()
        .is_some_and(|spatial| {
            spatial.replicate_count == request.source_request.prediction.replicates
                && spatial.prediction_seed == request.source_request.prediction.seed
                && equal(spatial.neighbor_radius_um, request.neighbor_radius_um)
                && spatial.neighbor_pair_count == request.neighbor_pairs.len()
                && spatial_summary_valid(&spatial.node_density_variance)
                && spatial_summary_valid(&spatial.neighbor_density_mean_absolute_difference)
        });
    if result.format != "marklab.bayesian_arbitrary_window_lgcp_fit"
        || result.version != 1
        || result.backend.name != request.backend.name
        || result.backend.version != request.backend.version
        || result.backend.python_version != request.backend.python_version
        || result.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || result.backend.worker_sha256 != request.backend.worker_sha256
        || result.source_backend.name != request.source_request.backend.name
        || result.source_backend.version != request.source_request.backend.version
        || result.source_backend.python_version != request.source_request.backend.python_version
        || result.source_backend.environment_lock_sha256
            != request.source_request.backend.environment_lock_sha256
        || result.source_backend.worker_sha256 != request.source_request.backend.worker_sha256
        || result.input.events_digest != request.input.events_digest
        || result.input.event_membership_digest != request.input.event_membership_digest
        || result.input.quadrature_digest != request.input.quadrature_digest
        || result.input.window_logical_digest != request.input.window_logical_digest
        || result.window.logical_digest != request.window.logical_digest
        || !equal(result.window.area_um2, request.window.area_um2)
        || !equal(result.window.perimeter_um, request.window.perimeter_um)
        || result.window.bounds_um != request.window.bounds_um
        || result.window.component_count != request.window.component_count
        || result.window.hole_count != request.window.hole_count
        || result.window.ring_count != request.window.ring_count
        || result.window.vertex_count != request.window.vertex_count
        || result.observed_event_count
            != request
                .nodes
                .iter()
                .map(|node| node.count as usize)
                .sum::<usize>()
        || result.quadrature_node_count != request.nodes.len()
        || result.nodes.len() != request.nodes.len()
        || !equal(
            result.quadrature_weight_um2,
            request.nodes.iter().map(|node| node.weight_um2).sum(),
        )
        || result.sampling.chains != request.source_request.sampling.chains
        || result.sampling.tune_per_chain != request.source_request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != request.source_request.sampling.draws_per_chain
        || result.sampling.completed_draws != draws
        || (result.fit_state == FitState::Complete) != diagnostics_pass
        || !scalar_valid(&result.posterior.intercept, false)
        || !scalar_valid(&result.posterior.coefficient, false)
        || !nodes_valid
        || result.posterior_predictive.observed_total_count as usize != result.observed_event_count
        || ![
            result.posterior_predictive.replicated_total_mean,
            result.posterior_predictive.replicated_total_sd,
            result.posterior_predictive.replicated_zero_cells_mean,
        ]
        .into_iter()
        .all(f64::is_finite)
        || result.posterior_predictive.replicated_total_mean < 0.0
        || result.posterior_predictive.replicated_total_sd < 0.0
        || result.posterior_predictive.replicated_zero_cells_mean < 0.0
        || result.seed != request.source_request.sampling.seed
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != request.source_request_sha256
        || result.covariance_sha256 != request.covariance_sha256
        || result.draw_node_work != draws * request.nodes.len() as u64
        || result.draw_node_work > result.maximum_draw_node_work
        || result.maximum_draw_node_work != request.maximum_draw_node_work
        || result.maximum_events != request.maximum_events
        || result.maximum_quadrature_nodes != request.maximum_quadrature_nodes
        || result.maximum_predictive_points
            != request.source_request.prediction.maximum_total_points
        || result.maximum_neighbor_pairs != request.maximum_neighbor_pairs
        || result.dense_covariance_elements != request.nodes.len() * request.nodes.len()
        || result.dense_factorization_work_units != request.dense_factorization_work_units
        || result.statistical_unit != "one_observed_point_pattern"
        || result.null_model != "none_descriptive_latent_intensity_model"
        || result.assumptions
            != [
                "event_to_weighted_node_membership_is_exact_and_complete",
                "quadrature_weights_partition_the_exact_window_area",
                "matern_amplitude_and_length_scale_are_fixed_not_inferred",
                "weighted_nodes_are_a_recorded_coarse_window_approximation",
                "latent_intensity_is_not_point_interaction_or_attraction",
            ]
        || result.finite_result_policy != "nonconverged_is_diagnostic_only_reject_non_finite"
        || result.model.window != "exact_multipolygon"
        || result.model.kernel != "fixed_matern_3_2_on_physical_quadrature_representatives"
        || !equal(
            result.model.field_amplitude,
            request.source_request.model.construction.field_amplitude,
        )
        || !equal(
            result.model.field_length_scale_um,
            request
                .source_request
                .model
                .construction
                .field_length_scale_um,
        )
        || !equal(
            result.model.jitter,
            request.source_request.model.construction.jitter,
        )
        || !equal(
            result.model.intercept_prior_mean,
            request
                .source_request
                .model
                .construction
                .intercept_prior_mean,
        )
        || !equal(
            result.model.intercept_prior_sd,
            request.source_request.model.construction.intercept_prior_sd,
        )
        || !equal(
            result.model.coefficient_prior_mean,
            request
                .source_request
                .model
                .construction
                .coefficient_prior_mean,
        )
        || !equal(
            result.model.coefficient_prior_sd,
            request
                .source_request
                .model
                .construction
                .coefficient_prior_sd,
        )
        || (result.fit_state == FitState::Complete) != spatial_ppc_valid
        || (result.fit_state != FitState::Complete && result.spatial_posterior_predictive.is_some())
        || (result.fit_state == FitState::Complete
            && result.claim_status != "experimental_single_pattern_latent_field")
        || (result.fit_state != FitState::Complete
            && result.claim_status != "diagnostic_only_nonconverged")
    {
        return Err(BayesCliError::Backend(
            "arbitrary-window LGCP identity, diagnostics, or physical result differs".into(),
        ));
    }
    Ok(())
}

impl FitResult {
    pub(crate) fn validate_for(
        &self,
        prepared: &PreparedArbitraryWindowLgcpFit,
    ) -> Result<(), BayesCliError> {
        validate(self, &prepared.request, &prepared.request_sha256)
    }
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
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
        && value.sd >= 0.0
        && value.interval_lower <= value.interval_upper
        && (!positive || (value.mean > 0.0 && value.interval_lower >= 0.0))
}

fn equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}
