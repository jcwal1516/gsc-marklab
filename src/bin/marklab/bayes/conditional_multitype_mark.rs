pub(crate) use marklab_bayes::protocol::conditional_multitype_mark::{
    validate_points, validate_result, CsvPoint, DiagnosticPolicy, Edge, Output, PointRow, Priors,
    Resources, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use marklab_bayes::is_worker_timeout;
use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::PYMC_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    /// CSV of points: point_id or cell_id, x_um, y_um, and type_id or histologic_compartment.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Point-type label used as the model reference.
    #[arg(long)]
    pub(crate) reference_type: String,
    /// Spatial radius in micrometers.
    #[arg(long)]
    pub(crate) radius_um: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior scale for interaction.
    #[arg(long)]
    pub(crate) interaction_prior_sd: f64,
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
    /// Upper bound on points.
    #[arg(long)]
    pub(crate) maximum_points: usize,
    /// Upper bound on types.
    #[arg(long)]
    pub(crate) maximum_types: usize,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    pub(crate) maximum_neighbor_visits: u64,
    /// Upper bound on edges.
    #[arg(long)]
    pub(crate) maximum_edges: usize,
    /// Upper bound on draw parameter work.
    #[arg(long)]
    pub(crate) maximum_draw_parameter_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    pub(crate) maximum_working_bytes: usize,
    /// Upper bound on tree depth.
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(crate) out: PathBuf,
}

pub(crate) struct Prepared {
    backend: BackendContract,
    request: WorkerRequest,
    request_bytes: Vec<u8>,
    request_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }

    pub(crate) fn request_sha256(&self) -> &str {
        &self.request_sha256
    }
}

pub(super) fn run(args: Args) -> Result<(), CliError> {
    let output_path = args.out.clone();
    publish_json(&output_path, &execute(args)?)
}

pub(crate) fn execute(args: Args) -> Result<Output, CliError> {
    execute_prepared(&prepare(args)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, CliError> {
    validate_controls(&args)?;
    let metadata = fs::metadata(&args.input).map_err(|source| CliError::Io {
        path: args.input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "conditional multitype input file or size is invalid".into(),
        ));
    }
    let input_bytes = fs::read(&args.input).map_err(|source| CliError::Io {
        path: args.input.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input_bytes.as_slice());
    let headers = reader.headers()?.iter().collect::<Vec<_>>();
    let minimal = ["point_id", "x_um", "y_um", "type_id"];
    let admitted_cellvit = [
        "cell_id",
        "x_um",
        "y_um",
        "mark",
        "case_id",
        "timepoint",
        "protein",
        "valid_tumor",
        "valid_ihc",
        "slide_id",
        "histologic_compartment",
        "mark_probability",
    ];
    if headers != minimal && headers != admitted_cellvit {
        return Err(CliError::Input(
            "conditional multitype CSV headers differ from exact contract".into(),
        ));
    }
    let mut points = reader
        .deserialize::<CsvPoint>()
        .collect::<Result<Vec<_>, _>>()?;
    if !(6..=args.maximum_points).contains(&points.len()) {
        return Err(CliError::Input(
            "conditional multitype point count is invalid".into(),
        ));
    }
    points.sort_by(|left, right| left.point_id.cmp(&right.point_id));
    validate_points(&points).map_err(super::protocol_error)?;
    let mut type_ids = points
        .iter()
        .map(|row| row.type_id.clone())
        .collect::<Vec<_>>();
    type_ids.sort();
    type_ids.dedup();
    if !(3..=args.maximum_types.min(8)).contains(&type_ids.len())
        || !type_ids.contains(&args.reference_type)
    {
        return Err(CliError::Input(
            "conditional multitype types or reference are invalid".into(),
        ));
    }
    type_ids.retain(|value| value != &args.reference_type);
    type_ids.insert(0, args.reference_type.clone());
    let type_index = type_ids
        .iter()
        .enumerate()
        .map(|(index, value)| (value.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let visits = (points.len() as u64)
        .checked_mul(points.len().saturating_sub(1) as u64)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| CliError::Input("conditional multitype neighbor work overflows".into()))?;
    if visits > args.maximum_neighbor_visits {
        return Err(CliError::Input(format!(
            "conditional multitype neighbor visits exceed maximum: {visits} > {}",
            args.maximum_neighbor_visits
        )));
    }
    let mut counts = vec![vec![0_u32; type_ids.len()]; points.len()];
    let mut edges = Vec::new();
    for left in 0..points.len() {
        for right in left + 1..points.len() {
            let distance = (points[left].x_um - points[right].x_um)
                .hypot(points[left].y_um - points[right].y_um);
            if !distance.is_finite() {
                return Err(CliError::Input("neighbor distance is nonfinite".into()));
            }
            if distance <= args.radius_um {
                if edges.len() == args.maximum_edges {
                    return Err(CliError::Input(
                        "conditional multitype edge ceiling exceeded".into(),
                    ));
                }
                let left_type = type_index[points[left].type_id.as_str()];
                let right_type = type_index[points[right].type_id.as_str()];
                counts[left][right_type] += 1;
                counts[right][left_type] += 1;
                edges.push(Edge { left, right });
            }
        }
    }
    if edges.is_empty() {
        return Err(CliError::Input(
            "conditional multitype graph has no radius edges".into(),
        ));
    }
    let parameter_count = type_ids.len() - 1 + type_ids.len() * (type_ids.len() + 1) / 2 - 1;
    let completed_draws = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .ok_or_else(|| CliError::Input("completed draws overflow".into()))?;
    let draw_parameter_work = completed_draws
        .checked_mul(parameter_count as u64)
        .ok_or_else(|| CliError::Input("draw-parameter work overflows".into()))?;
    if draw_parameter_work > args.maximum_draw_parameter_work {
        return Err(CliError::Input(
            "conditional multitype draw-parameter work exceeds maximum".into(),
        ));
    }
    let estimated_working_bytes = points
        .len()
        .checked_mul(type_ids.len())
        .and_then(|value| value.checked_mul(16))
        .and_then(|value| value.checked_add(edges.len().checked_mul(16)?))
        .and_then(|value| {
            value.checked_add(
                parameter_count
                    .checked_mul(usize::try_from(completed_draws).ok()?)?
                    .checked_mul(8)?,
            )
        })
        .ok_or_else(|| CliError::Input("conditional multitype memory estimate overflows".into()))?;
    if estimated_working_bytes > args.maximum_working_bytes {
        return Err(CliError::Input(
            "conditional multitype working-byte estimate exceeds maximum".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let resources = Resources {
        maximum_points: args.maximum_points,
        maximum_types: args.maximum_types,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        neighbor_visits: visits,
        maximum_edges: args.maximum_edges,
        maximum_draw_parameter_work: args.maximum_draw_parameter_work,
        draw_parameter_work,
        maximum_working_bytes: args.maximum_working_bytes,
        estimated_working_bytes,
        memory_scope: "retained_request_graph_and_posterior_summaries_excluding_pinned_runtime_rss"
            .into(),
        maximum_output_bytes: 2 * 1_048_576,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
    };
    let backend = backend_contract()?;
    let rows = points
        .iter()
        .zip(counts)
        .map(|(point, neighbor_counts)| PointRow {
            point_id: point.point_id.clone(),
            type_index: type_index[point.type_id.as_str()],
            neighbor_counts,
        })
        .collect::<Vec<_>>();
    let request = WorkerRequest {
        format: "marklab.pymc_conditional_multitype_mark_request".into(),
        version: 1,
        backend: backend.clone(),
        input_sha256: sha256_hex(&input_bytes),
        type_ids: type_ids.clone(),
        reference_type: args.reference_type.clone(),
        radius_um: args.radius_um,
        points: rows,
        edges,
        priors: Priors {
            intercept_sd: args.intercept_prior_sd,
            interaction_sd: args.interaction_prior_sd,
        },
        sampling,
        diagnostic_policy: DiagnosticPolicy {
            maximum_r_hat: 1.01,
            minimum_bulk_ess: 400.0,
            minimum_tail_ess: 400.0,
            minimum_ebfmi: 0.2,
            maximum_divergences: 0,
            maximum_tree_depth_hits: 0,
        },
        resources,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    if request_bytes.len() > MAXIMUM_INPUT_BYTES as usize {
        return Err(CliError::Input(
            "conditional multitype worker request exceeds input ceiling".into(),
        ));
    }
    let request_sha = sha256_hex(&request_bytes);
    Ok(Prepared {
        backend,
        request,
        request_bytes,
        request_sha256: request_sha,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute_prepared(prepared: &Prepared) -> Result<Output, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let worker_bytes = run_worker(
        repository,
        "marklab_pymc_conditional_multitype_mark_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&worker_bytes)?;
    validate_result(
        &result,
        &prepared.request,
        &prepared.backend,
        &prepared.request_sha256,
    )
    .map_err(super::protocol_error)?;
    let point_count = prepared.request.points.len();
    let edge_count = prepared.request.edges.len();
    Ok(Output {
        format: "marklab.conditional_multitype_mark_fit".into(),
        version: 1,
        backend: result.backend,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        radius_um: prepared.request.radius_um,
        type_ids: prepared.request.type_ids.clone(),
        reference_type: prepared.request.reference_type.clone(),
        point_count,
        type_count: prepared.request.type_ids.len(),
        edge_count,
        type_intercepts: result.type_intercepts,
        pair_potentials: result.pair_potentials,
        pair_affinity_contrasts: result.pair_affinity_contrasts,
        comparison: result.comparison,
        posterior_predictive: result.posterior_predictive,
        diagnostics: result.diagnostics,
        resources: Resources {
            maximum_points: prepared.request.resources.maximum_points,
            maximum_types: prepared.request.resources.maximum_types,
            maximum_neighbor_visits: prepared.request.resources.maximum_neighbor_visits,
            neighbor_visits: prepared.request.resources.neighbor_visits,
            maximum_edges: prepared.request.resources.maximum_edges,
            maximum_draw_parameter_work: prepared.request.resources.maximum_draw_parameter_work,
            draw_parameter_work: prepared.request.resources.draw_parameter_work,
            maximum_working_bytes: prepared.request.resources.maximum_working_bytes,
            estimated_working_bytes: prepared.request.resources.estimated_working_bytes,
            memory_scope: prepared.request.resources.memory_scope.clone(),
            maximum_output_bytes: prepared.request.resources.maximum_output_bytes,
            maximum_tree_depth: prepared.request.resources.maximum_tree_depth,
            timeout_seconds: prepared.request.resources.timeout_seconds,
        },
        statistical_unit: StatisticalUnit::OneFixedLocationPattern,
        null_model: NullHypothesis::FixedLocationIndependentCategoricalLabelsInterceptOnly,
        assumptions: [
            "cell_locations_and_radius_graph_are_conditioned_on".into(),
            "symmetric_pair_potentials_use_reference_reference_zero_gauge".into(),
            "composite_conditional_likelihood_is_not_a_normalized_joint_likelihood".into(),
            "one_pattern_does_not_support_patient_population_inference".into(),
        ],
        finite_result_policy:
            "all_posterior_diagnostics_summaries_scores_and_predictions_must_be_finite".into(),
        claim_status: ClaimStatus::ExperimentalConditionalMarkPseudolikelihood,
    })
}

pub(crate) fn backend_contract() -> Result<BackendContract, CliError> {
    let directory = marklab::backend::python_backend_assets_root()?.join("workers/python");
    Ok(BackendContract {
        name: "pymc",
        version: PYMC_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(
            &directory.join("marklab_pymc_conditional_multitype_mark_worker.py"),
        )?),
    })
}

fn validate_controls(args: &Args) -> Result<(), CliError> {
    if ![
        args.radius_um,
        args.intercept_prior_sd,
        args.interaction_prior_sd,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value > 0.0)
        || !(3..=8).contains(&args.maximum_types)
        || args.maximum_points < 6
        || args.maximum_neighbor_visits == 0
        || args.maximum_edges == 0
        || args.maximum_draw_parameter_work == 0
        || args.maximum_working_bytes == 0
        || !(10..=14).contains(&args.maximum_tree_depth)
        || !is_worker_timeout(args.timeout_seconds)
    {
        return Err(CliError::Input(
            "conditional multitype controls are invalid".into(),
        ));
    }
    Ok(())
}
