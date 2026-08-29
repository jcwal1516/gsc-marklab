use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_fit::{
        self, predictive_valid, NodePosterior, PatientEffect, PatternEffect,
        PatternPosteriorPredictive, Posterior, ResultDocument, WorkerRequest,
    },
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct AgreementCli {
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
    ReplicatedArbitraryWindowLgcpAgreement(Box<Arguments>),
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
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    #[arg(long)]
    minimum_field_tolerance: f64,
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Serialize)]
struct NumpyroRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: &'a WorkerRequest,
    maximum_tree_depth: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumpyroResult {
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
    posterior: Posterior,
    patient_effects: Vec<PatientEffect>,
    pattern_effects: Vec<PatternEffect>,
    nodes: Vec<NodePosterior>,
    pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Serialize)]
struct ScalarAgreement {
    absolute_mean_difference: f64,
    combined_mcse: f64,
    standardized_difference: f64,
    intervals_overlap: bool,
    passes: bool,
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
    intercept: ScalarAgreement,
    group_effect: ScalarAgreement,
    covariate_effect: ScalarAgreement,
    patient_sd: ScalarAgreement,
    pattern_sd: ScalarAgreement,
    patient_effects: VectorAgreement,
    pattern_effects: VectorAgreement,
    latent_effect: VectorAgreement,
    expected_count: VectorAgreement,
    patient_count: usize,
    pattern_count: usize,
    node_count: usize,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
    all_pass: bool,
}

#[derive(Debug, Serialize)]
struct AgreementResult {
    format: String,
    version: u32,
    pymc: ResultDocument,
    numpyro: NumpyroResult,
    comparison: Comparison,
    fit_state: FitState,
    agreement_status: String,
    statistical_unit: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = AgreementCli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowLgcpAgreement(arguments) = command;
    run(*arguments)
}

fn run(arguments: Arguments) -> Result<(), BayesCliError> {
    for value in [
        arguments.maximum_standardized_difference,
        arguments.minimum_parameter_tolerance,
        arguments.minimum_field_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesCliError::Input(
                "replicated LGCP agreement controls are invalid".into(),
            ));
        }
    }
    if !(10..=14).contains(&arguments.numpyro_maximum_tree_depth) {
        return Err(BayesCliError::Input(
            "NumPyro replicated LGCP tree-depth bound is invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let prepared = replicated_arbitrary_window_lgcp_fit::prepare(
        arguments.input,
        arguments.reference_group,
        arguments.comparison_group,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.group_effect_prior_sd,
        arguments.covariate_effect_prior_sd,
        arguments.patient_sd_prior_scale,
        arguments.pattern_sd_prior_scale,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
        sampling,
        arguments.maximum_patients,
        arguments.maximum_patterns,
        arguments.maximum_nodes_per_pattern,
        arguments.maximum_total_nodes,
        arguments.maximum_total_events,
        arguments.maximum_draw_node_work,
        arguments.timeout_seconds,
    )?;
    let pymc = replicated_arbitrary_window_lgcp_fit::execute(&prepared)?;
    let numpyro = execute_numpyro(
        &prepared.request,
        &prepared.request_sha256,
        arguments.numpyro_maximum_tree_depth,
        arguments.timeout_seconds,
    )?;
    validate_numpyro(
        &numpyro,
        &prepared.request,
        &prepared.request_sha256,
        arguments.numpyro_maximum_tree_depth,
        &pymc,
    )?;
    let comparison = compare(
        &pymc,
        &numpyro,
        arguments.maximum_standardized_difference,
        arguments.minimum_parameter_tolerance,
        arguments.minimum_field_tolerance,
    );
    let complete = pymc.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &arguments.out,
        &AgreementResult {
            format: "marklab.replicated_arbitrary_window_lgcp_backend_agreement".into(),
            version: 1,
            pymc,
            numpyro,
            comparison,
            fit_state: if complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            agreement_status: if complete {
                "agree_within_monte_carlo_error"
            } else {
                "diagnostic_only_disagreement_or_nonconvergence"
            }
            .into(),
            statistical_unit: "patient".into(),
            assumptions: vec![
                "same_exact_patients_patterns_windows_nodes_priors_draws_target_accept_and_seed"
                    .into(),
                "numpyro_tree_depth_capacity_is_explicit_and_sampler_specific".into(),
                "independent_pymc_and_numpyro_nuts_implementations".into(),
                "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible".into(),
            claim_status: "experimental_cross_backend_validation".into(),
        },
    )
}

fn execute_numpyro(
    source: &WorkerRequest,
    source_sha256: &str,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
) -> Result<NumpyroResult, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker =
        read(&directory.join("marklab_numpyro_replicated_arbitrary_window_lgcp_worker.py"))?;
    let request = NumpyroRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        jax_version: JAX_VERSION,
        source_request_sha256: source_sha256,
        source_request: source,
        maximum_tree_depth,
    };
    let bytes = serde_json::to_vec(&request)?;
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_worker.py",
        &bytes,
        timeout_seconds,
    )?;
    serde_json::from_slice(&output).map_err(Into::into)
}

fn validate_numpyro(
    result: &NumpyroResult,
    source: &WorkerRequest,
    source_sha256: &str,
    maximum_tree_depth: u32,
    pymc: &ResultDocument,
) -> Result<(), BayesCliError> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker =
        read(&directory.join("marklab_numpyro_replicated_arbitrary_window_lgcp_worker.py"))?;
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    let request = NumpyroRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: source_sha256,
        source_request: source,
        maximum_tree_depth,
    };
    let request_sha256 = sha256_hex(&serde_json::to_vec(&request)?);
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
    ];
    let identities_match = result
        .patient_effects
        .iter()
        .zip(&pymc.patient_effects)
        .all(|(left, right)| left.patient_id == right.patient_id && summary_valid(&left.effect))
        && result
            .pattern_effects
            .iter()
            .zip(&pymc.pattern_effects)
            .all(|(left, right)| {
                left.pattern_id == right.pattern_id && summary_valid(&left.effect)
            })
        && result.nodes.iter().zip(&pymc.nodes).all(|(left, right)| {
            left.pattern_id == right.pattern_id
                && left.node_id == right.node_id
                && summary_valid(&left.latent_effect)
                && summary_valid(&left.expected_count)
                && left.expected_count.mean > 0.0
        });
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= 1.01
        && result.diagnostics.ess_bulk >= 400.0
        && result.diagnostics.ess_tail >= 400.0
        && result.diagnostics.minimum_ebfmi >= 0.2
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_arbitrary_window_lgcp_result"
        || result.version != 1
        || !backend_matches(&result.backend, &backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != pymc.input_sha256
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_sha256
        || result.maximum_tree_depth != maximum_tree_depth
        || result.sampling.completed_draws != pymc.sampling.completed_draws
        || result.patient_effects.len() != pymc.patient_effects.len()
        || result.pattern_effects.len() != pymc.pattern_effects.len()
        || result.nodes.len() != pymc.nodes.len()
        || result.pattern_posterior_predictive.len() != pymc.pattern_posterior_predictive.len()
        || !predictive_valid(
            &result.pattern_posterior_predictive,
            source,
            result.sampling.completed_draws,
        )
        || summaries.into_iter().any(|summary| !summary_valid(summary))
        || !identities_match
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "NumPyro replicated LGCP result differs".into(),
        ));
    }
    Ok(())
}

fn compare(
    pymc: &ResultDocument,
    numpyro: &NumpyroResult,
    maximum: f64,
    parameter_tolerance: f64,
    field_tolerance: f64,
) -> Comparison {
    let ess_pymc = pymc.diagnostics.ess_bulk;
    let ess_numpyro = numpyro.diagnostics.ess_bulk;
    let intercept = compare_scalar(
        &pymc.posterior.intercept,
        &numpyro.posterior.intercept,
        ess_pymc,
        ess_numpyro,
        maximum,
        parameter_tolerance,
    );
    let group_effect = compare_scalar(
        &pymc.posterior.group_effect,
        &numpyro.posterior.group_effect,
        ess_pymc,
        ess_numpyro,
        maximum,
        parameter_tolerance,
    );
    let covariate_effect = compare_scalar(
        &pymc.posterior.covariate_effect,
        &numpyro.posterior.covariate_effect,
        ess_pymc,
        ess_numpyro,
        maximum,
        parameter_tolerance,
    );
    let patient_sd = compare_scalar(
        &pymc.posterior.patient_sd,
        &numpyro.posterior.patient_sd,
        ess_pymc,
        ess_numpyro,
        maximum,
        parameter_tolerance,
    );
    let pattern_sd = compare_scalar(
        &pymc.posterior.pattern_sd,
        &numpyro.posterior.pattern_sd,
        ess_pymc,
        ess_numpyro,
        maximum,
        parameter_tolerance,
    );
    let patient_effects = compare_vector(
        pymc.patient_effects.iter().map(|value| &value.effect),
        numpyro.patient_effects.iter().map(|value| &value.effect),
        ess_pymc,
        ess_numpyro,
        maximum,
        field_tolerance,
    );
    let pattern_effects = compare_vector(
        pymc.pattern_effects.iter().map(|value| &value.effect),
        numpyro.pattern_effects.iter().map(|value| &value.effect),
        ess_pymc,
        ess_numpyro,
        maximum,
        field_tolerance,
    );
    let latent_effect = compare_vector(
        pymc.nodes.iter().map(|value| &value.latent_effect),
        numpyro.nodes.iter().map(|value| &value.latent_effect),
        ess_pymc,
        ess_numpyro,
        maximum,
        field_tolerance,
    );
    let expected_count = compare_vector(
        pymc.nodes.iter().map(|value| &value.expected_count),
        numpyro.nodes.iter().map(|value| &value.expected_count),
        ess_pymc,
        ess_numpyro,
        maximum,
        field_tolerance,
    );
    let all_pass = [
        intercept.passes,
        group_effect.passes,
        covariate_effect.passes,
        patient_sd.passes,
        pattern_sd.passes,
        patient_effects.passes,
        pattern_effects.passes,
        latent_effect.passes,
        expected_count.passes,
    ]
    .into_iter()
    .all(|passes| passes);
    Comparison {
        intercept,
        group_effect,
        covariate_effect,
        patient_sd,
        pattern_sd,
        patient_effects,
        pattern_effects,
        latent_effect,
        expected_count,
        patient_count: pymc.patient_effects.len(),
        pattern_count: pymc.pattern_effects.len(),
        node_count: pymc.nodes.len(),
        maximum_standardized_difference: maximum,
        minimum_parameter_tolerance: parameter_tolerance,
        minimum_field_tolerance: field_tolerance,
        all_pass,
    }
}

fn compare_scalar(
    left: &SarScalarSummary,
    right: &SarScalarSummary,
    left_ess: f64,
    right_ess: f64,
    maximum: f64,
    tolerance: f64,
) -> ScalarAgreement {
    let difference = (left.mean - right.mean).abs();
    let combined_mcse =
        ((left.sd / left_ess.sqrt()).powi(2) + (right.sd / right_ess.sqrt()).powi(2)).sqrt();
    let standardized = difference / combined_mcse.max(tolerance);
    let intervals_overlap = overlap(left, right);
    ScalarAgreement {
        absolute_mean_difference: difference,
        combined_mcse,
        standardized_difference: standardized,
        intervals_overlap,
        passes: standardized <= maximum && intervals_overlap,
    }
}

fn compare_vector<'a>(
    left: impl Iterator<Item = &'a SarScalarSummary>,
    right: impl Iterator<Item = &'a SarScalarSummary>,
    left_ess: f64,
    right_ess: f64,
    maximum: f64,
    tolerance: f64,
) -> VectorAgreement {
    let comparisons = left
        .zip(right)
        .map(|(left, right)| compare_scalar(left, right, left_ess, right_ess, maximum, tolerance))
        .collect::<Vec<_>>();
    let maximum_standardized_difference = comparisons
        .iter()
        .map(|value| value.standardized_difference)
        .fold(0.0_f64, f64::max);
    let all_intervals_overlap = comparisons.iter().all(|value| value.intervals_overlap);
    VectorAgreement {
        element_count: comparisons.len(),
        maximum_standardized_difference,
        all_intervals_overlap,
        passes: maximum_standardized_difference <= maximum && all_intervals_overlap,
    }
}

fn overlap(left: &SarScalarSummary, right: &SarScalarSummary) -> bool {
    left.interval_lower <= right.interval_upper && right.interval_lower <= left.interval_upper
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
