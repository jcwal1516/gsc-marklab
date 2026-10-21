pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_fit::{
    validate_result, Node, Patient, Pattern, Priors, Resources, ResultDocument, WorkerRequest,
    WorkerResult,
};

use crate::input_file::read_file;
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

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Per-node quadrature CSV (pattern_id, patient_id, group, node_id, x_um, y_um, count, ...).
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
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
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    pub(crate) maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    pub(crate) maximum_total_nodes: usize,
    /// Upper bound on total events.
    #[arg(long)]
    pub(crate) maximum_total_events: u64,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(crate) maximum_draw_node_work: u64,
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
    x_um: f64,
    y_um: f64,
    weight_um2: f64,
    window_area_um2: f64,
    covariate: f64,
    count: u64,
    window_sha256: String,
    event_sha256: String,
}

pub(crate) struct PreparedReplicatedArbitraryWindowLgcpFit {
    pub(crate) request: WorkerRequest,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) request_sha256: String,
    pub(crate) timeout_seconds: u64,
}

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    let output = arguments.out.clone();
    let prepared = prepare(*arguments)?;
    publish_json(&output, &execute(&prepared)?)
}

pub(crate) fn execute(
    prepared: &PreparedReplicatedArbitraryWindowLgcpFit,
) -> Result<ResultDocument, CliError> {
    let request = &prepared.request;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let bytes = run_worker(
        repository,
        "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let worker: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_result(&worker, request, &prepared.request_sha256).map_err(super::protocol_error)?;
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
        statistical_unit: StatisticalUnit::Patient,
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        null_model: NullHypothesis::ComparisonGroupLogIntensityEffectEqualsZero,
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
            ClaimStatus::ExperimentalReplicatedPatternHierarchy
        } else {
            ClaimStatus::DiagnosticOnlyNonconverged
        },
    })
}

pub(crate) fn prepare(
    arguments: Arguments,
) -> Result<PreparedReplicatedArbitraryWindowLgcpFit, CliError> {
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

fn prepare_request(arguments: Arguments) -> Result<WorkerRequest, CliError> {
    let bytes = fs::read(&arguments.input).map_err(|source| CliError::Io {
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
        return Err(CliError::Input(
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
                return Err(CliError::Input(
                    "replicated LGCP patient group changes across rows".into(),
                ));
            }
            _ => {}
        }
    }
    if patient_groups.len() > arguments.maximum_patients {
        return Err(CliError::Input(
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
                return Err(CliError::Input(
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
        return Err(CliError::Input(
            "replicated LGCP pattern count is invalid".into(),
        ));
    }
    let mut patterns = Vec::with_capacity(grouped.len());
    let mut nodes = Vec::new();
    let mut pattern_counts = vec![0_usize; patients.len()];
    let mut cholesky = Vec::new();
    for (pattern_index, (pattern_id, pattern_rows)) in grouped.into_iter().enumerate() {
        if !(4..=arguments.maximum_nodes_per_pattern).contains(&pattern_rows.len()) {
            return Err(CliError::Input(
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
            return Err(CliError::Input(
                "replicated LGCP pattern identity changes".into(),
            ));
        }
        let event_count = pattern_rows.iter().map(|row| row.count).sum();
        let weight_sum = pattern_rows.iter().map(|row| row.weight_um2).sum::<f64>();
        if (weight_sum - first.window_area_um2).abs() > 1e-10 * first.window_area_um2.abs().max(1.0)
        {
            return Err(CliError::Input(
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
        return Err(CliError::Input(
            "replicated LGCP requires four patients per group and two to four patterns each".into(),
        ));
    }
    let total_events = nodes.iter().map(|node| node.count).sum::<u64>();
    if total_events > arguments.maximum_total_events {
        return Err(CliError::Input(
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
        .ok_or_else(|| CliError::Input("replicated LGCP work overflows".into()))?;
    if work > arguments.maximum_draw_node_work {
        return Err(CliError::Input(
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
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock_path = directory.join("uv.lock");
    let worker_path = directory.join("marklab_pymc_replicated_arbitrary_window_lgcp_worker.py");
    let lock = read_file(&lock_path)?;
    let worker = read_file(&worker_path)?;
    Ok(WorkerRequest {
        format: "marklab.pymc_replicated_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "pymc",
            version: PYMC_VERSION,
            python_version: PYTHON_VERSION,
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

fn validate_controls(arguments: &Arguments, rows: &[CsvRow]) -> Result<(), CliError> {
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
        return Err(CliError::Input(
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
            return Err(CliError::Input("replicated LGCP row is invalid".into()));
        }
    }
    Ok(())
}

fn cholesky_block(
    rows: &[CsvRow],
    amplitude: f64,
    length: f64,
    jitter: f64,
) -> Result<Vec<f64>, CliError> {
    let n = rows.len();
    let mut covariance = vec![0.0; n * n];
    for row in 0..n {
        for column in 0..n {
            let distance =
                (rows[row].x_um - rows[column].x_um).hypot(rows[row].y_um - rows[column].y_um);
            covariance[row * n + column] =
                matern32(distance, amplitude, length) + if row == column { jitter } else { 0.0 };
        }
    }
    dense::cholesky(&covariance, n)
        .ok_or_else(|| CliError::Input("replicated LGCP covariance is invalid".into()))
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl PreparedReplicatedArbitraryWindowLgcpFit {
    pub(crate) fn validate_result(&self, result: &ResultDocument) -> Result<(), CliError> {
        result
            .validate_for(&self.request, &self.request_sha256)
            .map_err(super::protocol_error)
    }
}
