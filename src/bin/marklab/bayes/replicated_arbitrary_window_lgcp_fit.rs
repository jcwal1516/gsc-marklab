use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, run_worker, BayesCliError};

const PYMC_VERSION: &str = "6.3.0";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct FitCli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand)]
enum TopLevel {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    FitReplicatedArbitraryWindowLgcp(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
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
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    pattern_id: String,
    patient_id: String,
    group: String,
    cohort: String,
    node_id: String,
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    window_area_um2: f64,
    covariate: f64,
    count: u64,
    window_sha256: String,
    event_sha256: String,
}

#[derive(Debug, Serialize)]
struct Patient {
    patient_id: String,
    group: String,
    group_index: u8,
}

#[derive(Debug, Serialize)]
struct Pattern {
    pattern_id: String,
    patient_id: String,
    patient_index: usize,
    cohort: String,
    window_sha256: String,
    window_area_um2: f64,
    event_sha256: String,
    event_count: u64,
    node_count: usize,
}

#[derive(Debug, Serialize)]
struct Node {
    pattern_id: String,
    pattern_index: usize,
    patient_index: usize,
    node_id: String,
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    covariate: f64,
    count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Priors {
    intercept_mean: f64,
    intercept_sd: f64,
    group_effect_sd: f64,
    covariate_effect_sd: f64,
    patient_sd_scale: f64,
    pattern_sd_scale: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_nodes_per_pattern: usize,
    maximum_total_nodes: usize,
    maximum_total_events: u64,
    maximum_draw_node_work: u64,
    draw_node_work: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkerRequest {
    format: &'static str,
    version: u32,
    pub(crate) backend: BackendContract,
    input_sha256: String,
    reference_group: String,
    comparison_group: String,
    patients: Vec<Patient>,
    patterns: Vec<Pattern>,
    nodes: Vec<Node>,
    cholesky: Vec<f64>,
    priors: Priors,
    sampling: NutsSamplingSpec,
    diagnostic_policy: DiagnosticPolicy,
    resources: Resources,
}

impl WorkerRequest {
    pub(crate) fn input_sha256(&self) -> &str {
        &self.input_sha256
    }

    pub(crate) fn first_node_identity(&self) -> (&str, &str) {
        (&self.nodes[0].pattern_id, &self.nodes[0].node_id)
    }

    pub(crate) fn completed_draws(&self) -> u64 {
        u64::from(self.sampling.chains) * u64::from(self.sampling.draws_per_chain)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Posterior {
    pub(crate) intercept: SarScalarSummary,
    pub(crate) group_effect: SarScalarSummary,
    pub(crate) covariate_effect: SarScalarSummary,
    pub(crate) patient_sd: SarScalarSummary,
    pub(crate) pattern_sd: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatientEffect {
    pub(crate) patient_id: String,
    pub(crate) effect: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternEffect {
    pub(crate) pattern_id: String,
    pub(crate) effect: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodePosterior {
    pub(crate) pattern_id: String,
    pub(crate) node_id: String,
    pub(crate) latent_effect: SarScalarSummary,
    pub(crate) expected_count: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatternPosteriorPredictive {
    pub(crate) pattern_id: String,
    pub(crate) observed_total_count: u64,
    pub(crate) replicate_count: u64,
    pub(crate) replicated_total_count_mean: f64,
    pub(crate) replicated_total_count_sd: f64,
    pub(crate) replicated_total_count_interval_lower: f64,
    pub(crate) replicated_total_count_interval_upper: f64,
    pub(crate) total_count_two_sided_tail_probability: f64,
    pub(crate) observed_node_count_variance: f64,
    pub(crate) replicated_node_count_variance_mean: f64,
    pub(crate) node_variance_two_sided_tail_probability: f64,
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
    pub(crate) sampling: SamplingSummary,
    posterior: Posterior,
    patient_effects: Vec<PatientEffect>,
    pattern_effects: Vec<PatternEffect>,
    nodes: Vec<NodePosterior>,
    pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResultDocument {
    format: String,
    version: u32,
    pub(crate) backend: WorkerBackend,
    pub(crate) input_sha256: String,
    pub(crate) request_sha256: String,
    pub(crate) fit_state: FitState,
    pub(crate) sampling: SamplingSummary,
    pub(crate) posterior: Posterior,
    pub(crate) patient_effects: Vec<PatientEffect>,
    pub(crate) pattern_effects: Vec<PatternEffect>,
    pub(crate) nodes: Vec<NodePosterior>,
    pub(crate) pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    pub(crate) diagnostics: NormalMeanDiagnostics,
    patient_count: usize,
    pattern_count: usize,
    total_node_count: usize,
    total_event_count: u64,
    groups: [String; 2],
    cohort_count: usize,
    priors: Priors,
    resources: Resources,
    statistical_unit: String,
    pattern_unit: String,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct PreparedReplicatedArbitraryWindowLgcpFit {
    pub(crate) request: WorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) timeout_seconds: u64,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = FitCli::parse_from(std::env::args_os()).command;
    let Command::FitReplicatedArbitraryWindowLgcp(arguments) = command;
    let output = arguments.out.clone();
    let prepared = prepare_arguments(*arguments)?;
    publish_json(&output, &execute(&prepared)?)
}

pub(crate) fn execute(
    prepared: &PreparedReplicatedArbitraryWindowLgcpFit,
) -> Result<ResultDocument, BayesCliError> {
    let request = &prepared.request;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes = run_worker(
        repository,
        "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let worker: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_result(&worker, request, &prepared.request_sha256)?;
    let cohort_count = request
        .patterns
        .iter()
        .map(|pattern| pattern.cohort.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    Ok(ResultDocument {
        format: worker.format,
        version: worker.version,
        backend: worker.backend,
        input_sha256: worker.input_sha256,
        request_sha256: worker.request_sha256,
        fit_state: worker.fit_state,
        sampling: worker.sampling,
        posterior: worker.posterior,
        patient_effects: worker.patient_effects,
        pattern_effects: worker.pattern_effects,
        nodes: worker.nodes,
        pattern_posterior_predictive: worker.pattern_posterior_predictive,
        diagnostics: worker.diagnostics,
        patient_count: request.patients.len(),
        pattern_count: request.patterns.len(),
        total_node_count: request.nodes.len(),
        total_event_count: request.nodes.iter().map(|node| node.count).sum(),
        groups: [
            request.reference_group.clone(),
            request.comparison_group.clone(),
        ],
        cohort_count,
        priors: request.priors.clone(),
        resources: request.resources.clone(),
        statistical_unit: "patient".into(),
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        null_model: "comparison_group_log_intensity_effect_equals_zero".into(),
        assumptions: vec![
            "patients_are_independent_population_units".into(),
            "slide_patterns_are_repeated_measurements_not_population_replicates".into(),
            "each_exact_window_and_event_set_is_provenance_complete".into(),
            "fixed_matern_fields_are_independent_across_slide_patterns".into(),
            "pattern_effects_sum_to_zero_within_patient".into(),
            "latent_fields_sum_to_zero_within_slide_pattern".into(),
            "group_effect_is_associational_not_causal_or_clinical".into(),
        ],
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if worker.fit_state == FitState::Complete {
            "experimental_replicated_pattern_hierarchy"
        } else {
            "diagnostic_only_nonconverged"
        }
        .into(),
    })
}

fn prepare_arguments(
    arguments: Arguments,
) -> Result<PreparedReplicatedArbitraryWindowLgcpFit, BayesCliError> {
    let timeout_seconds = arguments.timeout_seconds;
    let request = prepare_request(arguments)?;
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedReplicatedArbitraryWindowLgcpFit {
        request,
        request_bytes,
        request_sha256,
        timeout_seconds,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    covariate_effect_prior_sd: f64,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_nodes_per_pattern: usize,
    maximum_total_nodes: usize,
    maximum_total_events: u64,
    maximum_draw_node_work: u64,
    timeout_seconds: u64,
) -> Result<PreparedReplicatedArbitraryWindowLgcpFit, BayesCliError> {
    prepare_arguments(Arguments {
        input,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        covariate_effect_prior_sd,
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude,
        field_length_scale_um,
        jitter,
        chains: sampling.chains,
        tune: sampling.tune_per_chain,
        draws: sampling.draws_per_chain,
        target_accept: sampling.target_accept,
        seed: sampling.seed,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        timeout_seconds,
        out: PathBuf::new(),
    })
}

fn prepare_request(arguments: Arguments) -> Result<WorkerRequest, BayesCliError> {
    let bytes = fs::read(&arguments.input).map_err(|source| BayesCliError::Io {
        path: arguments.input.clone(),
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
        return Err(BayesCliError::Input(
            "replicated exact-window LGCP headers differ".into(),
        ));
    }
    let mut rows = reader.deserialize().collect::<Result<Vec<CsvRow>, _>>()?;
    rows.sort_by(|left, right| {
        (&left.pattern_id, &left.node_id).cmp(&(&right.pattern_id, &right.node_id))
    });
    validate_controls(&arguments, &rows)?;
    let mut patient_groups = BTreeMap::new();
    for row in &rows {
        match patient_groups.entry(row.patient_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(row.group.clone());
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &row.group => {
                return Err(BayesCliError::Input(
                    "replicated LGCP patient group changes across rows".into(),
                ));
            }
            _ => {}
        }
    }
    if patient_groups.len() > arguments.maximum_patients {
        return Err(BayesCliError::Input(
            "replicated LGCP patient ceiling exceeded".into(),
        ));
    }
    let patients = patient_groups
        .iter()
        .map(|(patient_id, group)| {
            let group_index = if group == &arguments.reference_group {
                0
            } else if group == &arguments.comparison_group {
                1
            } else {
                return Err(BayesCliError::Input(
                    "replicated LGCP has a foreign group".into(),
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
    let mut grouped = BTreeMap::<String, Vec<CsvRow>>::new();
    for row in rows {
        grouped.entry(row.pattern_id.clone()).or_default().push(row);
    }
    if !(6..=arguments.maximum_patterns).contains(&grouped.len()) {
        return Err(BayesCliError::Input(
            "replicated LGCP pattern count is invalid".into(),
        ));
    }
    let mut patterns = Vec::with_capacity(grouped.len());
    let mut nodes = Vec::new();
    let mut pattern_counts = vec![0_usize; patients.len()];
    let mut cholesky = Vec::new();
    for (pattern_index, (pattern_id, pattern_rows)) in grouped.into_iter().enumerate() {
        if !(4..=arguments.maximum_nodes_per_pattern).contains(&pattern_rows.len()) {
            return Err(BayesCliError::Input(
                "replicated LGCP node count is invalid".into(),
            ));
        }
        let first = &pattern_rows[0];
        let patient_index = *patient_indices.get(first.patient_id.as_str()).unwrap();
        pattern_counts[patient_index] += 1;
        if pattern_rows.iter().any(|row| {
            row.patient_id != first.patient_id
                || row.group != first.group
                || row.cohort != first.cohort
                || row.window_sha256 != first.window_sha256
                || row.window_area_um2 != first.window_area_um2
                || row.event_sha256 != first.event_sha256
        }) {
            return Err(BayesCliError::Input(
                "replicated LGCP pattern identity changes".into(),
            ));
        }
        let event_count = pattern_rows.iter().map(|row| row.count).sum();
        let weight_sum = pattern_rows.iter().map(|row| row.weight_um2).sum::<f64>();
        if (weight_sum - first.window_area_um2).abs() > 1e-10 * first.window_area_um2.abs().max(1.0)
        {
            return Err(BayesCliError::Input(
                "replicated LGCP node weights do not conserve exact window area".into(),
            ));
        }
        patterns.push(Pattern {
            pattern_id: pattern_id.clone(),
            patient_id: first.patient_id.clone(),
            patient_index,
            cohort: first.cohort.clone(),
            window_sha256: first.window_sha256.clone(),
            window_area_um2: first.window_area_um2,
            event_sha256: first.event_sha256.clone(),
            event_count,
            node_count: pattern_rows.len(),
        });
        let block = cholesky_block(
            &pattern_rows,
            arguments.field_amplitude,
            arguments.field_length_scale_um,
            arguments.jitter,
        )?;
        cholesky.push((nodes.len(), pattern_rows.len(), block));
        for row in pattern_rows {
            nodes.push(Node {
                pattern_id: pattern_id.clone(),
                pattern_index,
                patient_index,
                node_id: row.node_id,
                x_um: row.x_um,
                y_um: row.y_um,
                weight_um2: row.weight_um2,
                covariate: row.covariate,
                count: row.count,
            });
        }
    }
    if pattern_counts.iter().any(|count| !(2..=4).contains(count))
        || patients
            .iter()
            .filter(|patient| patient.group_index == 0)
            .count()
            < 4
        || patients
            .iter()
            .filter(|patient| patient.group_index == 1)
            .count()
            < 4
        || nodes.len() > arguments.maximum_total_nodes
    {
        return Err(BayesCliError::Input(
            "replicated LGCP requires four patients per group and two to four patterns each".into(),
        ));
    }
    let total_events = nodes.iter().map(|node| node.count).sum::<u64>();
    if total_events > arguments.maximum_total_events {
        return Err(BayesCliError::Input(
            "replicated LGCP event ceiling exceeded".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let work = u64::from(arguments.chains)
        .checked_mul(u64::from(arguments.draws))
        .and_then(|value| value.checked_mul(nodes.len() as u64))
        .ok_or_else(|| BayesCliError::Input("replicated LGCP work overflows".into()))?;
    if work > arguments.maximum_draw_node_work {
        return Err(BayesCliError::Input(
            "replicated LGCP draw-node ceiling exceeded".into(),
        ));
    }
    let mut full = vec![0.0; nodes.len() * nodes.len()];
    for (start, dimension, block) in cholesky {
        for row in 0..dimension {
            for column in 0..dimension {
                full[(start + row) * nodes.len() + start + column] =
                    block[row * dimension + column];
            }
        }
    }
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let lock_path = directory.join("uv.lock");
    let worker_path = directory.join("marklab_pymc_replicated_arbitrary_window_lgcp_worker.py");
    let lock = read(&lock_path)?;
    let worker = read(&worker_path)?;
    Ok(WorkerRequest {
        format: "marklab.pymc_replicated_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "pymc",
            version: PYMC_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        input_sha256: sha256_hex(&bytes),
        reference_group: arguments.reference_group,
        comparison_group: arguments.comparison_group,
        patients,
        patterns,
        nodes,
        cholesky: full,
        priors: Priors {
            intercept_mean: arguments.intercept_prior_mean,
            intercept_sd: arguments.intercept_prior_sd,
            group_effect_sd: arguments.group_effect_prior_sd,
            covariate_effect_sd: arguments.covariate_effect_prior_sd,
            patient_sd_scale: arguments.patient_sd_prior_scale,
            pattern_sd_scale: arguments.pattern_sd_prior_scale,
            field_amplitude: arguments.field_amplitude,
            field_length_scale_um: arguments.field_length_scale_um,
            jitter: arguments.jitter,
        },
        sampling,
        diagnostic_policy: DiagnosticPolicy::default(),
        resources: Resources {
            maximum_patients: arguments.maximum_patients,
            maximum_patterns: arguments.maximum_patterns,
            maximum_nodes_per_pattern: arguments.maximum_nodes_per_pattern,
            maximum_total_nodes: arguments.maximum_total_nodes,
            maximum_total_events: arguments.maximum_total_events,
            maximum_draw_node_work: arguments.maximum_draw_node_work,
            draw_node_work: work,
            timeout_seconds: arguments.timeout_seconds,
        },
    })
}

fn validate_controls(arguments: &Arguments, rows: &[CsvRow]) -> Result<(), BayesCliError> {
    let scales = [
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.group_effect_prior_sd,
        arguments.covariate_effect_prior_sd,
        arguments.patient_sd_prior_scale,
        arguments.pattern_sd_prior_scale,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
    ];
    if rows.is_empty()
        || arguments.reference_group.is_empty()
        || arguments.reference_group == arguments.comparison_group
        || !scales.into_iter().all(f64::is_finite)
        || scales[1..].iter().any(|value| *value <= 0.0)
        || !(8..=32).contains(&arguments.maximum_patients)
        || !(16..=64).contains(&arguments.maximum_patterns)
        || !(4..=36).contains(&arguments.maximum_nodes_per_pattern)
        || !(64..=512).contains(&arguments.maximum_total_nodes)
        || arguments.maximum_total_events == 0
        || arguments.maximum_draw_node_work == 0
        || arguments.timeout_seconds == 0
        || arguments.timeout_seconds > 3600
    {
        return Err(BayesCliError::Input(
            "replicated LGCP controls are invalid".into(),
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        if row.pattern_id.is_empty()
            || row.patient_id.is_empty()
            || row.group.is_empty()
            || row.cohort.is_empty()
            || row.node_id.is_empty()
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
                && rows[index - 1].node_id == row.node_id)
        {
            return Err(BayesCliError::Input(
                "replicated LGCP row is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn cholesky_block(
    rows: &[CsvRow],
    amplitude: f64,
    length: f64,
    jitter: f64,
) -> Result<Vec<f64>, BayesCliError> {
    let n = rows.len();
    let mut covariance = vec![0.0; n * n];
    for row in 0..n {
        for column in 0..n {
            let scaled = 3.0_f64.sqrt()
                * (rows[row].x_um - rows[column].x_um).hypot(rows[row].y_um - rows[column].y_um)
                / length;
            covariance[row * n + column] = amplitude.powi(2) * (1.0 + scaled) * (-scaled).exp()
                + if row == column { jitter } else { 0.0 };
        }
    }
    let mut lower = vec![0.0; n * n];
    for row in 0..n {
        for column in 0..=row {
            let mut value = covariance[row * n + column];
            for inner in 0..column {
                value -= lower[row * n + inner] * lower[column * n + inner];
            }
            if row == column {
                if value <= 0.0 || !value.is_finite() {
                    return Err(BayesCliError::Input(
                        "replicated LGCP covariance is invalid".into(),
                    ));
                }
                lower[row * n + column] = value.sqrt();
            } else {
                lower[row * n + column] = value / lower[column * n + column];
            }
        }
    }
    Ok(lower)
}

fn validate_result(
    result: &WorkerResult,
    request: &WorkerRequest,
    request_sha256: &str,
) -> Result<(), BayesCliError> {
    let policy = &request.diagnostic_policy;
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && result.diagnostics.divergences <= policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
    ];
    let patient_effects_valid =
        result
            .patient_effects
            .iter()
            .zip(&request.patients)
            .all(|(effect, patient)| {
                effect.patient_id == patient.patient_id && summary_valid(&effect.effect)
            });
    let pattern_effects_valid =
        result
            .pattern_effects
            .iter()
            .zip(&request.patterns)
            .all(|(effect, pattern)| {
                effect.pattern_id == pattern.pattern_id && summary_valid(&effect.effect)
            });
    let predictive_valid = predictive_valid(
        &result.pattern_posterior_predictive,
        request,
        result.sampling.completed_draws,
    );
    let nodes_valid = result.nodes.len() == request.nodes.len()
        && result.nodes.iter().zip(&request.nodes).all(|(row, node)| {
            row.pattern_id == node.pattern_id
                && row.node_id == node.node_id
                && summary_valid(&row.latent_effect)
                && summary_valid(&row.expected_count)
                && row.expected_count.mean > 0.0
        });
    if result.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"
        || result.version != 2
        || result.backend.name != request.backend.name
        || result.backend.version != request.backend.version
        || result.backend.python_version != request.backend.python_version
        || result.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || result.backend.worker_sha256 != request.backend.worker_sha256
        || result.input_sha256 != request.input_sha256
        || result.request_sha256 != request_sha256
        || result.sampling.completed_draws
            != u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain)
        || result.patient_effects.len() != request.patients.len()
        || result.pattern_effects.len() != request.patterns.len()
        || summaries.iter().any(|summary| !summary_valid(summary))
        || !patient_effects_valid
        || !pattern_effects_valid
        || !predictive_valid
        || !nodes_valid
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "replicated LGCP result differs".into(),
        ));
    }
    Ok(())
}

impl ResultDocument {
    pub(crate) fn validate_for(
        &self,
        prepared: &PreparedReplicatedArbitraryWindowLgcpFit,
    ) -> Result<(), BayesCliError> {
        let request = &prepared.request;
        let policy = &request.diagnostic_policy;
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
            && self.diagnostics.divergences <= policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
        let patient_effects_valid =
            self.patient_effects
                .iter()
                .zip(&request.patients)
                .all(|(effect, patient)| {
                    effect.patient_id == patient.patient_id && summary_valid(&effect.effect)
                });
        let pattern_effects_valid =
            self.pattern_effects
                .iter()
                .zip(&request.patterns)
                .all(|(effect, pattern)| {
                    effect.pattern_id == pattern.pattern_id && summary_valid(&effect.effect)
                });
        let predictive_valid = predictive_valid(
            &self.pattern_posterior_predictive,
            request,
            self.sampling.completed_draws,
        );
        let nodes_valid = self.nodes.len() == request.nodes.len()
            && self.nodes.iter().zip(&request.nodes).all(|(row, node)| {
                row.pattern_id == node.pattern_id
                    && row.node_id == node.node_id
                    && summary_valid(&row.latent_effect)
                    && summary_valid(&row.expected_count)
                    && row.expected_count.mean > 0.0
            });
        if self.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"
            || self.version != 2
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.input_sha256 != request.input_sha256
            || self.request_sha256 != prepared.request_sha256
            || self.patient_count != request.patients.len()
            || self.pattern_count != request.patterns.len()
            || self.total_node_count != request.nodes.len()
            || self.total_event_count != request.nodes.iter().map(|node| node.count).sum::<u64>()
            || self.patient_effects.len() != request.patients.len()
            || self.pattern_effects.len() != request.patterns.len()
            || !patient_effects_valid
            || !pattern_effects_valid
            || !predictive_valid
            || !nodes_valid
            || self.statistical_unit != "patient"
            || self.pattern_unit != "slide_pattern_nested_in_patient"
            || (self.fit_state == FitState::Complete) != diagnostics_pass
        {
            return Err(BayesCliError::Backend(
                "replicated LGCP durable result identity or diagnostics differ".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn predictive_valid(
    rows: &[PatternPosteriorPredictive],
    request: &WorkerRequest,
    completed_draws: u64,
) -> bool {
    rows.len() == request.patterns.len()
        && rows.iter().zip(&request.patterns).all(|(row, pattern)| {
            let finite = [
                row.replicated_total_count_mean,
                row.replicated_total_count_sd,
                row.replicated_total_count_interval_lower,
                row.replicated_total_count_interval_upper,
                row.total_count_two_sided_tail_probability,
                row.observed_node_count_variance,
                row.replicated_node_count_variance_mean,
                row.node_variance_two_sided_tail_probability,
            ]
            .into_iter()
            .all(f64::is_finite);
            row.pattern_id == pattern.pattern_id
                && row.observed_total_count == pattern.event_count
                && row.replicate_count == completed_draws
                && finite
                && row.replicated_total_count_sd >= 0.0
                && row.replicated_total_count_interval_lower
                    <= row.replicated_total_count_interval_upper
                && row.observed_node_count_variance >= 0.0
                && row.replicated_node_count_variance_mean >= 0.0
                && (0.0..=1.0).contains(&row.total_count_two_sided_tail_probability)
                && (0.0..=1.0).contains(&row.node_variance_two_sided_tail_probability)
        })
}

fn summary_valid(summary: &SarScalarSummary) -> bool {
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
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}
