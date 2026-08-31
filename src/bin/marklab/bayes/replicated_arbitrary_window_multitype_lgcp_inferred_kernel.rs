use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
    replicated_arbitrary_window_multitype_lgcp::{
        self, hierarchy_effects_valid, pair_differences_valid, summary_valid,
        type_posteriors_valid, Args as SourceArgs, HierarchyEffect, NodeTypePosterior,
        PairDifference, PatternTypePredictive, TypePosterior,
    },
    run_worker, BayesCliError,
};

const PYMC_VERSION: &str = "6.3.0";
const MAXIMUM_ADAPTER_BYTES: u64 = 8 * 1_048_576;

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
    FitReplicatedArbitraryWindowMultitypeLgcpInferredKernel(Box<Args>),
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) reference_group: String,
    #[arg(long)]
    pub(crate) comparison_group: String,
    #[arg(long)]
    pub(crate) reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(crate) covariate_effect_prior_sd: f64,
    #[arg(long)]
    pub(crate) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(crate) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(crate) field_amplitude_prior_scale: f64,
    #[arg(long)]
    pub(crate) field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    pub(crate) jitter: f64,
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
    pub(crate) maximum_nodes_per_pattern: usize,
    #[arg(long)]
    pub(crate) maximum_total_nodes: usize,
    #[arg(long)]
    pub(crate) maximum_total_node_type_rows: usize,
    #[arg(long)]
    pub(crate) maximum_total_events: u64,
    #[arg(long)]
    pub(crate) maximum_draw_node_type_work: u64,
    #[arg(long)]
    pub(crate) maximum_kernel_cube_work: u64,
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KernelPriors {
    field_amplitude_scale: f64,
    field_length_scale_scale_um: f64,
    jitter: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KernelResources {
    maximum_kernel_cube_work: u64,
    kernel_cube_work: u64,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KernelPosterior {
    field_amplitude: SarScalarSummary,
    field_length_scale_um: SarScalarSummary,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    source_backend: BackendContract,
    source_request_sha256: &'a str,
    source_request: serde_json::Value,
    kernel_priors: KernelPriors,
    resources: KernelResources,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    kernel_posterior: KernelPosterior,
    type_posteriors: Vec<TypePosterior>,
    group_effect_differences: Vec<PairDifference>,
    patient_type_effects: Vec<HierarchyEffect>,
    pattern_type_effects: Vec<HierarchyEffect>,
    node_type_posteriors: Vec<NodeTypePosterior>,
    pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    diagnostics: NormalMeanDiagnostics,
    kernel_cube_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    kernel_posterior: KernelPosterior,
    type_posteriors: Vec<TypePosterior>,
    group_effect_differences: Vec<PairDifference>,
    patient_type_effects: Vec<HierarchyEffect>,
    pattern_type_effects: Vec<HierarchyEffect>,
    node_type_posteriors: Vec<NodeTypePosterior>,
    pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    diagnostics: NormalMeanDiagnostics,
    patient_count: usize,
    pattern_count: usize,
    type_count: usize,
    total_node_count: usize,
    total_node_type_row_count: usize,
    total_event_count: u64,
    groups: [String; 2],
    reference_type: String,
    cohort_count: usize,
    kernel_priors: KernelPriors,
    resources: KernelResources,
    statistical_unit: String,
    pattern_unit: String,
    likelihood: String,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct Prepared {
    source: replicated_arbitrary_window_multitype_lgcp::Prepared,
    source_value: serde_json::Value,
    backend: BackendContract,
    kernel_priors: KernelPriors,
    resources: KernelResources,
    request_bytes: Vec<u8>,
    request_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn backend(&self) -> &BackendContract {
        &self.backend
    }

    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

impl Output {
    pub(crate) fn validate_for(&self, prepared: &Prepared) -> Result<(), BayesCliError> {
        let mut value = serde_json::to_value(self)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| BayesCliError::Backend("inferred-kernel result differs".into()))?;
        for name in [
            "patient_count",
            "pattern_count",
            "type_count",
            "total_node_count",
            "total_node_type_row_count",
            "total_event_count",
            "groups",
            "reference_type",
            "cohort_count",
            "kernel_priors",
            "resources",
            "statistical_unit",
            "pattern_unit",
            "likelihood",
            "null_model",
            "assumptions",
            "finite_result_policy",
            "claim_status",
        ] {
            object.remove(name);
        }
        object.insert(
            "kernel_cube_work".into(),
            prepared.resources.kernel_cube_work.into(),
        );
        let result: WorkerResult = serde_json::from_value(value)?;
        validate_result(&result, prepared)?;
        let source = &prepared.source_value;
        if self.patient_count != array(source, "patients")?.len()
            || self.pattern_count != array(source, "patterns")?.len()
            || self.type_count != array(source, "type_ids")?.len()
            || self.total_node_count != array(source, "nodes")?.len()
            || self.total_node_type_row_count != array(source, "node_type_counts")?.len()
            || self.statistical_unit != "patient"
            || self.pattern_unit != "slide_pattern_nested_in_patient"
        {
            return Err(BayesCliError::Backend(
                "durable replicated multitype inferred-kernel metadata differs".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::FitReplicatedArbitraryWindowMultitypeLgcpInferredKernel(args) = command;
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, BayesCliError> {
    if ![
        args.field_amplitude_prior_scale,
        args.field_length_scale_prior_scale_um,
        args.jitter,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value > 0.0)
        || args.maximum_kernel_cube_work == 0
        || args.maximum_kernel_cube_work > 1_000_000
        || !(10..=14).contains(&args.maximum_tree_depth)
    {
        return Err(BayesCliError::Input(
            "replicated multitype inferred-kernel controls are invalid".into(),
        ));
    }
    let source = replicated_arbitrary_window_multitype_lgcp::prepare(SourceArgs {
        input: args.input,
        reference_group: args.reference_group,
        comparison_group: args.comparison_group,
        reference_type: args.reference_type,
        intercept_prior_mean: args.intercept_prior_mean,
        intercept_prior_sd: args.intercept_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        covariate_effect_prior_sd: args.covariate_effect_prior_sd,
        patient_sd_prior_scale: args.patient_sd_prior_scale,
        pattern_sd_prior_scale: args.pattern_sd_prior_scale,
        field_amplitude: args.field_amplitude_prior_scale,
        field_length_scale_um: args.field_length_scale_prior_scale_um,
        jitter: args.jitter,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_types: args.maximum_types,
        maximum_nodes_per_pattern: args.maximum_nodes_per_pattern,
        maximum_total_nodes: args.maximum_total_nodes,
        maximum_total_node_type_rows: args.maximum_total_node_type_rows,
        maximum_total_events: args.maximum_total_events,
        maximum_draw_node_type_work: args.maximum_draw_node_type_work,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let source_value: serde_json::Value = serde_json::from_slice(source.request_bytes())?;
    let kernel_cube_work = array(&source_value, "patterns")?
        .iter()
        .try_fold(0_u64, |total, row| {
            row["node_count"]
                .as_u64()
                .and_then(|nodes| nodes.checked_pow(3))
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| BayesCliError::Input("multitype kernel work overflows".into()))?;
    if kernel_cube_work > args.maximum_kernel_cube_work {
        return Err(BayesCliError::Input(format!(
            "multitype kernel cube work exceeds maximum: {kernel_cube_work} > {}",
            args.maximum_kernel_cube_work
        )));
    }
    let kernel_priors = KernelPriors {
        field_amplitude_scale: args.field_amplitude_prior_scale,
        field_length_scale_scale_um: args.field_length_scale_prior_scale_um,
        jitter: args.jitter,
    };
    let resources = KernelResources {
        maximum_kernel_cube_work: args.maximum_kernel_cube_work,
        kernel_cube_work,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "pymc",
        version: PYMC_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read_bounded(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_bounded(&directory.join(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        ))?),
    };
    let request = WorkerRequest {
        format: "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request",
        version: 1,
        backend: backend.clone(),
        source_backend: source.backend().clone(),
        source_request_sha256: source.request_sha256(),
        source_request: source_value.clone(),
        kernel_priors: kernel_priors.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        source,
        source_value,
        backend,
        kernel_priors,
        resources,
        request_bytes,
        request_sha256,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_result(&result, prepared)?;
    let source = &prepared.source_value;
    let patients = array(source, "patients")?;
    let patterns = array(source, "patterns")?;
    let types = strings(source, "type_ids")?;
    let nodes = array(source, "nodes")?;
    let counts = array(source, "node_type_counts")?;
    Ok(Output {
        format: result.format,
        version: result.version,
        backend: result.backend,
        source_backend: result.source_backend,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        source_request_sha256: result.source_request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        kernel_posterior: result.kernel_posterior,
        type_posteriors: result.type_posteriors,
        group_effect_differences: result.group_effect_differences,
        patient_type_effects: result.patient_type_effects,
        pattern_type_effects: result.pattern_type_effects,
        node_type_posteriors: result.node_type_posteriors,
        pattern_type_posterior_predictive: result.pattern_type_posterior_predictive,
        diagnostics: result.diagnostics,
        patient_count: patients.len(),
        pattern_count: patterns.len(),
        type_count: types.len(),
        total_node_count: nodes.len(),
        total_node_type_row_count: counts.len(),
        total_event_count: counts.iter().filter_map(|row| row["count"].as_u64()).sum(),
        groups: [
            string(source, "reference_group")?,
            string(source, "comparison_group")?,
        ],
        reference_type: string(source, "reference_type")?,
        cohort_count: patterns
            .iter()
            .filter_map(|row| row["cohort"].as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        kernel_priors: prepared.kernel_priors.clone(),
        resources: prepared.resources.clone(),
        statistical_unit: "patient".into(),
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        likelihood: "independent_type_specific_poisson_intensities_over_one_shared_exact_window"
            .into(),
        null_model: "every_type_comparison_group_log_intensity_effect_equals_zero".into(),
        assumptions: [
            "one_shared_isotropic_matern_kernel_across_types_and_slide_patterns",
            "type_specific_latent_fields_are_conditionally_independent_given_shared_kernel_scales",
            "kernel_scales_are_inferred_inside_the_model_not_selected_post_hoc",
            "each_exact_window_and_typed_event_partition_is_provenance_complete",
            "patients_not_slides_nodes_types_or_cells_are_population_replicates",
            "field_and_slide_effects_are_zero_sum_within_declared_levels",
            "no_pairwise_gibbs_interaction_or_cross_type_latent_covariance_is_claimed",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if result.fit_state == FitState::Complete {
            "experimental_inferred_shared_multitype_kernel"
        } else {
            "diagnostic_only_nonconverged"
        }
        .into(),
    })
}

fn validate_result(result: &WorkerResult, prepared: &Prepared) -> Result<(), BayesCliError> {
    let source = &prepared.source_value;
    let types = strings(source, "type_ids")?;
    let patients = identities(array(source, "patients")?, "patient_id")?;
    let patterns = identities(array(source, "patterns")?, "pattern_id")?;
    let nodes = array(source, "nodes")?;
    let counts = array(source, "node_type_counts")?;
    let policy = DiagnosticPolicy::default();
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
    let nodes_valid = result.node_type_posteriors.len() == counts.len()
        && result
            .node_type_posteriors
            .iter()
            .zip(counts)
            .all(|(row, count)| {
                let Some(node_index) = count["node_index"].as_u64().map(|value| value as usize)
                else {
                    return false;
                };
                let Some(type_index) = count["type_index"].as_u64().map(|value| value as usize)
                else {
                    return false;
                };
                nodes.get(node_index).is_some_and(|node| {
                    row.pattern_id == node["pattern_id"]
                        && row.node_id == node["node_id"]
                        && types.get(type_index) == Some(&row.type_id)
                        && summary_valid(&row.latent_effect)
                        && summary_valid(&row.expected_count)
                        && row.expected_count.mean > 0.0
                })
            });
    let complete_draws = source["sampling"]["chains"].as_u64().unwrap_or(0)
        * source["sampling"]["draws_per_chain"].as_u64().unwrap_or(0);
    if result.format
        != "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit"
        || result.version != 1
        || !backend_matches(&result.backend, &prepared.backend)
        || !backend_matches(&result.source_backend, prepared.source.backend())
        || result.input_sha256 != source["input_sha256"]
        || result.request_sha256 != prepared.request_sha256
        || result.source_request_sha256 != prepared.source.request_sha256()
        || result.sampling.completed_draws != complete_draws
        || result.kernel_cube_work != prepared.resources.kernel_cube_work
        || !summary_valid(&result.kernel_posterior.field_amplitude)
        || !summary_valid(&result.kernel_posterior.field_length_scale_um)
        || result.kernel_posterior.field_amplitude.mean <= 0.0
        || result.kernel_posterior.field_length_scale_um.mean <= 0.0
        || !type_posteriors_valid(&result.type_posteriors, &types)
        || !pair_differences_valid(&result.group_effect_differences, &types)
        || !hierarchy_effects_valid(
            &result.patient_type_effects,
            patients.iter().map(String::as_str),
            &types,
        )
        || !hierarchy_effects_valid(
            &result.pattern_type_effects,
            patterns.iter().map(String::as_str),
            &types,
        )
        || !nodes_valid
        || !predictive_valid(
            &result.pattern_type_posterior_predictive,
            source,
            complete_draws,
        )
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "replicated multitype inferred-kernel result differs".into(),
        ));
    }
    Ok(())
}

pub(crate) fn predictive_valid(
    rows: &[PatternTypePredictive],
    source: &serde_json::Value,
    draws: u64,
) -> bool {
    let Ok(patterns) = array(source, "patterns") else {
        return false;
    };
    let Ok(types) = strings(source, "type_ids") else {
        return false;
    };
    let Ok(nodes) = array(source, "nodes") else {
        return false;
    };
    let Ok(counts) = array(source, "node_type_counts") else {
        return false;
    };
    rows.len() == patterns.len() * types.len()
        && rows.iter().enumerate().all(|(index, row)| {
            let pattern_index = index / types.len();
            let type_index = index % types.len();
            let observed = counts
                .iter()
                .filter(|count| {
                    count["type_index"].as_u64() == Some(type_index as u64)
                        && count["node_index"].as_u64().is_some_and(|node| {
                            nodes[node as usize]["pattern_index"].as_u64()
                                == Some(pattern_index as u64)
                        })
                })
                .filter_map(|count| count["count"].as_u64())
                .sum::<u64>();
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
            row.pattern_id == patterns[pattern_index]["pattern_id"]
                && row.type_id == types[type_index]
                && row.observed_total_count == observed
                && row.replicate_count == draws
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

fn array<'a>(
    value: &'a serde_json::Value,
    name: &str,
) -> Result<&'a [serde_json::Value], BayesCliError> {
    value[name]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| BayesCliError::Input(format!("prepared {name} differ")))
}

fn strings(value: &serde_json::Value, name: &str) -> Result<Vec<String>, BayesCliError> {
    array(value, name)?
        .iter()
        .map(|row| {
            row.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| BayesCliError::Input(format!("prepared {name} differ")))
        })
        .collect()
}

fn identities(rows: &[serde_json::Value], name: &str) -> Result<Vec<String>, BayesCliError> {
    rows.iter()
        .map(|row| {
            row[name]
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| BayesCliError::Input(format!("prepared {name} differs")))
        })
        .collect()
}

fn string(value: &serde_json::Value, name: &str) -> Result<String, BayesCliError> {
    value[name]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| BayesCliError::Input(format!("prepared {name} differs")))
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(
        path,
        MAXIMUM_ADAPTER_BYTES,
        "replicated multitype inferred-kernel adapter is absent or oversized",
    )
}
