use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json, replicated_arbitrary_window_lgcp_agreement::backend_matches, run_worker,
    BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";
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
    FitJointReplicatedLocationMark(Box<Args>),
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) location_input: PathBuf,
    #[arg(long)]
    pub(crate) mark_input: PathBuf,
    #[arg(long)]
    pub(crate) reference_group: String,
    #[arg(long)]
    pub(crate) comparison_group: String,
    #[arg(long)]
    pub(crate) reference_type: String,
    #[arg(long)]
    pub(crate) neighbor_radius_um: f64,
    #[arg(long)]
    pub(crate) location_prior_sd: f64,
    #[arg(long)]
    pub(crate) group_prior_sd: f64,
    #[arg(long)]
    pub(crate) patient_ecology_prior_scale: f64,
    #[arg(long)]
    pub(crate) mark_intercept_prior_sd: f64,
    #[arg(long)]
    pub(crate) mark_group_prior_sd: f64,
    #[arg(long)]
    pub(crate) mark_loading_prior_sd: f64,
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
    pub(crate) maximum_patients: usize,
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
    #[arg(long)]
    pub(crate) maximum_types: usize,
    #[arg(long)]
    pub(crate) maximum_location_rows: usize,
    #[arg(long)]
    pub(crate) maximum_mark_points: usize,
    #[arg(long)]
    pub(crate) maximum_neighbor_visits: u64,
    #[arg(long)]
    pub(crate) maximum_draw_observation_work: u64,
    #[arg(long)]
    pub(crate) maximum_working_bytes: u64,
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Priors {
    location_sd: f64,
    group_sd: f64,
    patient_ecology_scale: f64,
    mark_intercept_sd: f64,
    mark_group_sd: f64,
    mark_loading_sd: f64,
    interaction_sd: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Sampling {
    chains: u32,
    tune: u32,
    draws: u32,
    target_accept: f64,
    seed: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_types: usize,
    maximum_location_rows: usize,
    maximum_mark_points: usize,
    maximum_neighbor_visits: u64,
    neighbor_visits: u64,
    maximum_draw_observation_work: u64,
    draw_observation_work: u64,
    maximum_working_bytes: u64,
    estimated_working_bytes: u64,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
    maximum_output_bytes: usize,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    location_csv: &'a str,
    mark_csv: &'a str,
    location_sha256: &'a str,
    mark_sha256: &'a str,
    reference_group: &'a str,
    comparison_group: &'a str,
    reference_type: &'a str,
    neighbor_radius_um: f64,
    priors: Priors,
    sampling: Sampling,
    resources: Resources,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    mean: f64,
    sd: f64,
    interval_lower: f64,
    interval_upper: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointPosterior {
    patient_ecology_sd: Summary,
    mark_loadings: Vec<Summary>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PredictiveScore {
    mean_log_predictive_density: f64,
    uncertainty: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HeldoutComparison {
    joint: PredictiveScore,
    location_only: PredictiveScore,
    conditional_mark_only: PredictiveScore,
    joint_minus_separate_baselines: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PpcMetric {
    observed: f64,
    replicated_mean: f64,
    two_sided_tail_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PosteriorPredictive {
    total_count: PpcMetric,
    mark_proportions: PpcMetric,
    cross_type_enrichment: PpcMetric,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diagnostics {
    maximum_r_hat: f64,
    minimum_bulk_ess: f64,
    minimum_tail_ess: f64,
    minimum_ebfmi: f64,
    divergences: u64,
    max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    location_sha256: String,
    mark_sha256: String,
    patient_count: usize,
    pattern_count: usize,
    training_pattern_count: usize,
    heldout_pattern_count: usize,
    type_ids: Vec<String>,
    training_pattern_ids: Vec<String>,
    heldout_pattern_ids: Vec<String>,
    joint_posterior: JointPosterior,
    heldout_comparison: HeldoutComparison,
    posterior_predictive: PosteriorPredictive,
    diagnostics: Diagnostics,
    fit_state: FitState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    location_sha256: String,
    mark_sha256: String,
    patient_count: usize,
    pattern_count: usize,
    training_pattern_count: usize,
    heldout_pattern_count: usize,
    type_ids: Vec<String>,
    training_pattern_ids: Vec<String>,
    heldout_pattern_ids: Vec<String>,
    joint_posterior: JointPosterior,
    heldout_comparison: HeldoutComparison,
    posterior_predictive: PosteriorPredictive,
    diagnostics: Diagnostics,
    fit_state: FitState,
    sampling: Sampling,
    resources: Resources,
    statistical_unit: String,
    location_likelihood: String,
    mark_likelihood: String,
    holdout_policy: String,
    null_model: String,
    assumptions: Vec<String>,
    failure_policy: String,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct Prepared {
    backend: BackendContract,
    request_bytes: Vec<u8>,
    request_sha256: String,
    location_sha256: String,
    mark_sha256: String,
    sampling: Sampling,
    resources: Resources,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::FitJointReplicatedLocationMark(args) = command;
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, BayesCliError> {
    let location_bytes = read(&args.location_input)?;
    let mark_bytes = read(&args.mark_input)?;
    let location_csv = String::from_utf8(location_bytes)
        .map_err(|_| BayesCliError::Input("location CSV is not UTF-8".into()))?;
    let mark_csv = String::from_utf8(mark_bytes)
        .map_err(|_| BayesCliError::Input("mark CSV is not UTF-8".into()))?;
    let location_rows = location_csv.lines().count().saturating_sub(1);
    let mark_rows = mark_csv.lines().count().saturating_sub(1);
    let mut pattern_counts = BTreeMap::<String, u64>::new();
    let mut reader = csv::Reader::from_reader(mark_csv.as_bytes());
    #[derive(Deserialize)]
    struct MarkRow {
        pattern_id: String,
    }
    for row in reader.deserialize::<MarkRow>() {
        let row = row?;
        *pattern_counts.entry(row.pattern_id).or_default() += 1;
    }
    let neighbor_visits = pattern_counts
        .values()
        .try_fold(0_u64, |total, count| {
            count
                .checked_mul(count.saturating_sub(1))
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| BayesCliError::Input("neighbor work overflows".into()))?;
    let draw_observation_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul((location_rows + mark_rows) as u64))
        .and_then(|value| value.checked_mul(3))
        .ok_or_else(|| BayesCliError::Input("joint draw work overflows".into()))?;
    let estimated_working_bytes = (location_rows as u64)
        .checked_mul(256)
        .and_then(|value| value.checked_add((mark_rows as u64).checked_mul(512)?))
        .and_then(|value| value.checked_add(neighbor_visits.checked_mul(16)?))
        .and_then(|value| value.checked_add(384 * 1024 * 1024))
        .ok_or_else(|| BayesCliError::Input("joint memory estimate overflows".into()))?;
    let priors = Priors {
        location_sd: args.location_prior_sd,
        group_sd: args.group_prior_sd,
        patient_ecology_scale: args.patient_ecology_prior_scale,
        mark_intercept_sd: args.mark_intercept_prior_sd,
        mark_group_sd: args.mark_group_prior_sd,
        mark_loading_sd: args.mark_loading_prior_sd,
        interaction_sd: args.interaction_prior_sd,
    };
    let sampling = Sampling {
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    if priors
        .clone()
        .into_values()
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
        || !args.neighbor_radius_um.is_finite()
        || args.neighbor_radius_um <= 0.0
        || location_rows > args.maximum_location_rows
        || mark_rows > args.maximum_mark_points
        || neighbor_visits > args.maximum_neighbor_visits
        || draw_observation_work > args.maximum_draw_observation_work
        || estimated_working_bytes > args.maximum_working_bytes
        || !(10..=14).contains(&args.maximum_tree_depth)
    {
        return Err(BayesCliError::Input(
            "joint location-mark controls or resources differ".into(),
        ));
    }
    let resources = Resources {
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_types: args.maximum_types,
        maximum_location_rows: args.maximum_location_rows,
        maximum_mark_points: args.maximum_mark_points,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        neighbor_visits,
        maximum_draw_observation_work: args.maximum_draw_observation_work,
        draw_observation_work,
        maximum_working_bytes: args.maximum_working_bytes,
        estimated_working_bytes,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        maximum_output_bytes: 4 * 1024 * 1024,
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workers = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&workers.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(
            &workers.join("marklab_numpyro_joint_replicated_location_mark_worker.py"),
        )?),
    };
    let location_sha256 = sha256_hex(location_csv.as_bytes());
    let mark_sha256 = sha256_hex(mark_csv.as_bytes());
    let request = WorkerRequest {
        format: "marklab.numpyro_joint_replicated_location_mark_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        location_csv: &location_csv,
        mark_csv: &mark_csv,
        location_sha256: &location_sha256,
        mark_sha256: &mark_sha256,
        reference_group: &args.reference_group,
        comparison_group: &args.comparison_group,
        reference_type: &args.reference_type,
        neighbor_radius_um: args.neighbor_radius_um,
        priors,
        sampling: sampling.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        backend,
        request_bytes,
        request_sha256,
        location_sha256,
        mark_sha256,
        sampling,
        resources,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_numpyro_joint_replicated_location_mark_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, prepared)?;
    Ok(Output {
        format: "marklab.joint_replicated_location_mark_fit".into(),
        version: 1,
        backend: result.backend,
        jax_version: result.jax_version,
        request_sha256: result.request_sha256,
        location_sha256: result.location_sha256,
        mark_sha256: result.mark_sha256,
        patient_count: result.patient_count,
        pattern_count: result.pattern_count,
        training_pattern_count: result.training_pattern_count,
        heldout_pattern_count: result.heldout_pattern_count,
        type_ids: result.type_ids,
        training_pattern_ids: result.training_pattern_ids,
        heldout_pattern_ids: result.heldout_pattern_ids,
        joint_posterior: result.joint_posterior,
        heldout_comparison: result.heldout_comparison,
        posterior_predictive: result.posterior_predictive,
        diagnostics: result.diagnostics,
        fit_state: result.fit_state,
        sampling: prepared.sampling.clone(),
        resources: prepared.resources.clone(),
        statistical_unit: "patient".into(),
        location_likelihood: "exact_window_quadrature_poisson_total_location_intensity".into(),
        mark_likelihood: "conditional_categorical_mark_on_fixed_physical_radius_graph".into(),
        holdout_policy:
            "lexicographically_first_pattern_train_second_pattern_evaluation_per_patient".into(),
        null_model: "zero_shared_patient_ecology_loading_and_zero_group_effects".into(),
        assumptions: vec![
            "each_patient_has_exactly_two_provenance_matched_patterns".into(),
            "location_and_mark_tables_share_patient_pattern_group_and_type_identity".into(),
            "shared_patient_ecology_is_a_statistical_coupling_not_a_biological_mechanism".into(),
            "evaluation_patterns_are_not_used_by_any_of_the_three_fits".into(),
            "patients_not_patterns_nodes_cells_or_edges_are_population_replicates".into(),
        ],
        failure_policy: "backend_failure_or_nonfinite_result_stops_publication".into(),
        finite_result_policy: "reject_non_finite_posterior_predictive_or_heldout_summaries".into(),
        claim_status: if result.fit_state == FitState::Complete {
            "joint_spatial_association_not_communication_or_causality"
        } else {
            "diagnostic_only_nonconverged_joint_location_mark"
        }
        .into(),
    })
}

impl Output {
    pub(crate) fn validate_for(&self, prepared: &Prepared) -> Result<(), BayesCliError> {
        if self.format != "marklab.joint_replicated_location_mark_fit"
            || self.version != 1
            || self.request_sha256 != prepared.request_sha256
            || self.location_sha256 != prepared.location_sha256
            || self.mark_sha256 != prepared.mark_sha256
            || self.sampling != prepared.sampling
            || self.resources != prepared.resources
            || self.statistical_unit != "patient"
            || self.claim_status
                != if self.fit_state == FitState::Complete {
                    "joint_spatial_association_not_communication_or_causality"
                } else {
                    "diagnostic_only_nonconverged_joint_location_mark"
                }
        {
            return Err(BayesCliError::Backend(
                "durable joint location-mark result differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_worker(result: &WorkerResult, prepared: &Prepared) -> Result<(), BayesCliError> {
    let summaries = [
        &result.joint_posterior.patient_ecology_sd,
        &result.heldout_comparison.joint.uncertainty,
        &result.heldout_comparison.location_only.uncertainty,
        &result.heldout_comparison.conditional_mark_only.uncertainty,
        &result.heldout_comparison.joint_minus_separate_baselines,
    ];
    let finite = summaries.iter().all(|row| {
        [row.mean, row.sd, row.interval_lower, row.interval_upper]
            .into_iter()
            .all(f64::is_finite)
            && row.sd >= 0.0
            && row.interval_lower <= row.interval_upper
    }) && [
        &result.posterior_predictive.total_count,
        &result.posterior_predictive.mark_proportions,
        &result.posterior_predictive.cross_type_enrichment,
    ]
    .iter()
    .all(|row| {
        [
            row.observed,
            row.replicated_mean,
            row.two_sided_tail_probability,
        ]
        .into_iter()
        .all(f64::is_finite)
            && (0.0..=1.0).contains(&row.two_sided_tail_probability)
    });
    if result.format != "marklab.numpyro_joint_replicated_location_mark_result"
        || result.version != 1
        || !backend_matches(&result.backend, &prepared.backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != prepared.request_sha256
        || result.location_sha256 != prepared.location_sha256
        || result.mark_sha256 != prepared.mark_sha256
        || result.patient_count < 2
        || result.pattern_count != result.patient_count * 2
        || result.training_pattern_count != result.patient_count
        || result.heldout_pattern_count != result.patient_count
        || result.type_ids.len() < 2
        || (result.fit_state == FitState::Complete)
            != (result.diagnostics.maximum_r_hat <= 1.01
                && result.diagnostics.minimum_bulk_ess >= 100.0
                && result.diagnostics.minimum_tail_ess >= 100.0
                && result.diagnostics.minimum_ebfmi >= 0.3
                && result.diagnostics.divergences == 0
                && result.diagnostics.max_tree_depth_hits == 0)
        || !finite
    {
        return Err(BayesCliError::Backend(
            "joint location-mark backend result differs".into(),
        ));
    }
    Ok(())
}

impl Priors {
    fn into_values(self) -> [f64; 7] {
        [
            self.location_sd,
            self.group_sd,
            self.patient_ecology_scale,
            self.mark_intercept_sd,
            self.mark_group_sd,
            self.mark_loading_sd,
            self.interaction_sd,
        ]
    }
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "input must be a regular file within 16 MiB: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
