use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    posterior_validation::diagnostics_pass,
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{
        self, Args as FitArgs, Output as FitOutput,
    },
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";
const MAXIMUM_RESULT_BYTES: u64 = 16 * 1_048_576;

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
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelAgreement(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
    #[arg(long)]
    pymc_result: PathBuf,
    #[command(flatten)]
    fit: AgreementFitArgs,
    #[arg(long)]
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    #[arg(long)]
    minimum_field_tolerance: f64,
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct AgreementFitArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
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
    field_amplitude_prior_scale: f64,
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
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
    maximum_types: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
}

impl AgreementFitArgs {
    fn into_fit(self) -> FitArgs {
        FitArgs {
            input: self.input,
            reference_group: self.reference_group,
            comparison_group: self.comparison_group,
            reference_type: self.reference_type,
            intercept_prior_mean: self.intercept_prior_mean,
            intercept_prior_sd: self.intercept_prior_sd,
            group_effect_prior_sd: self.group_effect_prior_sd,
            covariate_effect_prior_sd: self.covariate_effect_prior_sd,
            patient_sd_prior_scale: self.patient_sd_prior_scale,
            pattern_sd_prior_scale: self.pattern_sd_prior_scale,
            field_amplitude_prior_scale: self.field_amplitude_prior_scale,
            field_length_scale_prior_scale_um: self.field_length_scale_prior_scale_um,
            jitter: self.jitter,
            chains: self.chains,
            tune: self.tune,
            draws: self.draws,
            target_accept: self.target_accept,
            seed: self.seed,
            maximum_patients: self.maximum_patients,
            maximum_patterns: self.maximum_patterns,
            maximum_types: self.maximum_types,
            maximum_nodes_per_pattern: self.maximum_nodes_per_pattern,
            maximum_total_nodes: self.maximum_total_nodes,
            maximum_total_node_type_rows: self.maximum_total_node_type_rows,
            maximum_total_events: self.maximum_total_events,
            maximum_draw_node_type_work: self.maximum_draw_node_type_work,
            maximum_kernel_cube_work: self.maximum_kernel_cube_work,
            maximum_tree_depth: self.maximum_tree_depth,
            timeout_seconds: self.timeout_seconds,
            out: PathBuf::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TypeRow {
    type_id: String,
    intercept: SarScalarSummary,
    group_effect: SarScalarSummary,
    covariate_effect: SarScalarSummary,
    patient_sd: SarScalarSummary,
    pattern_sd: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct KernelRow {
    field_amplitude: SarScalarSummary,
    field_length_scale_um: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct DifferenceRow {
    type_a: String,
    type_b: String,
    difference: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct EffectRow {
    owner_id: String,
    type_id: String,
    effect: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct NodeRow {
    pattern_id: String,
    node_id: String,
    type_id: String,
    latent_effect: SarScalarSummary,
    expected_count: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct FitView {
    input_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    kernel_posterior: KernelRow,
    type_posteriors: Vec<TypeRow>,
    group_effect_differences: Vec<DifferenceRow>,
    patient_type_effects: Vec<EffectRow>,
    pattern_type_effects: Vec<EffectRow>,
    node_type_posteriors: Vec<NodeRow>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Deserialize)]
struct NumpyroView {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    maximum_tree_depth: u32,
    fit_state: FitState,
    sampling: SamplingSummary,
    kernel_posterior: KernelRow,
    type_posteriors: Vec<TypeRow>,
    group_effect_differences: Vec<DifferenceRow>,
    patient_type_effects: Vec<EffectRow>,
    pattern_type_effects: Vec<EffectRow>,
    node_type_posteriors: Vec<NodeRow>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Serialize)]
struct VectorAgreement {
    element_count: usize,
    maximum_standardized_difference: f64,
    all_intervals_overlap: bool,
    passes: bool,
}

#[derive(Debug, Serialize)]
struct Comparison {
    type_parameters: VectorAgreement,
    kernel_parameters: VectorAgreement,
    group_effect_differences: VectorAgreement,
    patient_type_effects: VectorAgreement,
    pattern_type_effects: VectorAgreement,
    latent_effects: VectorAgreement,
    expected_counts: VectorAgreement,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
    all_pass: bool,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    pymc: FitOutput,
    numpyro: serde_json::Value,
    comparison: Comparison,
    fit_state: FitState,
    agreement_status: &'static str,
    statistical_unit: &'static str,
    assumptions: [&'static str; 4],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

#[derive(Serialize)]
struct WorkerRequest {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: String,
    source_request: serde_json::Value,
    maximum_tree_depth: u32,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelAgreement(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    for value in [
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesCliError::Input(
                "inferred multitype agreement controls are invalid".into(),
            ));
        }
    }
    if !(10..=14).contains(&args.numpyro_maximum_tree_depth) {
        return Err(BayesCliError::Input(
            "NumPyro inferred multitype tree depth is invalid".into(),
        ));
    }
    let timeout = args.fit.timeout_seconds;
    let prepared =
        replicated_arbitrary_window_multitype_lgcp_inferred_kernel::prepare(args.fit.into_fit())?;
    let pymc: FitOutput = serde_json::from_slice(&read_bounded(&args.pymc_result)?)?;
    pymc.validate_for(&prepared)?;
    let pymc_view: FitView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    let (result, backend, request_sha) =
        execute_numpyro(&prepared, args.numpyro_maximum_tree_depth, timeout)?;
    let numpyro: NumpyroView = serde_json::from_value(result.clone())?;
    validate_numpyro(
        &numpyro,
        &pymc_view,
        &backend,
        &request_sha,
        sha256_hex(prepared.request_bytes()),
        args.numpyro_maximum_tree_depth,
    )?;
    let comparison = compare(
        &pymc_view,
        &numpyro,
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    );
    let complete = pymc_view.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &args.out,
        &Output {
            format:
                "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_backend_agreement",
            version: 1,
            pymc,
            numpyro: result,
            comparison,
            fit_state: if complete { FitState::Complete } else { FitState::Nonconverged },
            agreement_status: if complete {
                "agree_within_monte_carlo_error"
            } else {
                "diagnostic_only_disagreement_or_nonconvergence"
            },
            statistical_unit: "patient",
            assumptions: [
                "same_exact_patients_patterns_windows_types_priors_draws_target_accept_and_seed",
                "numpyro_tree_depth_capacity_is_explicit_and_sampler_specific",
                "independent_dynamic_block_cholesky_hierarchy_and_likelihood_implementations",
                "patients_not_patterns_nodes_types_or_cells_are_population_replicates",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: "experimental_cross_backend_validation",
        },
    )
}

fn execute_numpyro(
    prepared: &replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Prepared,
    depth: u32,
    timeout: u64,
) -> Result<(serde_json::Value, BackendContract, String), BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workers = repository.join("workers/python");
    let worker_name =
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py";
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&fs::read(workers.join("uv.lock")).map_err(
            |source| BayesCliError::Io {
                path: workers.join("uv.lock"),
                source,
            },
        )?),
        worker_sha256: sha256_hex(&fs::read(workers.join(worker_name)).map_err(|source| {
            BayesCliError::Io {
                path: workers.join(worker_name),
                source,
            }
        })?),
    };
    let request = WorkerRequest {
        format:
            "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: sha256_hex(prepared.request_bytes()),
        source_request: serde_json::from_slice(prepared.request_bytes())?,
        maximum_tree_depth: depth,
    };
    let bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&bytes);
    let output = run_worker(repository, worker_name, &bytes, timeout)?;
    Ok((serde_json::from_slice(&output)?, backend, request_sha))
}

#[allow(clippy::too_many_arguments)]
fn validate_numpyro(
    result: &NumpyroView,
    baseline: &FitView,
    backend: &BackendContract,
    request_sha: &str,
    source_sha: String,
    depth: u32,
) -> Result<(), BayesCliError> {
    let complete = diagnostics_pass(&result.diagnostics, &DiagnosticPolicy::default());
    if result.format
        != "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != baseline.input_sha256
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || result.maximum_tree_depth != depth
        || result.sampling.completed_draws != baseline.sampling.completed_draws
        || !rows_match(baseline, result)
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "NumPyro inferred multitype LGCP result differs".into(),
        ));
    }
    Ok(())
}

fn rows_match(left: &FitView, right: &NumpyroView) -> bool {
    left.type_posteriors.len() == right.type_posteriors.len()
        && left
            .type_posteriors
            .iter()
            .zip(&right.type_posteriors)
            .all(|(a, b)| a.type_id == b.type_id)
        && left.group_effect_differences.len() == right.group_effect_differences.len()
        && left
            .group_effect_differences
            .iter()
            .zip(&right.group_effect_differences)
            .all(|(a, b)| a.type_a == b.type_a && a.type_b == b.type_b)
        && effects_match(&left.patient_type_effects, &right.patient_type_effects)
        && effects_match(&left.pattern_type_effects, &right.pattern_type_effects)
        && left.node_type_posteriors.len() == right.node_type_posteriors.len()
        && left
            .node_type_posteriors
            .iter()
            .zip(&right.node_type_posteriors)
            .all(|(a, b)| {
                a.pattern_id == b.pattern_id && a.node_id == b.node_id && a.type_id == b.type_id
            })
}

fn effects_match(left: &[EffectRow], right: &[EffectRow]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.owner_id == b.owner_id && a.type_id == b.type_id)
}

fn compare(
    left: &FitView,
    right: &NumpyroView,
    maximum: f64,
    parameter_tolerance: f64,
    field_tolerance: f64,
) -> Comparison {
    let left_ess = left.diagnostics.ess_bulk;
    let right_ess = right.diagnostics.ess_bulk;
    let type_parameters = compare_vector(
        left.type_posteriors.iter().flat_map(type_summaries),
        right.type_posteriors.iter().flat_map(type_summaries),
        left_ess,
        right_ess,
        maximum,
        parameter_tolerance,
    );
    let kernel_parameters = compare_vector(
        [
            &left.kernel_posterior.field_amplitude,
            &left.kernel_posterior.field_length_scale_um,
        ]
        .into_iter(),
        [
            &right.kernel_posterior.field_amplitude,
            &right.kernel_posterior.field_length_scale_um,
        ]
        .into_iter(),
        left_ess,
        right_ess,
        maximum,
        parameter_tolerance,
    );
    let group_effect_differences = compare_vector(
        left.group_effect_differences
            .iter()
            .map(|row| &row.difference),
        right
            .group_effect_differences
            .iter()
            .map(|row| &row.difference),
        left_ess,
        right_ess,
        maximum,
        parameter_tolerance,
    );
    let patient_type_effects = compare_vector(
        left.patient_type_effects.iter().map(|row| &row.effect),
        right.patient_type_effects.iter().map(|row| &row.effect),
        left_ess,
        right_ess,
        maximum,
        parameter_tolerance,
    );
    let pattern_type_effects = compare_vector(
        left.pattern_type_effects.iter().map(|row| &row.effect),
        right.pattern_type_effects.iter().map(|row| &row.effect),
        left_ess,
        right_ess,
        maximum,
        parameter_tolerance,
    );
    let latent_effects = compare_vector(
        left.node_type_posteriors
            .iter()
            .map(|row| &row.latent_effect),
        right
            .node_type_posteriors
            .iter()
            .map(|row| &row.latent_effect),
        left_ess,
        right_ess,
        maximum,
        field_tolerance,
    );
    let expected_counts = compare_vector(
        left.node_type_posteriors
            .iter()
            .map(|row| &row.expected_count),
        right
            .node_type_posteriors
            .iter()
            .map(|row| &row.expected_count),
        left_ess,
        right_ess,
        maximum,
        field_tolerance,
    );
    let all_pass = [
        &type_parameters,
        &kernel_parameters,
        &group_effect_differences,
        &patient_type_effects,
        &pattern_type_effects,
        &latent_effects,
        &expected_counts,
    ]
    .into_iter()
    .all(|row| row.passes);
    Comparison {
        type_parameters,
        kernel_parameters,
        group_effect_differences,
        patient_type_effects,
        pattern_type_effects,
        latent_effects,
        expected_counts,
        maximum_standardized_difference: maximum,
        minimum_parameter_tolerance: parameter_tolerance,
        minimum_field_tolerance: field_tolerance,
        all_pass,
    }
}

fn type_summaries(row: &TypeRow) -> [&SarScalarSummary; 5] {
    [
        &row.intercept,
        &row.group_effect,
        &row.covariate_effect,
        &row.patient_sd,
        &row.pattern_sd,
    ]
}

fn compare_vector<'a>(
    left: impl Iterator<Item = &'a SarScalarSummary>,
    right: impl Iterator<Item = &'a SarScalarSummary>,
    left_ess: f64,
    right_ess: f64,
    maximum: f64,
    tolerance: f64,
) -> VectorAgreement {
    let mut element_count = 0;
    let mut maximum_standardized_difference = 0.0_f64;
    let mut all_intervals_overlap = true;
    for (left, right) in left.zip(right) {
        element_count += 1;
        let combined =
            ((left.sd / left_ess.sqrt()).powi(2) + (right.sd / right_ess.sqrt()).powi(2)).sqrt();
        maximum_standardized_difference = maximum_standardized_difference
            .max((left.mean - right.mean).abs() / combined.max(tolerance));
        all_intervals_overlap &= left.interval_lower <= right.interval_upper
            && right.interval_lower <= left.interval_upper;
    }
    VectorAgreement {
        element_count,
        maximum_standardized_difference,
        all_intervals_overlap,
        passes: maximum_standardized_difference <= maximum && all_intervals_overlap,
    }
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(
        path,
        MAXIMUM_RESULT_BYTES,
        "PyMC inferred multitype result is absent or oversized",
    )
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
