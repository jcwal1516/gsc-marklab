use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_ipp_fit::{self, PreparedArbitraryWindowIppFit},
    publish_json, run_worker, BayesCliError,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MembershipRow {
    event_id: String,
    quadrature_node_id: String,
}

#[derive(Debug, Serialize)]
struct SpatialPpcRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    source_request_sha256: &'a str,
    source_request: &'a marklab_bayes::InhomogeneousPoissonFitWorkerRequest,
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
struct SpatialPpcInputIdentity {
    events_digest: String,
    event_membership_digest: String,
    quadrature_digest: String,
    window_logical_digest: String,
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
struct SpatialPpcSummaries {
    node_density_variance: SpatialPpcSummary,
    neighbor_density_mean_absolute_difference: SpatialPpcSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpatialPpcResult {
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

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let SpatialPpcTopLevel::Bayes { command } =
        SpatialPpcCli::parse_from(std::env::args_os()).command;
    let SpatialPpcCommand::ArbitraryWindowIppSpatialPpc(arguments) = command;
    if !arguments.neighbor_radius_um.is_finite()
        || arguments.neighbor_radius_um <= 0.0
        || arguments.maximum_neighbor_pairs == 0
        || arguments.maximum_neighbor_pairs > 1_000_000
    {
        return Err(BayesCliError::Input(
            "spatial PPC neighbor radius or pair ceiling is invalid".into(),
        ));
    }
    let membership_path = arguments.event_membership.clone();
    let prepared = arbitrary_window_ipp_fit::prepare(
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
    let membership = read_membership(&membership_path)?;
    let event_membership_digest = sha256_hex(&serde_json::to_vec(&membership)?);
    let observed_node_counts = observed_counts(&prepared, &membership)?;
    let neighbor_pairs = neighbor_pairs(&prepared, arguments.neighbor_radius_um)?;
    if neighbor_pairs.is_empty() || neighbor_pairs.len() > arguments.maximum_neighbor_pairs {
        return Err(BayesCliError::Input(format!(
            "spatial PPC physical neighbor pairs must be 1..={}: observed {}",
            arguments.maximum_neighbor_pairs,
            neighbor_pairs.len()
        )));
    }
    let posterior_draws = u64::from(arguments.chains)
        .checked_mul(u64::from(arguments.draws))
        .ok_or_else(|| BayesCliError::Input("spatial PPC draw count overflows".into()))?;
    let maximum_predictive_work = posterior_draws
        .checked_mul((arguments.maximum_quadrature_nodes + arguments.maximum_neighbor_pairs) as u64)
        .ok_or_else(|| BayesCliError::Input("spatial PPC work ceiling overflows".into()))?;
    if maximum_predictive_work > 100_000_000 {
        return Err(BayesCliError::Input(
            "spatial PPC predictive work ceiling exceeds 100000000".into(),
        ));
    }
    let input = SpatialPpcInputIdentity {
        events_digest: prepared.input_identity.events_digest.clone(),
        event_membership_digest,
        quadrature_digest: prepared.input_identity.quadrature_digest.clone(),
        window_logical_digest: prepared.input_identity.window_logical_digest.clone(),
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
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
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
        input: input.clone(),
        observed_node_counts,
        node_weights_um2: prepared
            .input
            .spec
            .quadrature
            .iter()
            .map(|node| node.weight_um2)
            .collect(),
        neighbor_pairs,
        neighbor_radius_um: arguments.neighbor_radius_um,
        prediction_seed: arguments.prediction_seed,
        maximum_predictive_work,
        maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_pymc_arbitrary_window_ipp_spatial_ppc_worker.py",
        &request_bytes,
        arguments.timeout_seconds,
    )?;
    let result: SpatialPpcResult = serde_json::from_slice(&bytes)?;
    validate(&result, &request, &request_sha256, &prepared)?;
    publish_json(&arguments.out, &result)
}

fn read_membership(path: &PathBuf) -> Result<Vec<MembershipRow>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_path(path)?;
    let rows = reader.deserialize().collect::<Result<Vec<_>, _>>()?;
    if rows.len() > 100_000 {
        return Err(BayesCliError::Input(
            "spatial PPC membership exceeds 100000 rows".into(),
        ));
    }
    Ok(rows)
}

fn observed_counts(
    prepared: &PreparedArbitraryWindowIppFit,
    membership: &[MembershipRow],
) -> Result<Vec<u64>, BayesCliError> {
    if membership.len() != prepared.input.spec.events.len() {
        return Err(BayesCliError::Input(
            "spatial PPC membership must contain every event exactly once".into(),
        ));
    }
    let node_indices = prepared
        .input
        .spec
        .quadrature
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut counts = vec![0_u64; node_indices.len()];
    for (event, row) in prepared.input.spec.events.iter().zip(membership) {
        if event.event_id != row.event_id {
            return Err(BayesCliError::Input(
                "spatial PPC membership event identities differ from canonical events".into(),
            ));
        }
        let index = node_indices
            .get(row.quadrature_node_id.as_str())
            .ok_or_else(|| {
                BayesCliError::Input(format!(
                    "spatial PPC event {} references absent quadrature node {}",
                    row.event_id, row.quadrature_node_id
                ))
            })?;
        counts[*index] += 1;
    }
    Ok(counts)
}

fn neighbor_pairs(
    prepared: &PreparedArbitraryWindowIppFit,
    radius_um: f64,
) -> Result<Vec<[usize; 2]>, BayesCliError> {
    let radius_squared = radius_um * radius_um;
    let nodes = &prepared.input.spec.quadrature;
    let mut pairs = Vec::new();
    for left in 0..nodes.len() {
        for right in (left + 1)..nodes.len() {
            let dx = nodes[left].x_um - nodes[right].x_um;
            let dy = nodes[left].y_um - nodes[right].y_um;
            let distance_squared = dx.mul_add(dx, dy * dy);
            if !distance_squared.is_finite() {
                return Err(BayesCliError::Input(
                    "spatial PPC neighbor distance is non-finite".into(),
                ));
            }
            if distance_squared <= radius_squared {
                pairs.push([left, right]);
            }
        }
    }
    Ok(pairs)
}

fn validate(
    result: &SpatialPpcResult,
    request: &SpatialPpcRequest<'_>,
    request_sha256: &str,
    prepared: &PreparedArbitraryWindowIppFit,
) -> Result<(), BayesCliError> {
    let draws = u64::from(prepared.request.sampling.chains)
        * u64::from(prepared.request.sampling.draws_per_chain);
    let expected_work =
        draws * (prepared.input.spec.quadrature.len() + request.neighbor_pairs.len()) as u64;
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
        && result.diagnostics.r_hat <= prepared.request.diagnostic_policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= prepared.request.diagnostic_policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= prepared.request.diagnostic_policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= prepared.request.diagnostic_policy.minimum_ebfmi
        && result.diagnostics.divergences <= prepared.request.diagnostic_policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits
            <= prepared.request.diagnostic_policy.maximum_tree_depth_hits;
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
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256
        || result.sampling.chains != prepared.request.sampling.chains
        || result.sampling.tune_per_chain != prepared.request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != prepared.request.sampling.draws_per_chain
        || result.sampling.completed_draws != draws
        || (result.fit_state == FitState::Complete) != diagnostics_pass
        || result.observed_event_count != prepared.input.spec.events.len()
        || result.quadrature_node_count != prepared.input.spec.quadrature.len()
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
