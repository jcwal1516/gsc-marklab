use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_ipp_fit::{self, PreparedArbitraryWindowIppFit},
    arbitrary_window_ipp_membership, publish_json, run_worker, BayesCliError,
};

const PYMC_VERSION: &str = "6.3.0";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct SpatialPpcCli {
    #[command(subcommand)]
    command: SpatialPpcTopLevel,
}

#[derive(Debug, Subcommand)]
enum SpatialPpcTopLevel {
    Bayes {
        #[command(subcommand)]
        command: SpatialPpcCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SpatialPpcCommand {
    ArbitraryWindowIppSpatialPpc(Box<Arguments>),
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
    prediction_seed: u64,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SpatialPpcRequest {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    pub(super) source_request_sha256: String,
    source_request: marklab_bayes::InhomogeneousPoissonFitWorkerRequest,
    input: SpatialPpcInputIdentity,
    observed_node_counts: Vec<u64>,
    node_weights_um2: Vec<f64>,
    neighbor_pairs: Vec<[usize; 2]>,
    neighbor_radius_um: f64,
    prediction_seed: u64,
    maximum_predictive_work: u64,
    maximum_neighbor_pairs: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpatialPpcInputIdentity {
    events_digest: String,
    event_membership_digest: String,
    quadrature_digest: String,
    window_logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialPpcSummary {
    observed: f64,
    replicated_mean: f64,
    replicated_sd: f64,
    probability_replicated_at_least_observed: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialPpcSummaries {
    node_density_variance: SpatialPpcSummary,
    neighbor_density_mean_absolute_difference: SpatialPpcSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpatialPpcResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    input: SpatialPpcInputIdentity,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    diagnostics: NormalMeanDiagnostics,
    observed_event_count: usize,
    quadrature_node_count: usize,
    neighbor_radius_um: f64,
    neighbor_pair_count: usize,
    posterior_predictive_replicates: u64,
    prediction_seed: u64,
    total_predictive_work: u64,
    maximum_predictive_work: u64,
    maximum_neighbor_pairs: usize,
    summaries: SpatialPpcSummaries,
    statistical_unit: String,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
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

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let SpatialPpcTopLevel::Bayes { command } =
        SpatialPpcCli::parse_from(std::env::args_os()).command;
    let SpatialPpcCommand::ArbitraryWindowIppSpatialPpc(arguments) = command;
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

pub(crate) fn prepare(
    parameters: SpatialPpcParameters,
) -> Result<PreparedSpatialPpc, BayesCliError> {
    if !parameters.neighbor_radius_um.is_finite()
        || parameters.neighbor_radius_um <= 0.0
        || parameters.maximum_neighbor_pairs == 0
        || parameters.maximum_neighbor_pairs > 1_000_000
    {
        return Err(BayesCliError::Input(
            "spatial PPC neighbor radius or pair ceiling is invalid".into(),
        ));
    }
    let prepared = arbitrary_window_ipp_fit::prepare(
        parameters.events,
        parameters.quadrature,
        parameters.window,
        parameters.intercept_prior_mean,
        parameters.intercept_prior_sd,
        parameters.coefficient_prior_mean,
        parameters.coefficient_prior_sd,
        parameters.sampling,
        parameters.maximum_events,
        parameters.maximum_quadrature_nodes,
        parameters.maximum_draw_node_work,
        parameters.timeout_seconds,
    )?;
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
        return Err(BayesCliError::Input(format!(
            "spatial PPC physical neighbor pairs must be 1..={}: observed {}",
            parameters.maximum_neighbor_pairs,
            neighbor_pairs.len()
        )));
    }
    let posterior_draws = u64::from(prepared.request.sampling.chains)
        .checked_mul(u64::from(prepared.request.sampling.draws_per_chain))
        .ok_or_else(|| BayesCliError::Input("spatial PPC draw count overflows".into()))?;
    let maximum_predictive_work = posterior_draws
        .checked_mul(
            (parameters.maximum_quadrature_nodes + parameters.maximum_neighbor_pairs) as u64,
        )
        .ok_or_else(|| BayesCliError::Input("spatial PPC work ceiling overflows".into()))?;
    if maximum_predictive_work > 100_000_000 {
        return Err(BayesCliError::Input(
            "spatial PPC predictive work ceiling exceeds 100000000".into(),
        ));
    }
    let input = SpatialPpcInputIdentity {
        events_digest: prepared.input_identity.events_digest.clone(),
        event_membership_digest: membership.digest,
        quadrature_digest: prepared.input_identity.quadrature_digest.clone(),
        window_logical_digest: prepared.input_identity.window_logical_digest.clone(),
    };
    let repository = &marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_pymc_arbitrary_window_ipp_spatial_ppc_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = SpatialPpcRequest {
        format: "marklab.pymc_arbitrary_window_ipp_spatial_ppc_request",
        version: 1,
        backend: BackendContract {
            name: "pymc",
            version: PYMC_VERSION,
            python_version: "3.12",
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

pub(crate) fn execute(prepared: &PreparedSpatialPpc) -> Result<SpatialPpcResult, BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
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
) -> Result<(), BayesCliError> {
    let request = &prepared.request;
    let source_fit = &prepared.source_fit;
    let draws = u64::from(source_fit.request.sampling.chains)
        * u64::from(source_fit.request.sampling.draws_per_chain);
    let expected_work =
        draws * (source_fit.input.spec.quadrature.len() + request.neighbor_pairs.len()) as u64;
    let summary_valid = |summary: &SpatialPpcSummary| {
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
    let diagnostics_finite = [
        result.diagnostics.r_hat,
        result.diagnostics.ess_bulk,
        result.diagnostics.ess_tail,
        result.diagnostics.mcse_mean,
        result.diagnostics.mcse_sd,
        result.diagnostics.minimum_ebfmi,
    ]
    .into_iter()
    .all(f64::is_finite);
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && diagnostics_finite
        && result.diagnostics.r_hat <= source_fit.request.diagnostic_policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= source_fit.request.diagnostic_policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= source_fit.request.diagnostic_policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= source_fit.request.diagnostic_policy.minimum_ebfmi
        && result.diagnostics.divergences
            <= source_fit.request.diagnostic_policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits
            <= source_fit.request.diagnostic_policy.maximum_tree_depth_hits;
    if result.format != "marklab.arbitrary_window_ipp_spatial_ppc"
        || result.version != 1
        || result.backend.name != request.backend.name
        || result.backend.version != request.backend.version
        || result.backend.python_version != request.backend.python_version
        || result.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || result.backend.worker_sha256 != request.backend.worker_sha256
        || result.input.events_digest != request.input.events_digest
        || result.input.event_membership_digest != request.input.event_membership_digest
        || result.input.quadrature_digest != request.input.quadrature_digest
        || result.input.window_logical_digest != request.input.window_logical_digest
        || result.request_sha256 != prepared.request_sha256
        || result.source_request_sha256 != source_fit.request_sha256
        || result.sampling.chains != source_fit.request.sampling.chains
        || result.sampling.tune_per_chain != source_fit.request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != source_fit.request.sampling.draws_per_chain
        || result.sampling.completed_draws != draws
        || (result.fit_state == FitState::Complete) != diagnostics_pass
        || result.observed_event_count != source_fit.input.spec.events.len()
        || result.quadrature_node_count != source_fit.input.spec.quadrature.len()
        || !equal(result.neighbor_radius_um, request.neighbor_radius_um)
        || result.neighbor_pair_count != request.neighbor_pairs.len()
        || result.posterior_predictive_replicates != draws
        || result.prediction_seed != request.prediction_seed
        || result.total_predictive_work != expected_work
        || result.total_predictive_work > result.maximum_predictive_work
        || result.maximum_predictive_work != request.maximum_predictive_work
        || result.maximum_neighbor_pairs != request.maximum_neighbor_pairs
        || !summary_valid(&result.summaries.node_density_variance)
        || !summary_valid(&result.summaries.neighbor_density_mean_absolute_difference)
        || result.statistical_unit != "one_observed_point_pattern"
        || result.null_model
            != "fitted_log_linear_inhomogeneous_poisson_process_conditional_on_fixed_covariate"
        || result.assumptions
            != [
                "event_to_quadrature_membership_is_exact_and_complete",
                "quadrature_weights_partition_the_exact_window_area",
                "neighbor_pairs_use_fixed_physical_representative_distance",
                "node_density_is_count_per_square_micrometer",
            ]
        || result.finite_result_policy != "reject_non_finite_nonconverged_is_diagnostic_only"
        || (result.fit_state == FitState::Complete
            && result.claim_status != "experimental_single_pattern_spatial_ppc")
        || (result.fit_state != FitState::Complete
            && result.claim_status != "diagnostic_only_nonconverged")
    {
        return Err(BayesCliError::Backend(
            "arbitrary-window IPP spatial PPC identity, bounds, or result differs".into(),
        ));
    }
    Ok(())
}

fn equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12 * left.abs().max(right.abs()).max(1.0)
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<SpatialPpcCli>(|| run_cli().map_err(super::into_marklab_error))
}
