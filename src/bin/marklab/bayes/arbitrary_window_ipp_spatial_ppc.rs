pub(crate) use marklab_bayes::protocol::arbitrary_window_ipp_spatial_ppc::{
    SpatialPpcInputIdentity, SpatialPpcRequest, SpatialPpcResult,
};

use std::fs;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::arbitrary_window_ipp_fit;
use super::arbitrary_window_ipp_fit::PreparedArbitraryWindowIppFit;
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
    events: PathBuf,
    /// Table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    event_membership: PathBuf,
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
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    prediction_seed: u64,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    neighbor_radius_um: f64,
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on neighbor pairs.
    #[arg(long)]
    maximum_neighbor_pairs: usize,
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

pub(crate) struct SpatialPpcParameters {
    pub events: PathBuf,
    pub event_membership: PathBuf,
    pub quadrature: PathBuf,
    pub window: PathBuf,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_mean: f64,
    pub coefficient_prior_sd: f64,
    pub sampling: NutsSamplingSpec,
    pub prediction_seed: u64,
    pub neighbor_radius_um: f64,
    pub maximum_events: usize,
    pub maximum_quadrature_nodes: usize,
    pub maximum_neighbor_pairs: usize,
    pub maximum_draw_node_work: u64,
    pub timeout_seconds: u64,
}

pub(crate) struct PreparedSpatialPpc {
    pub(super) source_fit: PreparedArbitraryWindowIppFit,
    pub(super) request: SpatialPpcRequest,
    pub(super) request_bytes: Vec<u8>,
    pub(super) request_sha256: String,
    timeout_seconds: u64,
}

impl PreparedSpatialPpc {
    pub(crate) fn backend_contract(&self) -> &BackendContract {
        &self.request.backend
    }

    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    let output = arguments.out.clone();
    let prepared = prepare(SpatialPpcParameters {
        events: arguments.events,
        event_membership: arguments.event_membership,
        quadrature: arguments.quadrature,
        window: arguments.window,
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
        prediction_seed: arguments.prediction_seed,
        neighbor_radius_um: arguments.neighbor_radius_um,
        maximum_events: arguments.maximum_events,
        maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
        maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
        maximum_draw_node_work: arguments.maximum_draw_node_work,
        timeout_seconds: arguments.timeout_seconds,
    })?;
    let result = execute(&prepared)?;
    publish_json(&output, &result)
}

pub(crate) fn prepare(parameters: SpatialPpcParameters) -> Result<PreparedSpatialPpc, CliError> {
    if !parameters.neighbor_radius_um.is_finite()
        || parameters.neighbor_radius_um <= 0.0
        || parameters.maximum_neighbor_pairs == 0
        || parameters.maximum_neighbor_pairs > 1_000_000
    {
        return Err(CliError::Input(
            "spatial PPC neighbor radius or pair ceiling is invalid".into(),
        ));
    }
    let prepared =
        arbitrary_window_ipp_fit::prepare(crate::bayes::ArbitraryWindowIppFitParameters {
            events_path: parameters.events,
            quadrature_path: parameters.quadrature,
            window_path: parameters.window,
            intercept_prior_mean: parameters.intercept_prior_mean,
            intercept_prior_sd: parameters.intercept_prior_sd,
            coefficient_prior_mean: parameters.coefficient_prior_mean,
            coefficient_prior_sd: parameters.coefficient_prior_sd,
            sampling: parameters.sampling,
            maximum_events: parameters.maximum_events,
            maximum_quadrature_nodes: parameters.maximum_quadrature_nodes,
            maximum_draw_node_work: parameters.maximum_draw_node_work,
            timeout_seconds: parameters.timeout_seconds,
        })?;
    let membership = arbitrary_window_ipp_membership::prepare(
        &parameters.event_membership,
        &prepared.input.spec.events,
        &prepared.input.spec.quadrature,
    )?;
    let neighbor_pairs = arbitrary_window_ipp_membership::physical_neighbor_pairs(
        &prepared.input.spec.quadrature,
        parameters.neighbor_radius_um,
    )?;
    if neighbor_pairs.is_empty() || neighbor_pairs.len() > parameters.maximum_neighbor_pairs {
        return Err(CliError::Input(format!(
            "spatial PPC physical neighbor pairs must be 1..={}: observed {}",
            parameters.maximum_neighbor_pairs,
            neighbor_pairs.len()
        )));
    }
    let posterior_draws = u64::from(prepared.request.sampling.chains)
        .checked_mul(u64::from(prepared.request.sampling.draws_per_chain))
        .ok_or_else(|| CliError::Input("spatial PPC draw count overflows".into()))?;
    let maximum_predictive_work = posterior_draws
        .checked_mul(
            (parameters.maximum_quadrature_nodes + parameters.maximum_neighbor_pairs) as u64,
        )
        .ok_or_else(|| CliError::Input("spatial PPC work ceiling overflows".into()))?;
    if maximum_predictive_work > 100_000_000 {
        return Err(CliError::Input(
            "spatial PPC predictive work ceiling exceeds 100000000".into(),
        ));
    }
    let input = SpatialPpcInputIdentity {
        events_digest: prepared.input_identity.events_digest.clone(),
        event_membership_digest: membership.digest,
        quadrature_digest: prepared.input_identity.quadrature_digest.clone(),
        window_logical_digest: prepared.input_identity.window_logical_digest.clone(),
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_pymc_arbitrary_window_ipp_spatial_ppc_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = SpatialPpcRequest {
        format: "marklab.pymc_arbitrary_window_ipp_spatial_ppc_request",
        version: 1,
        backend: BackendContract {
            name: "pymc",
            version: PYMC_VERSION,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        source_request_sha256: prepared.request_sha256.clone(),
        source_request: prepared.request.clone(),
        input: input.clone(),
        observed_node_counts: membership.observed_node_counts,
        node_weights_um2: prepared
            .input
            .spec
            .quadrature
            .iter()
            .map(|node| node.weight_um2)
            .collect(),
        neighbor_pairs,
        neighbor_radius_um: parameters.neighbor_radius_um,
        prediction_seed: parameters.prediction_seed,
        maximum_predictive_work,
        maximum_neighbor_pairs: parameters.maximum_neighbor_pairs,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedSpatialPpc {
        source_fit: prepared,
        request,
        request_bytes,
        request_sha256,
        timeout_seconds: parameters.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &PreparedSpatialPpc) -> Result<SpatialPpcResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let bytes = run_worker(
        repository,
        "marklab_pymc_arbitrary_window_ipp_spatial_ppc_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: SpatialPpcResult = serde_json::from_slice(&bytes)?;
    validate(prepared, &result)?;
    Ok(result)
}

pub(crate) fn validate(
    prepared: &PreparedSpatialPpc,
    result: &SpatialPpcResult,
) -> Result<(), CliError> {
    marklab_bayes::protocol::arbitrary_window_ipp_spatial_ppc::validate(
        &prepared.request,
        &prepared.request_sha256,
        &prepared.source_fit.request,
        &prepared.source_fit.request_sha256,
        prepared.source_fit.input.spec.events.len(),
        prepared.source_fit.input.spec.quadrature.len(),
        result,
    )
    .map_err(super::protocol_error)
}
