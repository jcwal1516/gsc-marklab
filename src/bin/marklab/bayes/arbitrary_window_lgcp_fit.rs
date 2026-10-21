pub(crate) use marklab_bayes::protocol::arbitrary_window_lgcp_fit::{
    validate, FitResult, InputIdentity, PhysicalNode, WorkerRequest,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::json_sha256;
use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::GriddedLgcpFitSpec;
use marklab_bayes::GriddedLgcpFitWorkerRequest;
use marklab_bayes::GriddedLgcpSpec;
use marklab_bayes::InhomogeneousPoissonEvent;
use marklab_bayes::MidpointQuadratureValue;
use marklab_bayes::NutsSamplingSpec;
use marklab_bayes::RectangularWindow;
use marklab_numerics::dense;
use marklab_numerics::kernel::matern32;

use super::arbitrary_window_ipp;
use super::arbitrary_window_ipp_membership;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::PYMC_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(crate) events: PathBuf,
    /// Table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    pub(crate) event_membership: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(crate) quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    pub(crate) window: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(crate) coefficient_prior_sd: f64,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    pub(crate) field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(crate) field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(crate) jitter: f64,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    pub(crate) chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    pub(crate) tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    pub(crate) draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    pub(crate) target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(crate) seed: u64,
    /// Upper bound on events.
    #[arg(long)]
    pub(crate) maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    pub(crate) maximum_quadrature_nodes: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(crate) maximum_draw_node_work: u64,
    /// Number of posterior predictive simulations.
    #[arg(long)]
    pub(crate) prediction_replicates: u32,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    pub(crate) prediction_seed: u64,
    /// Upper bound on predictive points.
    #[arg(long)]
    pub(crate) maximum_predictive_points: u64,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    pub(crate) neighbor_radius_um: f64,
    /// Upper bound on neighbor pairs.
    #[arg(long)]
    pub(crate) maximum_neighbor_pairs: usize,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(crate) out: PathBuf,
}

pub(crate) struct PreparedArbitraryWindowLgcpFit {
    pub(crate) request: WorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) timeout_seconds: u64,
}

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    let output_path = arguments.out.clone();
    let prepared = prepare(*arguments)?;
    publish_json(&output_path, &execute(&prepared)?)
}

pub(crate) fn prepare(arguments: Arguments) -> Result<PreparedArbitraryWindowLgcpFit, CliError> {
    if !(4..=36).contains(&arguments.maximum_quadrature_nodes)
        || arguments.maximum_draw_node_work == 0
        || arguments.maximum_draw_node_work > 1_000_000
        || !arguments.neighbor_radius_um.is_finite()
        || arguments.neighbor_radius_um <= 0.0
        || arguments.maximum_neighbor_pairs == 0
        || arguments.maximum_neighbor_pairs > 1_000_000
    {
        return Err(CliError::Input(
            "arbitrary-window LGCP requires 4..36 nodes and bounded positive draw-node work".into(),
        ));
    }
    let maximum_work = arguments
        .maximum_events
        .checked_add(arguments.maximum_quadrature_nodes)
        .ok_or_else(|| CliError::Input("arbitrary-window LGCP work overflows".into()))?;
    let mut prepared = arbitrary_window_ipp::prepare(
        arguments.events.clone(),
        arguments.quadrature.clone(),
        arguments.window.clone(),
        0.0,
        0.0,
        marklab::spatial::ArbitraryWindowIppLimits {
            maximum_events: arguments.maximum_events,
            maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
            maximum_work,
            maximum_retained_bytes: 16 * 1024 * 1024,
        },
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
        .ok_or_else(|| CliError::Input("arbitrary-window LGCP draw work overflows".into()))?;
    if draw_node_work > arguments.maximum_draw_node_work {
        return Err(CliError::Input(format!(
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
        return Err(CliError::Input(format!(
            "arbitrary-window LGCP neighbor pairs must be 1..={}: observed {}",
            arguments.maximum_neighbor_pairs,
            neighbor_pairs.len()
        )));
    }
    let repository = &marklab::backend::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let source_worker_path = worker_directory.join("marklab_pymc_gridded_lgcp_worker.py");
    let worker_path = worker_directory.join("marklab_pymc_arbitrary_window_lgcp_worker.py");
    let lock = read_file(&lock_path)?;
    let source_worker = read_file(&source_worker_path)?;
    let worker = read_file(&worker_path)?;
    let source_request = source_request(
        &prepared.spec.quadrature,
        &membership.observed_node_counts,
        &arguments,
        sampling,
        sha256_hex(&lock),
        sha256_hex(&source_worker),
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
            python_version: PYTHON_VERSION,
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
        covariance_sha256: json_sha256(&physical_covariance)?,
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

pub(crate) fn execute(prepared: &PreparedArbitraryWindowLgcpFit) -> Result<FitResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let bytes = run_worker(
        repository,
        "marklab_pymc_arbitrary_window_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: FitResult = serde_json::from_slice(&bytes)?;
    validate(&result, &prepared.request, &prepared.request_sha256)
        .map_err(super::protocol_error)?;
    Ok(result)
}

fn source_request(
    physical_nodes: &[marklab::spatial::ArbitraryWindowIppQuadratureNode],
    counts: &[u64],
    arguments: &Arguments,
    sampling: NutsSamplingSpec,
    environment_digest: String,
    worker_digest: String,
) -> Result<GriddedLgcpFitWorkerRequest, CliError> {
    let intercept_prior_mean = arguments.intercept_prior_mean;
    let intercept_prior_sd = arguments.intercept_prior_sd;
    let coefficient_prior_mean = arguments.coefficient_prior_mean;
    let coefficient_prior_sd = arguments.coefficient_prior_sd;
    let field_amplitude = arguments.field_amplitude;
    let field_length_scale_um = arguments.field_length_scale_um;
    let jitter = arguments.jitter;
    let prediction_replicates = arguments.prediction_replicates;
    let prediction_seed = arguments.prediction_seed;
    let maximum_predictive_points = arguments.maximum_predictive_points;
    let timeout_seconds = arguments.timeout_seconds;

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
    .map_err(CliError::Model)
}

fn covariance_and_cholesky(
    nodes: &[marklab::spatial::ArbitraryWindowIppQuadratureNode],
    amplitude: f64,
    length_scale_um: f64,
    jitter: f64,
) -> Result<(Vec<f64>, Vec<f64>), CliError> {
    if ![amplitude, length_scale_um, jitter]
        .into_iter()
        .all(f64::is_finite)
        || amplitude <= 0.0
        || length_scale_um <= 0.0
        || jitter <= 0.0
    {
        return Err(CliError::Input(
            "arbitrary-window LGCP field controls must be positive and finite".into(),
        ));
    }
    let dimension = nodes.len();
    let mut covariance = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            let distance =
                (nodes[row].x_um - nodes[column].x_um).hypot(nodes[row].y_um - nodes[column].y_um);
            let mut value = matern32(distance, amplitude, length_scale_um);
            if row == column {
                value += jitter;
            }
            if !value.is_finite() {
                return Err(CliError::Input(
                    "arbitrary-window LGCP covariance is non-finite".into(),
                ));
            }
            covariance[row * dimension + column] = value;
        }
    }
    let lower = dense::cholesky(&covariance, dimension).ok_or_else(|| {
        CliError::Input("arbitrary-window LGCP covariance is not positive definite".into())
    })?;
    Ok((covariance, lower))
}

impl PreparedArbitraryWindowLgcpFit {
    pub(crate) fn validate_result(&self, result: &FitResult) -> Result<(), CliError> {
        result
            .validate_for(&self.request, &self.request_sha256)
            .map_err(super::protocol_error)
    }
}
