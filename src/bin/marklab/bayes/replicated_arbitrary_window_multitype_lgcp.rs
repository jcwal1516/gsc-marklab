pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp::{
    validate_result, Node, NodeTypeCount, Output, Patient, Pattern, Priors, Resources,
    WorkerRequest, WorkerResult,
};

use marklab_numerics::dense;
use marklab_numerics::kernel::matern32;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::DiagnosticPolicy;
use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;
use serde::Deserialize;

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
    /// Per-node, per-type quadrature CSV (pattern_id, patient_id, group, type_id, count, ...).
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    pub(crate) reference_type: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    pub(crate) covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    pub(crate) patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pub(crate) pattern_sd_prior_scale: f64,
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
    /// Upper bound on patients.
    #[arg(long)]
    pub(crate) maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
    /// Upper bound on types.
    #[arg(long)]
    pub(crate) maximum_types: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    pub(crate) maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    pub(crate) maximum_total_nodes: usize,
    /// Upper bound on total node type rows.
    #[arg(long)]
    pub(crate) maximum_total_node_type_rows: usize,
    /// Upper bound on total events.
    #[arg(long)]
    pub(crate) maximum_total_events: u64,
    /// Upper bound on draw node type work.
    #[arg(long)]
    pub(crate) maximum_draw_node_type_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    pattern_id: String,
    patient_id: String,
    group: String,
    cohort: String,
    node_id: String,
    type_id: String,
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    window_area_um2: f64,
    covariate: f64,
    count: u64,
    window_sha256: String,
    event_sha256: String,
}

pub(crate) struct Prepared {
    request: WorkerRequest,
    request_bytes: Vec<u8>,
    request_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn backend(&self) -> &BackendContract {
        &self.request.backend
    }

    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }

    pub(crate) fn request_sha256(&self) -> &str {
        &self.request_sha256
    }

    pub(crate) fn draw_node_type_work(&self) -> u64 {
        self.request.resources.draw_node_type_work
    }
}

pub(super) fn run(args: Box<Args>) -> Result<(), CliError> {
    let out = args.out.clone();
    publish_json(&out, &execute(*args)?)
}

pub(crate) fn execute(args: Args) -> Result<Output, CliError> {
    execute_prepared(&prepare(args)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, CliError> {
    let timeout_seconds = args.timeout_seconds;
    let request = prepare_request(args)?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        request,
        request_bytes,
        request_sha256,
        timeout_seconds,
    })
}

pub(crate) fn execute_prepared(prepared: &Prepared) -> Result<Output, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let bytes = run_worker(
        repository,
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_result(&result, &prepared.request, &prepared.request_sha256)
        .map_err(super::protocol_error)?;
    let request = &prepared.request;
    let cohort_count = request
        .patterns
        .iter()
        .map(|pattern| pattern.cohort.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    Ok(Output {
        format: result.format,
        version: result.version,
        backend: result.backend,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        type_posteriors: result.type_posteriors,
        group_effect_differences: result.group_effect_differences,
        patient_type_effects: result.patient_type_effects,
        pattern_type_effects: result.pattern_type_effects,
        node_type_posteriors: result.node_type_posteriors,
        pattern_type_posterior_predictive: result.pattern_type_posterior_predictive,
        diagnostics: result.diagnostics,
        patient_count: request.patients.len(),
        pattern_count: request.patterns.len(),
        type_count: request.type_ids.len(),
        total_node_count: request.nodes.len(),
        total_node_type_row_count: request.node_type_counts.len(),
        total_event_count: request.node_type_counts.iter().map(|row| row.count).sum(),
        groups: [
            request.reference_group.clone(),
            request.comparison_group.clone(),
        ],
        reference_type: request.reference_type.clone(),
        cohort_count,
        priors: request.priors.clone(),
        resources: request.resources.clone(),
        statistical_unit: StatisticalUnit::Patient,
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        likelihood: "independent_type_specific_poisson_intensities_over_one_shared_exact_window"
            .into(),
        null_model: NullHypothesis::AllTypeSpecificComparisonGroupLogIntensityEffectsEqualZero,
        assumptions: [
            "patients_are_independent_population_units".into(),
            "slide_patterns_are_repeated_measurements_not_population_replicates".into(),
            "each_type_is_a_complete_partition_of_one_shared_event_set".into(),
            "each_exact_window_and_typed_event_set_is_provenance_complete".into(),
            "fixed_matern_fields_are_independent_across_types_and_slide_patterns".into(),
            "this_is_a_multitype_cox_process_not_a_pairwise_gibbs_interaction_model".into(),
            "group_effects_are_associational_not_causal_or_clinical".into(),
        ],
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if result.fit_state == FitState::Complete {
            ClaimStatus::ExperimentalReplicatedMultitypePointProcess
        } else {
            ClaimStatus::DiagnosticOnlyNonconverged
        },
    })
}

fn prepare_request(args: Args) -> Result<WorkerRequest, CliError> {
    validate_controls(&args)?;
    let metadata = fs::metadata(&args.input).map_err(|source| CliError::Io {
        path: args.input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "replicated multitype LGCP input file or size is invalid".into(),
        ));
    }
    let bytes = fs::read(&args.input).map_err(|source| CliError::Io {
        path: args.input.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
    let expected = [
        "pattern_id",
        "patient_id",
        "group",
        "cohort",
        "node_id",
        "type_id",
        "x_um",
        "y_um",
        "weight_um2",
        "window_area_um2",
        "covariate",
        "count",
        "window_sha256",
        "event_sha256",
    ];
    if reader.headers()?.iter().ne(expected) {
        return Err(CliError::Input(
            "replicated exact-window multitype LGCP headers differ".into(),
        ));
    }
    let mut rows = reader.deserialize().collect::<Result<Vec<CsvRow>, _>>()?;
    rows.sort_by(|left, right| {
        (&left.pattern_id, &left.node_id, &left.type_id).cmp(&(
            &right.pattern_id,
            &right.node_id,
            &right.type_id,
        ))
    });
    validate_rows(&rows)?;
    let mut type_ids = rows
        .iter()
        .map(|row| row.type_id.clone())
        .collect::<Vec<_>>();
    type_ids.sort();
    type_ids.dedup();
    if !(3..=args.maximum_types).contains(&type_ids.len()) {
        return Err(CliError::Input(
            "replicated multitype LGCP type count is invalid".into(),
        ));
    }
    let reference = type_ids
        .iter()
        .position(|identity| identity == &args.reference_type)
        .ok_or_else(|| CliError::Input("reference type is absent".into()))?;
    let reference_type = type_ids.remove(reference);
    type_ids.insert(0, reference_type);
    let type_indices = type_ids
        .iter()
        .enumerate()
        .map(|(index, identity)| (identity.as_str(), index))
        .collect::<BTreeMap<_, _>>();

    let mut patient_groups = BTreeMap::<String, String>::new();
    for row in &rows {
        match patient_groups.entry(row.patient_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(row.group.clone());
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &row.group => {
                return Err(CliError::Input(
                    "replicated multitype LGCP patient group changes".into(),
                ));
            }
            _ => {}
        }
    }
    if patient_groups.len() > args.maximum_patients {
        return Err(CliError::Input(
            "replicated multitype LGCP patient ceiling exceeded".into(),
        ));
    }
    let patients = patient_groups
        .iter()
        .map(|(patient_id, group)| {
            let group_index = if group == &args.reference_group {
                0
            } else if group == &args.comparison_group {
                1
            } else {
                return Err(CliError::Input(
                    "replicated multitype LGCP has a foreign group".into(),
                ));
            };
            Ok(Patient {
                patient_id: patient_id.clone(),
                group: group.clone(),
                group_index,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let patient_indices = patients
        .iter()
        .enumerate()
        .map(|(index, patient)| (patient.patient_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut by_pattern = BTreeMap::<String, Vec<CsvRow>>::new();
    for row in rows {
        by_pattern
            .entry(row.pattern_id.clone())
            .or_default()
            .push(row);
    }
    if !(16..=args.maximum_patterns).contains(&by_pattern.len()) {
        return Err(CliError::Input(
            "replicated multitype LGCP pattern count is invalid".into(),
        ));
    }

    let mut patterns = Vec::with_capacity(by_pattern.len());
    let mut nodes = Vec::new();
    let mut node_type_counts = Vec::new();
    let mut blocks = Vec::new();
    let mut pattern_counts = vec![0_usize; patients.len()];
    for (pattern_index, (pattern_id, pattern_rows)) in by_pattern.into_iter().enumerate() {
        let first = &pattern_rows[0];
        let patient_index = *patient_indices
            .get(first.patient_id.as_str())
            .ok_or_else(|| CliError::Input("pattern patient is absent".into()))?;
        pattern_counts[patient_index] += 1;
        if pattern_rows.iter().any(|row| {
            row.patient_id != first.patient_id
                || row.group != first.group
                || row.cohort != first.cohort
                || row.window_sha256 != first.window_sha256
                || row.window_area_um2 != first.window_area_um2
                || row.event_sha256 != first.event_sha256
        }) {
            return Err(CliError::Input(
                "replicated multitype LGCP pattern identity changes".into(),
            ));
        }
        let patient_id = first.patient_id.clone();
        let cohort = first.cohort.clone();
        let window_sha256 = first.window_sha256.clone();
        let window_area_um2 = first.window_area_um2;
        let event_sha256 = first.event_sha256.clone();
        let mut by_node = BTreeMap::<String, Vec<CsvRow>>::new();
        for row in pattern_rows {
            by_node.entry(row.node_id.clone()).or_default().push(row);
        }
        if !(4..=args.maximum_nodes_per_pattern).contains(&by_node.len()) {
            return Err(CliError::Input(
                "replicated multitype LGCP node count is invalid".into(),
            ));
        }
        let mut block_rows = Vec::with_capacity(by_node.len());
        let pattern_node_start = nodes.len();
        let mut event_count = 0_u64;
        let mut area = 0.0;
        for (node_id, mut typed_rows) in by_node {
            typed_rows.sort_by_key(|row| type_indices[row.type_id.as_str()]);
            if typed_rows.len() != type_ids.len()
                || typed_rows
                    .iter()
                    .zip(&type_ids)
                    .any(|(row, identity)| &row.type_id != identity)
            {
                return Err(CliError::Input(
                    "every replicated multitype LGCP node requires every type exactly once".into(),
                ));
            }
            let node_first = &typed_rows[0];
            if typed_rows.iter().any(|row| {
                row.x_um != node_first.x_um
                    || row.y_um != node_first.y_um
                    || row.weight_um2 != node_first.weight_um2
                    || row.covariate != node_first.covariate
            }) {
                return Err(CliError::Input(
                    "typed node geometry or covariate changes across types".into(),
                ));
            }
            let node_index = nodes.len();
            area += node_first.weight_um2;
            nodes.push(Node {
                pattern_id: pattern_id.clone(),
                pattern_index,
                patient_index,
                node_id,
                x_um: node_first.x_um,
                y_um: node_first.y_um,
                weight_um2: node_first.weight_um2,
                covariate: node_first.covariate,
            });
            block_rows.push((node_first.x_um, node_first.y_um));
            for row in typed_rows {
                event_count = event_count
                    .checked_add(row.count)
                    .ok_or_else(|| CliError::Input("multitype event count overflows".into()))?;
                node_type_counts.push(NodeTypeCount {
                    node_index,
                    type_index: type_indices[row.type_id.as_str()],
                    count: row.count,
                });
            }
        }
        if (area - window_area_um2).abs() > 1e-10 * window_area_um2.abs().max(1.0) {
            return Err(CliError::Input(
                "replicated multitype LGCP node weights do not conserve exact window area".into(),
            ));
        }
        let dimension = nodes.len() - pattern_node_start;
        blocks.push((
            pattern_node_start,
            dimension,
            cholesky_block(
                &block_rows,
                args.field_amplitude,
                args.field_length_scale_um,
                args.jitter,
            )?,
        ));
        patterns.push(Pattern {
            pattern_id,
            patient_id,
            patient_index,
            cohort,
            window_sha256,
            window_area_um2,
            event_sha256,
            event_count,
            node_count: dimension,
        });
    }
    validate_hierarchy_and_work(&args, &patients, &patterns, &nodes, &node_type_counts)?;
    let mut cholesky = vec![0.0; nodes.len() * nodes.len()];
    for (start, dimension, block) in blocks {
        for row in 0..dimension {
            for column in 0..dimension {
                cholesky[(start + row) * nodes.len() + start + column] =
                    block[row * dimension + column];
            }
        }
    }
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let draw_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul(node_type_counts.len() as u64))
        .ok_or_else(|| CliError::Input("multitype draw work overflows".into()))?;
    let directory = marklab::backend::python_backend_assets_root()?.join("workers/python");
    let worker_name = "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py";
    Ok(WorkerRequest {
        format: "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "pymc",
            version: PYMC_VERSION,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: sha256_hex(&read_bounded(&directory.join("uv.lock"))?),
            worker_sha256: sha256_hex(&read_bounded(&directory.join(worker_name))?),
        },
        input_sha256: sha256_hex(&bytes),
        reference_group: args.reference_group,
        comparison_group: args.comparison_group,
        type_ids,
        reference_type: args.reference_type,
        patients,
        patterns,
        nodes,
        node_type_counts,
        cholesky,
        priors: Priors {
            intercept_mean: args.intercept_prior_mean,
            intercept_sd: args.intercept_prior_sd,
            group_effect_sd: args.group_effect_prior_sd,
            covariate_effect_sd: args.covariate_effect_prior_sd,
            patient_sd_scale: args.patient_sd_prior_scale,
            pattern_sd_scale: args.pattern_sd_prior_scale,
            field_amplitude: args.field_amplitude,
            field_length_scale_um: args.field_length_scale_um,
            jitter: args.jitter,
        },
        sampling,
        diagnostic_policy: DiagnosticPolicy::default(),
        resources: Resources {
            maximum_patients: args.maximum_patients,
            maximum_patterns: args.maximum_patterns,
            maximum_types: args.maximum_types,
            maximum_nodes_per_pattern: args.maximum_nodes_per_pattern,
            maximum_total_nodes: args.maximum_total_nodes,
            maximum_total_node_type_rows: args.maximum_total_node_type_rows,
            maximum_total_events: args.maximum_total_events,
            maximum_draw_node_type_work: args.maximum_draw_node_type_work,
            draw_node_type_work: draw_work,
            timeout_seconds: args.timeout_seconds,
        },
    })
}

fn validate_controls(args: &Args) -> Result<(), CliError> {
    let scales = [
        args.intercept_prior_mean,
        args.intercept_prior_sd,
        args.group_effect_prior_sd,
        args.covariate_effect_prior_sd,
        args.patient_sd_prior_scale,
        args.pattern_sd_prior_scale,
        args.field_amplitude,
        args.field_length_scale_um,
        args.jitter,
    ];
    if args.reference_group.is_empty()
        || args.reference_group == args.comparison_group
        || args.reference_type.is_empty()
        || !scales.into_iter().all(f64::is_finite)
        || scales[1..].iter().any(|value| *value <= 0.0)
        || !(8..=32).contains(&args.maximum_patients)
        || !(16..=64).contains(&args.maximum_patterns)
        || !(3..=8).contains(&args.maximum_types)
        || !(4..=36).contains(&args.maximum_nodes_per_pattern)
        || !(64..=512).contains(&args.maximum_total_nodes)
        || args.maximum_total_node_type_rows == 0
        || args.maximum_total_events == 0
        || args.maximum_draw_node_type_work == 0
        || args.timeout_seconds == 0
        || args.timeout_seconds > 3600
    {
        return Err(CliError::Input(
            "replicated multitype LGCP controls are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_rows(rows: &[CsvRow]) -> Result<(), CliError> {
    if rows.is_empty() {
        return Err(CliError::Input(
            "replicated multitype LGCP rows are empty".into(),
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        if row.pattern_id.is_empty()
            || row.patient_id.is_empty()
            || row.group.is_empty()
            || row.cohort.is_empty()
            || row.node_id.is_empty()
            || row.type_id.is_empty()
            || ![
                row.x_um,
                row.y_um,
                row.weight_um2,
                row.window_area_um2,
                row.covariate,
            ]
            .into_iter()
            .all(f64::is_finite)
            || row.weight_um2 <= 0.0
            || row.window_area_um2 <= 0.0
            || !is_digest(&row.window_sha256)
            || !is_digest(&row.event_sha256)
            || (index > 0
                && rows[index - 1].pattern_id == row.pattern_id
                && rows[index - 1].node_id == row.node_id
                && rows[index - 1].type_id == row.type_id)
        {
            return Err(CliError::Input(
                "replicated multitype LGCP row is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn validate_hierarchy_and_work(
    args: &Args,
    patients: &[Patient],
    patterns: &[Pattern],
    nodes: &[Node],
    rows: &[NodeTypeCount],
) -> Result<(), CliError> {
    let mut pattern_counts = vec![0_usize; patients.len()];
    for pattern in patterns {
        pattern_counts[pattern.patient_index] += 1;
    }
    let reference_patients = patients
        .iter()
        .filter(|patient| patient.group_index == 0)
        .count();
    let comparison_patients = patients
        .iter()
        .filter(|patient| patient.group_index == 1)
        .count();
    let events = rows
        .iter()
        .try_fold(0_u64, |total, row| total.checked_add(row.count));
    let work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul(rows.len() as u64));
    if pattern_counts.iter().any(|count| !(2..=4).contains(count))
        || reference_patients < 4
        || comparison_patients < 4
        || nodes.len() > args.maximum_total_nodes
        || rows.len() > args.maximum_total_node_type_rows
        || events.is_none_or(|value| value > args.maximum_total_events)
        || work.is_none_or(|value| value > args.maximum_draw_node_type_work)
    {
        return Err(CliError::Input(
            "replicated multitype LGCP hierarchy or work ceiling differs".into(),
        ));
    }
    Ok(())
}

fn cholesky_block(
    coordinates: &[(f64, f64)],
    amplitude: f64,
    length: f64,
    jitter: f64,
) -> Result<Vec<f64>, CliError> {
    let dimension = coordinates.len();
    let mut covariance = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            let distance = (coordinates[row].0 - coordinates[column].0)
                .hypot(coordinates[row].1 - coordinates[column].1);
            covariance[row * dimension + column] =
                matern32(distance, amplitude, length) + if row == column { jitter } else { 0.0 };
        }
    }
    dense::cholesky(&covariance, dimension)
        .ok_or_else(|| CliError::Input("replicated multitype LGCP covariance is invalid".into()))
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        8 * 1_048_576,
        "replicated multitype LGCP backend source is absent or oversized",
    )
}

impl Prepared {
    pub(crate) fn validate_result(&self, result: &Output) -> Result<(), CliError> {
        result
            .validate_for(&self.request, &self.request_sha256)
            .map_err(super::protocol_error)
    }
}
