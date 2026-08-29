use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, run_worker, BayesCliError};

const PYMC_VERSION: &str = "6.3.0";
const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: Top,
}

#[derive(Debug, Subcommand)]
enum Top {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    FitConditionalMultitypeMark(Box<Args>),
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) reference_type: String,
    #[arg(long)]
    pub(crate) radius_um: f64,
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    #[arg(long)]
    pub(crate) interaction_prior_sd: f64,
    #[arg(long)]
    pub(crate) chains: u32,
    #[arg(long)]
    pub(crate) tune: u32,
    #[arg(long)]
    pub(crate) draws: u32,
    #[arg(long)]
    pub(crate) target_accept: f64,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) maximum_points: usize,
    #[arg(long)]
    pub(crate) maximum_types: usize,
    #[arg(long)]
    pub(crate) maximum_neighbor_visits: u64,
    #[arg(long)]
    pub(crate) maximum_edges: usize,
    #[arg(long)]
    pub(crate) maximum_draw_parameter_work: u64,
    #[arg(long)]
    pub(crate) maximum_working_bytes: usize,
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CsvPoint {
    #[serde(alias = "cell_id")]
    point_id: String,
    x_um: f64,
    y_um: f64,
    #[serde(alias = "histologic_compartment")]
    type_id: String,
}

#[derive(Debug, Serialize)]
struct PointRow {
    point_id: String,
    type_index: usize,
    neighbor_counts: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct Edge {
    left: usize,
    right: usize,
}

#[derive(Debug, Serialize)]
struct Priors {
    intercept_sd: f64,
    interaction_sd: f64,
}

#[derive(Debug, Serialize)]
struct DiagnosticPolicy {
    maximum_r_hat: f64,
    minimum_bulk_ess: f64,
    minimum_tail_ess: f64,
    minimum_ebfmi: f64,
    maximum_divergences: u64,
    maximum_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    maximum_points: usize,
    maximum_types: usize,
    maximum_neighbor_visits: u64,
    neighbor_visits: u64,
    maximum_edges: usize,
    maximum_draw_parameter_work: u64,
    draw_parameter_work: u64,
    maximum_working_bytes: usize,
    estimated_working_bytes: usize,
    memory_scope: String,
    maximum_output_bytes: usize,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
}

#[derive(Debug, Serialize)]
struct WorkerRequest {
    format: String,
    version: u32,
    backend: BackendContract,
    input_sha256: String,
    type_ids: Vec<String>,
    reference_type: String,
    radius_um: f64,
    points: Vec<PointRow>,
    edges: Vec<Edge>,
    priors: Priors,
    sampling: NutsSamplingSpec,
    diagnostic_policy: DiagnosticPolicy,
    resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TypeSummary {
    type_id: String,
    intercept: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PotentialSummary {
    type_a: String,
    type_b: String,
    potential: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AffinitySummary {
    type_a: String,
    type_b: String,
    contrast: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Comparison {
    independent_label_composite_log_score: f64,
    spatial_composite_log_score: SarScalarSummary,
    mean_composite_log_score_improvement: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PosteriorPredictive {
    mode: String,
    observed_same_type_edges: u64,
    expected_same_type_edges: SarScalarSummary,
    observed_type_counts: Vec<u64>,
    expected_type_counts: Vec<SarScalarSummary>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    type_intercepts: Vec<TypeSummary>,
    pair_potentials: Vec<PotentialSummary>,
    pair_affinity_contrasts: Vec<AffinitySummary>,
    comparison: Comparison,
    posterior_predictive: PosteriorPredictive,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    radius_um: f64,
    type_ids: Vec<String>,
    reference_type: String,
    point_count: usize,
    type_count: usize,
    edge_count: usize,
    type_intercepts: Vec<TypeSummary>,
    pair_potentials: Vec<PotentialSummary>,
    pair_affinity_contrasts: Vec<AffinitySummary>,
    comparison: Comparison,
    posterior_predictive: PosteriorPredictive,
    diagnostics: NormalMeanDiagnostics,
    resources: Resources,
    statistical_unit: String,
    null_model: String,
    assumptions: [String; 4],
    finite_result_policy: String,
    claim_status: String,
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

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::FitConditionalMultitypeMark(args) = command;
    let output_path = args.out.clone();
    publish_json(&output_path, &execute(*args)?)
}

pub(crate) fn execute(args: Args) -> Result<Output, BayesCliError> {
    execute_prepared(&prepare(args)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, BayesCliError> {
    validate_controls(&args)?;
    let metadata = fs::metadata(&args.input).map_err(|source| BayesCliError::Io {
        path: args.input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "conditional multitype input file or size is invalid".into(),
        ));
    }
    let input_bytes = fs::read(&args.input).map_err(|source| BayesCliError::Io {
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
        return Err(BayesCliError::Input(
            "conditional multitype CSV headers differ from exact contract".into(),
        ));
    }
    let mut points = reader
        .deserialize::<CsvPoint>()
        .collect::<Result<Vec<_>, _>>()?;
    if !(6..=args.maximum_points).contains(&points.len()) {
        return Err(BayesCliError::Input(
            "conditional multitype point count is invalid".into(),
        ));
    }
    points.sort_by(|left, right| left.point_id.cmp(&right.point_id));
    validate_points(&points)?;
    let mut type_ids = points
        .iter()
        .map(|row| row.type_id.clone())
        .collect::<Vec<_>>();
    type_ids.sort();
    type_ids.dedup();
    if !(3..=args.maximum_types.min(8)).contains(&type_ids.len())
        || !type_ids.contains(&args.reference_type)
    {
        return Err(BayesCliError::Input(
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
        .ok_or_else(|| {
            BayesCliError::Input("conditional multitype neighbor work overflows".into())
        })?;
    if visits > args.maximum_neighbor_visits {
        return Err(BayesCliError::Input(format!(
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
                return Err(BayesCliError::Input(
                    "neighbor distance is nonfinite".into(),
                ));
            }
            if distance <= args.radius_um {
                if edges.len() == args.maximum_edges {
                    return Err(BayesCliError::Input(
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
        return Err(BayesCliError::Input(
            "conditional multitype graph has no radius edges".into(),
        ));
    }
    let parameter_count = type_ids.len() - 1 + type_ids.len() * (type_ids.len() + 1) / 2 - 1;
    let completed_draws = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .ok_or_else(|| BayesCliError::Input("completed draws overflow".into()))?;
    let draw_parameter_work = completed_draws
        .checked_mul(parameter_count as u64)
        .ok_or_else(|| BayesCliError::Input("draw-parameter work overflows".into()))?;
    if draw_parameter_work > args.maximum_draw_parameter_work {
        return Err(BayesCliError::Input(
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
        .ok_or_else(|| {
            BayesCliError::Input("conditional multitype memory estimate overflows".into())
        })?;
    if estimated_working_bytes > args.maximum_working_bytes {
        return Err(BayesCliError::Input(
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
        return Err(BayesCliError::Input(
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

pub(crate) fn execute_prepared(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
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
    )?;
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
        statistical_unit: "one_fixed_location_pattern".into(),
        null_model: "fixed_location_independent_categorical_labels_intercept_only".into(),
        assumptions: [
            "cell_locations_and_radius_graph_are_conditioned_on".into(),
            "symmetric_pair_potentials_use_reference_reference_zero_gauge".into(),
            "composite_conditional_likelihood_is_not_a_normalized_joint_likelihood".into(),
            "one_pattern_does_not_support_patient_population_inference".into(),
        ],
        finite_result_policy:
            "all_posterior_diagnostics_summaries_scores_and_predictions_must_be_finite".into(),
        claim_status: "experimental_conditional_mark_pseudolikelihood".into(),
    })
}

pub(crate) fn backend_contract() -> Result<BackendContract, BayesCliError> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workers/python");
    Ok(BackendContract {
        name: "pymc",
        version: PYMC_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(
            &directory.join("marklab_pymc_conditional_multitype_mark_worker.py"),
        )?),
    })
}

impl Output {
    pub(crate) fn validate_for(
        &self,
        input_sha256: &str,
        reference_type: &str,
        radius_um: f64,
        backend: &BackendContract,
    ) -> Result<(), BayesCliError> {
        let complete = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= 1.01
            && self.diagnostics.ess_bulk >= 400.0
            && self.diagnostics.ess_tail >= 400.0
            && self.diagnostics.minimum_ebfmi >= 0.2
            && self.diagnostics.divergences == 0
            && self.diagnostics.max_tree_depth_hits == 0;
        let expected_potentials = self.type_count * (self.type_count + 1) / 2;
        let expected_affinities = self.type_count * (self.type_count - 1) / 2;
        if self.format != "marklab.conditional_multitype_mark_fit"
            || self.version != 1
            || !backend_matches(&self.backend, backend)
            || self.input_sha256 != input_sha256
            || self.reference_type != reference_type
            || self.type_ids.first().map(String::as_str) != Some(reference_type)
            || self.radius_um.to_bits() != radius_um.to_bits()
            || self.point_count < 6
            || self.type_count != self.type_ids.len()
            || self.type_intercepts.len() != self.type_count
            || self.pair_potentials.len() != expected_potentials
            || self.pair_affinity_contrasts.len() != expected_affinities
            || self.posterior_predictive.mode != "one_step_conditionals_given_observed_neighbors"
            || self.posterior_predictive.observed_type_counts.len() != self.type_count
            || self.posterior_predictive.expected_type_counts.len() != self.type_count
            || (self.fit_state == FitState::Complete) != complete
        {
            return Err(BayesCliError::Backend(
                "conditional multitype durable result differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_controls(args: &Args) -> Result<(), BayesCliError> {
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
        || !(1..=3_600).contains(&args.timeout_seconds)
    {
        return Err(BayesCliError::Input(
            "conditional multitype controls are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_points(points: &[CsvPoint]) -> Result<(), BayesCliError> {
    let mut ids = HashSet::new();
    let mut coordinates = HashSet::new();
    for point in points {
        if point.point_id.is_empty()
            || point.point_id.trim() != point.point_id
            || point.type_id.is_empty()
            || point.type_id.trim() != point.type_id
            || ![point.x_um, point.y_um].into_iter().all(f64::is_finite)
            || !ids.insert(point.point_id.as_str())
            || !coordinates.insert((point.x_um.to_bits(), point.y_um.to_bits()))
        {
            return Err(BayesCliError::Input(
                "conditional multitype points require unique exact IDs/coordinates and finite values"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    backend: &BackendContract,
    request_sha: &str,
) -> Result<(), BayesCliError> {
    let summary_valid = |summary: &SarScalarSummary| {
        [
            summary.mean,
            summary.sd,
            summary.interval_lower,
            summary.interval_upper,
        ]
        .into_iter()
        .all(f64::is_finite)
            && summary.sd >= 0.0
            && summary.interval_lower <= summary.interval_upper
    };
    let diagnostics = &result.diagnostics;
    let complete = diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
        && diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
        && diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
        && diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= request.diagnostic_policy.maximum_tree_depth_hits;
    let expected_potentials = request.type_ids.len() * (request.type_ids.len() + 1) / 2;
    let expected_affinities = request.type_ids.len() * (request.type_ids.len() - 1) / 2;
    let summaries_valid = result
        .type_intercepts
        .iter()
        .all(|row| summary_valid(&row.intercept))
        && result
            .pair_potentials
            .iter()
            .all(|row| summary_valid(&row.potential))
        && result
            .pair_affinity_contrasts
            .iter()
            .all(|row| summary_valid(&row.contrast))
        && summary_valid(&result.comparison.spatial_composite_log_score)
        && summary_valid(&result.posterior_predictive.expected_same_type_edges)
        && result
            .posterior_predictive
            .expected_type_counts
            .iter()
            .all(summary_valid);
    let finite = [
        result.comparison.independent_label_composite_log_score,
        result.comparison.mean_composite_log_score_improvement,
    ]
    .into_iter()
    .all(f64::is_finite);
    if result.format != "marklab.pymc_conditional_multitype_mark_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha
        || result.type_intercepts.len() != request.type_ids.len()
        || result.pair_potentials.len() != expected_potentials
        || result.pair_affinity_contrasts.len() != expected_affinities
        || result.posterior_predictive.observed_type_counts.len() != request.type_ids.len()
        || result.posterior_predictive.expected_type_counts.len() != request.type_ids.len()
        || result.posterior_predictive.mode != "one_step_conditionals_given_observed_neighbors"
        || !summaries_valid
        || !finite
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "conditional multitype backend result differs".into(),
        ));
    }
    Ok(())
}

fn backend_matches(result: &WorkerBackend, expected: &BackendContract) -> bool {
    result.name == expected.name
        && result.version == expected.version
        && result.python_version == expected.python_version
        && result.environment_lock_sha256 == expected.environment_lock_sha256
        && result.worker_sha256 == expected.worker_sha256
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}
