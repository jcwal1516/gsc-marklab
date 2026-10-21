pub(crate) use marklab_bayes::protocol::replicated_conditional_multitype_mark::{
    validate_points, validate_result, CsvPoint, DiagnosticPolicy, Edge, Output, Patient, Pattern,
    PointRow, Priors, Resources, WorkerRequest, WorkerResult,
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

const MAXIMUM_INPUT_BYTES: u64 = 64 * 1_048_576;

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    /// CSV of points with pattern_id, patient_id, group, point_id, x_um, y_um and type_id.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
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
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    pub(crate) patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pub(crate) pattern_sd_prior_scale: f64,
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
    /// Upper bound on patients.
    #[arg(long)]
    pub(crate) maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
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

pub(super) fn run(args: Box<Args>) -> Result<(), CliError> {
    let output_path = args.out.clone();
    publish_json(&output_path, &execute(*args)?)
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
            "replicated conditional-mark input file or size is invalid".into(),
        ));
    }
    let input_bytes = fs::read(&args.input).map_err(|source| CliError::Io {
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
        return Err(CliError::Input(
            "replicated conditional-mark CSV headers differ".into(),
        ));
    }
    let mut points = reader
        .deserialize::<CsvPoint>()
        .collect::<Result<Vec<_>, _>>()?;
    if !(24..=args.maximum_points).contains(&points.len()) {
        return Err(CliError::Input(
            "replicated conditional-mark point count is invalid".into(),
        ));
    }
    points.sort_by(|left, right| {
        (&left.pattern_id, &left.point_id).cmp(&(&right.pattern_id, &right.point_id))
    });
    validate_points(&points).map_err(super::protocol_error)?;

    let mut type_ids = points
        .iter()
        .map(|point| point.type_id.clone())
        .collect::<Vec<_>>();
    type_ids.sort();
    type_ids.dedup();
    if !(3..=args.maximum_types.min(8)).contains(&type_ids.len())
        || !type_ids.contains(&args.reference_type)
    {
        return Err(CliError::Input(
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
                return Err(CliError::Input(
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
                return Err(CliError::Input(
                    "one pattern belongs to multiple patients or groups".into(),
                ));
            }
            _ => {}
        }
    }
    if patient_groups.len() > args.maximum_patients || pattern_owners.len() > args.maximum_patterns
    {
        return Err(CliError::Input(
            "replicated conditional-mark hierarchy exceeds its ceiling".into(),
        ));
    }
    let mut group_ids = patient_groups.values().cloned().collect::<Vec<_>>();
    group_ids.sort();
    group_ids.dedup();
    if group_ids.len() != 2 || !group_ids.contains(&args.reference_group) {
        return Err(CliError::Input(
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
        return Err(CliError::Input(
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
        return Err(CliError::Input(
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
            return Err(CliError::Input(
                "every pattern requires at least six points".into(),
            ));
        }
        let mut observed = vec![0_usize; type_ids.len()];
        for point in pattern_points {
            observed[type_index[point.type_id.as_str()]] += 1;
        }
        if observed.iter().any(|count| *count < 2) {
            return Err(CliError::Input(
                "every pattern requires at least two points of every type".into(),
            ));
        }
        let visits = (pattern_points.len() as u64)
            .checked_mul(pattern_points.len().saturating_sub(1) as u64)
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| CliError::Input("neighbor work overflows".into()))?;
        neighbor_visits = neighbor_visits
            .checked_add(visits)
            .ok_or_else(|| CliError::Input("neighbor work overflows".into()))?;
        let edge_begin = edges.len();
        for left in begin..end {
            for right in left + 1..end {
                let distance = (points[left].x_um - points[right].x_um)
                    .hypot(points[left].y_um - points[right].y_um);
                if !distance.is_finite() {
                    return Err(CliError::Input("neighbor distance is nonfinite".into()));
                }
                if distance <= args.radius_um {
                    if edges.len() == args.maximum_edges {
                        return Err(CliError::Input(
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
            return Err(CliError::Input(
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
        return Err(CliError::Input(format!(
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
        .ok_or_else(|| CliError::Input("parameter count overflows".into()))?;
    let completed_draws = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .ok_or_else(|| CliError::Input("completed draws overflow".into()))?;
    let draw_parameter_work = completed_draws
        .checked_mul(parameter_count as u64)
        .ok_or_else(|| CliError::Input("draw-parameter work overflows".into()))?;
    if draw_parameter_work > args.maximum_draw_parameter_work {
        return Err(CliError::Input(
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
        .ok_or_else(|| CliError::Input("working-byte estimate overflows".into()))?;
    if estimated_working_bytes > args.maximum_working_bytes {
        return Err(CliError::Input(
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
        return Err(CliError::Input(
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

pub(crate) fn execute_prepared(prepared: &Prepared) -> Result<Output, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
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
    )
    .map_err(super::protocol_error)?;
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
        statistical_unit: StatisticalUnit::Patient,
        null_model:
            NullHypothesis::ZeroPatientPopulationGroupShiftInFixedLocationConditionalMarkAffinity,
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
        claim_status: ClaimStatus::ExperimentalPatientPopulationConditionalMarkPseudolikelihood,
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
            &directory.join("marklab_pymc_replicated_conditional_multitype_mark_worker.py"),
        )?),
    })
}

fn validate_controls(args: &Args) -> Result<(), CliError> {
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
        || !is_worker_timeout(args.timeout_seconds)
    {
        return Err(CliError::Input(
            "replicated conditional-mark controls are invalid".into(),
        ));
    }
    Ok(())
}
