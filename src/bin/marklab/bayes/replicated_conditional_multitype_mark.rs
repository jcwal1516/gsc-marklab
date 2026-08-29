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
const MAXIMUM_INPUT_BYTES: u64 = 64 * 1_048_576;

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
    FitReplicatedConditionalMultitypeMark(Box<Args>),
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) reference_group: String,
    #[arg(long)]
    pub(crate) reference_type: String,
    #[arg(long)]
    pub(crate) radius_um: f64,
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    #[arg(long)]
    pub(crate) interaction_prior_sd: f64,
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(crate) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(crate) pattern_sd_prior_scale: f64,
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
    pub(crate) maximum_patients: usize,
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvPoint {
    pattern_id: String,
    patient_id: String,
    group: String,
    point_id: String,
    x_um: f64,
    y_um: f64,
    type_id: String,
}

#[derive(Debug, Serialize)]
struct Patient {
    patient_id: String,
    group_index: usize,
}

#[derive(Debug, Serialize)]
struct Pattern {
    pattern_id: String,
    patient_index: usize,
    group_index: usize,
    point_count: usize,
    edge_count: usize,
}

#[derive(Debug, Serialize)]
struct PointRow {
    point_id: String,
    pattern_index: usize,
    patient_index: usize,
    group_index: usize,
    type_index: usize,
    neighbor_counts: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct Edge {
    left: usize,
    right: usize,
    pattern_index: usize,
}

#[derive(Debug, Serialize)]
struct Priors {
    intercept_sd: f64,
    interaction_sd: f64,
    group_effect_sd: f64,
    patient_sd_scale: f64,
    pattern_sd_scale: f64,
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
    maximum_patients: usize,
    maximum_patterns: usize,
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
    group_ids: Vec<String>,
    reference_group: String,
    type_ids: Vec<String>,
    reference_type: String,
    radius_um: f64,
    patients: Vec<Patient>,
    patterns: Vec<Pattern>,
    points: Vec<PointRow>,
    edges: Vec<Edge>,
    priors: Priors,
    sampling: NutsSamplingSpec,
    diagnostic_policy: DiagnosticPolicy,
    resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NamedScaleSummary {
    level: String,
    component: String,
    scale: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PairSummary {
    type_a: String,
    type_b: String,
    contrast: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatientPairSummary {
    patient_id: String,
    group: String,
    type_a: String,
    type_b: String,
    contrast: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatternCheck {
    pattern_id: String,
    observed_same_type_edges: u64,
    expected_same_type_edges: SarScalarSummary,
    observed_type_counts: Vec<u64>,
    expected_type_counts: Vec<SarScalarSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Comparison {
    independent_pattern_label_composite_log_score: f64,
    spatial_hierarchical_composite_log_score: SarScalarSummary,
    mean_composite_log_score_improvement: f64,
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
    baseline_affinities: Vec<PairSummary>,
    group_affinity_shifts: Vec<PairSummary>,
    patient_affinities: Vec<PatientPairSummary>,
    hierarchy_scales: Vec<NamedScaleSummary>,
    comparison: Comparison,
    pattern_checks: Vec<PatternCheck>,
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
    group_ids: Vec<String>,
    reference_group: String,
    type_ids: Vec<String>,
    reference_type: String,
    patient_count: usize,
    pattern_count: usize,
    point_count: usize,
    type_count: usize,
    edge_count: usize,
    baseline_affinities: Vec<PairSummary>,
    group_affinity_shifts: Vec<PairSummary>,
    patient_affinities: Vec<PatientPairSummary>,
    hierarchy_scales: Vec<NamedScaleSummary>,
    comparison: Comparison,
    pattern_checks: Vec<PatternCheck>,
    diagnostics: NormalMeanDiagnostics,
    resources: Resources,
    statistical_unit: String,
    null_model: String,
    assumptions: [String; 5],
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
    let Command::FitReplicatedConditionalMultitypeMark(args) = command;
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
            "replicated conditional-mark input file or size is invalid".into(),
        ));
    }
    let input_bytes = fs::read(&args.input).map_err(|source| BayesCliError::Io {
        path: args.input.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input_bytes.as_slice());
    if reader.headers()?.iter().collect::<Vec<_>>()
        != [
            "pattern_id",
            "patient_id",
            "group",
            "point_id",
            "x_um",
            "y_um",
            "type_id",
        ]
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark CSV headers differ".into(),
        ));
    }
    let mut points = reader
        .deserialize::<CsvPoint>()
        .collect::<Result<Vec<_>, _>>()?;
    if !(24..=args.maximum_points).contains(&points.len()) {
        return Err(BayesCliError::Input(
            "replicated conditional-mark point count is invalid".into(),
        ));
    }
    points.sort_by(|left, right| {
        (&left.pattern_id, &left.point_id).cmp(&(&right.pattern_id, &right.point_id))
    });
    validate_points(&points)?;

    let mut type_ids = points
        .iter()
        .map(|point| point.type_id.clone())
        .collect::<Vec<_>>();
    type_ids.sort();
    type_ids.dedup();
    if !(3..=args.maximum_types.min(8)).contains(&type_ids.len())
        || !type_ids.contains(&args.reference_type)
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark type identities are invalid".into(),
        ));
    }
    type_ids.retain(|value| value != &args.reference_type);
    type_ids.insert(0, args.reference_type.clone());
    let type_index = type_ids
        .iter()
        .enumerate()
        .map(|(index, value)| (value.as_str(), index))
        .collect::<BTreeMap<_, _>>();

    let mut patient_groups = BTreeMap::<String, String>::new();
    let mut pattern_owners = BTreeMap::<String, (String, String)>::new();
    for point in &points {
        match patient_groups.insert(point.patient_id.clone(), point.group.clone()) {
            Some(group) if group != point.group => {
                return Err(BayesCliError::Input(
                    "one patient belongs to multiple groups".into(),
                ));
            }
            _ => {}
        }
        match pattern_owners.insert(
            point.pattern_id.clone(),
            (point.patient_id.clone(), point.group.clone()),
        ) {
            Some(owner) if owner != (point.patient_id.clone(), point.group.clone()) => {
                return Err(BayesCliError::Input(
                    "one pattern belongs to multiple patients or groups".into(),
                ));
            }
            _ => {}
        }
    }
    if patient_groups.len() > args.maximum_patients || pattern_owners.len() > args.maximum_patterns
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark hierarchy exceeds its ceiling".into(),
        ));
    }
    let mut group_ids = patient_groups.values().cloned().collect::<Vec<_>>();
    group_ids.sort();
    group_ids.dedup();
    if group_ids.len() != 2 || !group_ids.contains(&args.reference_group) {
        return Err(BayesCliError::Input(
            "exactly two groups including the reference are required".into(),
        ));
    }
    group_ids.retain(|value| value != &args.reference_group);
    group_ids.insert(0, args.reference_group.clone());
    let group_index = group_ids
        .iter()
        .enumerate()
        .map(|(index, value)| (value.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut patient_ids = patient_groups.keys().cloned().collect::<Vec<_>>();
    patient_ids.sort_by(|left, right| {
        (group_index[patient_groups[left].as_str()], left)
            .cmp(&(group_index[patient_groups[right].as_str()], right))
    });
    let patient_index = patient_ids
        .iter()
        .enumerate()
        .map(|(index, value)| (value.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let group_patient_counts = group_ids
        .iter()
        .map(|group| {
            patient_groups
                .values()
                .filter(|value| *value == group)
                .count()
        })
        .collect::<Vec<_>>();
    if group_patient_counts.iter().any(|count| *count < 2) {
        return Err(BayesCliError::Input(
            "each group requires at least two independent patients".into(),
        ));
    }
    let pattern_ids = pattern_owners.keys().cloned().collect::<Vec<_>>();
    let pattern_index = pattern_ids
        .iter()
        .enumerate()
        .map(|(index, value)| (value.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let pattern_counts = patient_ids
        .iter()
        .map(|patient| {
            pattern_owners
                .values()
                .filter(|(owner, _)| owner == patient)
                .count()
        })
        .collect::<Vec<_>>();
    if pattern_counts.iter().any(|count| *count < 2) {
        return Err(BayesCliError::Input(
            "each patient requires at least two nested patterns".into(),
        ));
    }

    let mut counts = vec![vec![0_u32; type_ids.len()]; points.len()];
    let mut edges = Vec::new();
    let mut neighbor_visits = 0_u64;
    let mut patterns = Vec::with_capacity(pattern_ids.len());
    for pattern_id in &pattern_ids {
        let begin = points.partition_point(|point| point.pattern_id < *pattern_id);
        let end = points.partition_point(|point| point.pattern_id <= *pattern_id);
        let pattern_points = &points[begin..end];
        if pattern_points.len() < 6 {
            return Err(BayesCliError::Input(
                "every pattern requires at least six points".into(),
            ));
        }
        let mut observed = vec![0_usize; type_ids.len()];
        for point in pattern_points {
            observed[type_index[point.type_id.as_str()]] += 1;
        }
        if observed.iter().any(|count| *count < 2) {
            return Err(BayesCliError::Input(
                "every pattern requires at least two points of every type".into(),
            ));
        }
        let visits = (pattern_points.len() as u64)
            .checked_mul(pattern_points.len().saturating_sub(1) as u64)
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| BayesCliError::Input("neighbor work overflows".into()))?;
        neighbor_visits = neighbor_visits
            .checked_add(visits)
            .ok_or_else(|| BayesCliError::Input("neighbor work overflows".into()))?;
        let edge_begin = edges.len();
        for left in begin..end {
            for right in left + 1..end {
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
                            "replicated conditional-mark edge ceiling exceeded".into(),
                        ));
                    }
                    let left_type = type_index[points[left].type_id.as_str()];
                    let right_type = type_index[points[right].type_id.as_str()];
                    counts[left][right_type] += 1;
                    counts[right][left_type] += 1;
                    edges.push(Edge {
                        left,
                        right,
                        pattern_index: pattern_index[pattern_id.as_str()],
                    });
                }
            }
        }
        if edges.len() == edge_begin {
            return Err(BayesCliError::Input(
                "every replicated conditional-mark pattern requires radius edges".into(),
            ));
        }
        let (owner, group) = &pattern_owners[pattern_id];
        patterns.push(Pattern {
            pattern_id: pattern_id.clone(),
            patient_index: patient_index[owner.as_str()],
            group_index: group_index[group.as_str()],
            point_count: pattern_points.len(),
            edge_count: edges.len() - edge_begin,
        });
    }
    if neighbor_visits > args.maximum_neighbor_visits {
        return Err(BayesCliError::Input(format!(
            "replicated conditional-mark neighbor visits exceed maximum: {neighbor_visits} > {}",
            args.maximum_neighbor_visits
        )));
    }

    let patients = patient_ids
        .iter()
        .map(|patient_id| Patient {
            patient_id: patient_id.clone(),
            group_index: group_index[patient_groups[patient_id].as_str()],
        })
        .collect::<Vec<_>>();
    let rows = points
        .iter()
        .zip(counts)
        .map(|(point, neighbor_counts)| PointRow {
            point_id: point.point_id.clone(),
            pattern_index: pattern_index[point.pattern_id.as_str()],
            patient_index: patient_index[point.patient_id.as_str()],
            group_index: group_index[point.group.as_str()],
            type_index: type_index[point.type_id.as_str()],
            neighbor_counts,
        })
        .collect::<Vec<_>>();

    let free_intercepts = type_ids.len() - 1;
    let free_potentials = type_ids.len() * (type_ids.len() + 1) / 2 - 1;
    let free_per_level = free_intercepts + free_potentials;
    let parameter_count = 2_usize
        .checked_mul(free_per_level)
        .and_then(|value| value.checked_add((patients.len() + patterns.len()) * free_per_level))
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| BayesCliError::Input("parameter count overflows".into()))?;
    let completed_draws = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .ok_or_else(|| BayesCliError::Input("completed draws overflow".into()))?;
    let draw_parameter_work = completed_draws
        .checked_mul(parameter_count as u64)
        .ok_or_else(|| BayesCliError::Input("draw-parameter work overflows".into()))?;
    if draw_parameter_work > args.maximum_draw_parameter_work {
        return Err(BayesCliError::Input(
            "replicated conditional-mark draw-parameter work exceeds maximum".into(),
        ));
    }
    let estimated_working_bytes = rows
        .len()
        .checked_mul(type_ids.len())
        .and_then(|value| value.checked_mul(16))
        .and_then(|value| value.checked_add(edges.len().checked_mul(24)?))
        .and_then(|value| {
            value.checked_add(
                parameter_count
                    .checked_mul(usize::try_from(completed_draws).ok()?)?
                    .checked_mul(8)?,
            )
        })
        .ok_or_else(|| BayesCliError::Input("working-byte estimate overflows".into()))?;
    if estimated_working_bytes > args.maximum_working_bytes {
        return Err(BayesCliError::Input(
            "replicated conditional-mark working-byte estimate exceeds maximum".into(),
        ));
    }
    let resources = Resources {
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_points: args.maximum_points,
        maximum_types: args.maximum_types,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        neighbor_visits,
        maximum_edges: args.maximum_edges,
        maximum_draw_parameter_work: args.maximum_draw_parameter_work,
        draw_parameter_work,
        maximum_working_bytes: args.maximum_working_bytes,
        estimated_working_bytes,
        memory_scope:
            "retained_request_graph_and_posterior_parameters_excluding_pinned_runtime_rss".into(),
        maximum_output_bytes: 8 * 1_048_576,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
    };
    let backend = backend_contract()?;
    let request = WorkerRequest {
        format: "marklab.pymc_replicated_conditional_multitype_mark_request".into(),
        version: 1,
        backend: backend.clone(),
        input_sha256: sha256_hex(&input_bytes),
        group_ids,
        reference_group: args.reference_group,
        type_ids,
        reference_type: args.reference_type,
        radius_um: args.radius_um,
        patients,
        patterns,
        points: rows,
        edges,
        priors: Priors {
            intercept_sd: args.intercept_prior_sd,
            interaction_sd: args.interaction_prior_sd,
            group_effect_sd: args.group_effect_prior_sd,
            patient_sd_scale: args.patient_sd_prior_scale,
            pattern_sd_scale: args.pattern_sd_prior_scale,
        },
        sampling: NutsSamplingSpec {
            chains: args.chains,
            tune_per_chain: args.tune,
            draws_per_chain: args.draws,
            target_accept: args.target_accept,
            seed: args.seed,
        },
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
            "replicated conditional-mark worker request exceeds input ceiling".into(),
        ));
    }
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        backend,
        request,
        request_bytes,
        request_sha256,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute_prepared(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_bytes = run_worker(
        repository,
        "marklab_pymc_replicated_conditional_multitype_mark_worker.py",
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
    Ok(Output {
        format: "marklab.replicated_conditional_multitype_mark_fit".into(),
        version: 1,
        backend: result.backend,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        radius_um: prepared.request.radius_um,
        group_ids: prepared.request.group_ids.clone(),
        reference_group: prepared.request.reference_group.clone(),
        type_ids: prepared.request.type_ids.clone(),
        reference_type: prepared.request.reference_type.clone(),
        patient_count: prepared.request.patients.len(),
        pattern_count: prepared.request.patterns.len(),
        point_count: prepared.request.points.len(),
        type_count: prepared.request.type_ids.len(),
        edge_count: prepared.request.edges.len(),
        baseline_affinities: result.baseline_affinities,
        group_affinity_shifts: result.group_affinity_shifts,
        patient_affinities: result.patient_affinities,
        hierarchy_scales: result.hierarchy_scales,
        comparison: result.comparison,
        pattern_checks: result.pattern_checks,
        diagnostics: result.diagnostics,
        resources: Resources {
            maximum_patients: prepared.request.resources.maximum_patients,
            maximum_patterns: prepared.request.resources.maximum_patterns,
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
        statistical_unit: "patient".into(),
        null_model:
            "zero_patient_population_group_shift_in_fixed_location_conditional_mark_affinity".into(),
        assumptions: [
            "cell_locations_and_within_pattern_radius_graphs_are_conditioned_on".into(),
            "patients_are_independent_and_patterns_are_nested_within_patient".into(),
            "composition_intercepts_and_spatial_pair_potentials_have_separate_hierarchies".into(),
            "symmetric_pair_potentials_use_reference_reference_zero_gauge_at_every_level".into(),
            "composite_conditional_likelihood_is_not_a_normalized_joint_likelihood".into(),
        ],
        finite_result_policy:
            "all_posterior_diagnostics_hierarchy_affinities_scores_and_predictions_must_be_finite"
                .into(),
        claim_status: "experimental_patient_population_conditional_mark_pseudolikelihood".into(),
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
            &directory.join("marklab_pymc_replicated_conditional_multitype_mark_worker.py"),
        )?),
    })
}

impl Output {
    pub(crate) fn validate_for(
        &self,
        input_sha256: &str,
        reference_group: &str,
        reference_type: &str,
        radius_um: f64,
        backend: &BackendContract,
    ) -> Result<(), BayesCliError> {
        let expected_pairs = self.type_count * (self.type_count - 1) / 2;
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
        if self.format != "marklab.replicated_conditional_multitype_mark_fit"
            || self.version != 1
            || !backend_matches(&self.backend, backend)
            || self.input_sha256 != input_sha256
            || self.reference_group != reference_group
            || self.group_ids.first().map(String::as_str) != Some(reference_group)
            || self.reference_type != reference_type
            || self.type_ids.first().map(String::as_str) != Some(reference_type)
            || self.radius_um.to_bits() != radius_um.to_bits()
            || self.group_ids.len() != 2
            || self.patient_count < 4
            || self.pattern_count < self.patient_count * 2
            || self.point_count < 24
            || self.type_count != self.type_ids.len()
            || self.baseline_affinities.len() != expected_pairs
            || self.group_affinity_shifts.len() != expected_pairs
            || self.patient_affinities.len() != self.patient_count * expected_pairs
            || self.pattern_checks.len() != self.pattern_count
            || self.edge_count == 0
            || (self.fit_state == FitState::Complete) != complete
        {
            return Err(BayesCliError::Backend(
                "durable replicated conditional-mark result differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_controls(args: &Args) -> Result<(), BayesCliError> {
    let scales = [
        args.radius_um,
        args.intercept_prior_sd,
        args.interaction_prior_sd,
        args.group_effect_prior_sd,
        args.patient_sd_prior_scale,
        args.pattern_sd_prior_scale,
    ];
    if args.reference_group.is_empty()
        || args.reference_type.is_empty()
        || scales
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        || !(2..=8).contains(&args.chains)
        || !(100..=100_000).contains(&args.tune)
        || !(100..=100_000).contains(&args.draws)
        || !(0.5..1.0).contains(&args.target_accept)
        || !(4..=64).contains(&args.maximum_patients)
        || !(8..=256).contains(&args.maximum_patterns)
        || !(24..=100_000).contains(&args.maximum_points)
        || !(3..=8).contains(&args.maximum_types)
        || args.maximum_neighbor_visits == 0
        || args.maximum_edges == 0
        || args.maximum_draw_parameter_work == 0
        || args.maximum_working_bytes == 0
        || !(10..=14).contains(&args.maximum_tree_depth)
        || !(1..=3_600).contains(&args.timeout_seconds)
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark controls are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_points(points: &[CsvPoint]) -> Result<(), BayesCliError> {
    let mut ids = HashSet::with_capacity(points.len());
    for point in points {
        if point.pattern_id.is_empty()
            || point.patient_id.is_empty()
            || point.group.is_empty()
            || point.point_id.is_empty()
            || point.type_id.is_empty()
            || point.pattern_id.len() > 256
            || point.patient_id.len() > 256
            || point.group.len() > 128
            || point.point_id.len() > 256
            || point.type_id.len() > 128
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || !ids.insert(point.point_id.as_str())
        {
            return Err(BayesCliError::Input(
                "replicated conditional-mark point identity or value is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    backend: &BackendContract,
    request_sha256: &str,
) -> Result<(), BayesCliError> {
    let expected_pairs = request.type_ids.len() * (request.type_ids.len() - 1) / 2;
    let summaries = result
        .baseline_affinities
        .iter()
        .map(|row| &row.contrast)
        .chain(result.group_affinity_shifts.iter().map(|row| &row.contrast))
        .chain(result.patient_affinities.iter().map(|row| &row.contrast))
        .chain(result.hierarchy_scales.iter().map(|row| &row.scale))
        .chain(result.pattern_checks.iter().flat_map(|row| {
            std::iter::once(&row.expected_same_type_edges).chain(row.expected_type_counts.iter())
        }))
        .chain(std::iter::once(
            &result.comparison.spatial_hierarchical_composite_log_score,
        ));
    let finite = summaries.into_iter().all(summary_finite)
        && result
            .pattern_checks
            .iter()
            .all(|row| row.expected_type_counts.len() == request.type_ids.len())
        && result
            .comparison
            .independent_pattern_label_composite_log_score
            .is_finite()
        && result
            .comparison
            .mean_composite_log_score_improvement
            .is_finite();
    let complete = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= 1.01
        && result.diagnostics.ess_bulk >= 400.0
        && result.diagnostics.ess_tail >= 400.0
        && result.diagnostics.minimum_ebfmi >= 0.2
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.pymc_replicated_conditional_multitype_mark_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha256
        || result.sampling.chains != request.sampling.chains
        || result.sampling.tune_per_chain != request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != request.sampling.draws_per_chain
        || result.sampling.completed_draws
            != u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain)
        || result.baseline_affinities.len() != expected_pairs
        || result.group_affinity_shifts.len() != expected_pairs
        || result.patient_affinities.len() != request.patients.len() * expected_pairs
        || result.hierarchy_scales.len() != 4
        || result.pattern_checks.len() != request.patterns.len()
        || !finite
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "replicated conditional-mark backend result differs".into(),
        ));
    }
    Ok(())
}

fn summary_finite(summary: &SarScalarSummary) -> bool {
    summary.mean.is_finite()
        && summary.sd.is_finite()
        && summary.interval_lower.is_finite()
        && summary.interval_upper.is_finite()
        && summary.sd >= 0.0
        && summary.interval_lower <= summary.interval_upper
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
