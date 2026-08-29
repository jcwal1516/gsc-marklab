use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::{
        backend_matches, compare_scalar, compare_vector, summary_valid, ScalarAgreement,
        VectorAgreement,
    },
    replicated_arbitrary_window_lgcp_fit::{NodePosterior, PatientEffect, PatternEffect},
    replicated_arbitrary_window_lgcp_inferred_kernel::{self, Posterior, ResultDocument},
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

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
    ReplicatedArbitraryWindowLgcpInferredKernelAgreement(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
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
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    pymc_maximum_tree_depth: u32,
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    #[arg(long)]
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    #[arg(long)]
    minimum_field_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumpyroResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    input_sha256: String,
    maximum_tree_depth: u32,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: Posterior,
    patient_effects: Vec<PatientEffect>,
    pattern_effects: Vec<PatternEffect>,
    nodes: Vec<NodePosterior>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Serialize)]
struct Comparison {
    intercept: ScalarAgreement,
    group_effect: ScalarAgreement,
    covariate_effect: ScalarAgreement,
    patient_sd: ScalarAgreement,
    pattern_sd: ScalarAgreement,
    field_amplitude: ScalarAgreement,
    field_length_scale_um: ScalarAgreement,
    patient_effects: VectorAgreement,
    pattern_effects: VectorAgreement,
    latent_effect: VectorAgreement,
    expected_count: VectorAgreement,
    patient_count: usize,
    pattern_count: usize,
    node_count: usize,
    all_pass: bool,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    pymc: ResultDocument,
    numpyro: NumpyroResult,
    comparison: Comparison,
    fit_state: FitState,
    agreement_status: &'static str,
    statistical_unit: &'static str,
    assumptions: [&'static str; 3],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowLgcpInferredKernelAgreement(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    if !(10..=14).contains(&args.pymc_maximum_tree_depth)
        || !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || [
            args.maximum_standardized_difference,
            args.minimum_parameter_tolerance,
            args.minimum_field_tolerance,
        ]
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
    {
        return Err(BayesCliError::Input(
            "inferred-kernel agreement controls are invalid".into(),
        ));
    }
    let prepared = replicated_arbitrary_window_lgcp_inferred_kernel::prepare(
        args.input,
        args.reference_group,
        args.comparison_group,
        args.intercept_prior_mean,
        args.intercept_prior_sd,
        args.group_effect_prior_sd,
        args.covariate_effect_prior_sd,
        args.patient_sd_prior_scale,
        args.pattern_sd_prior_scale,
        args.field_amplitude_prior_scale,
        args.field_length_scale_prior_scale_um,
        args.jitter,
        NutsSamplingSpec {
            chains: args.chains,
            tune_per_chain: args.tune,
            draws_per_chain: args.draws,
            target_accept: args.target_accept,
            seed: args.seed,
        },
        args.maximum_patients,
        args.maximum_patterns,
        args.maximum_nodes_per_pattern,
        args.maximum_total_nodes,
        args.maximum_total_events,
        args.maximum_draw_node_work,
        args.maximum_kernel_cube_work,
        args.pymc_maximum_tree_depth,
        args.timeout_seconds,
    )?;
    let pymc = replicated_arbitrary_window_lgcp_inferred_kernel::execute(&prepared)?;
    let (numpyro, request_sha, expected_backend) = execute_numpyro(
        &prepared,
        args.numpyro_maximum_tree_depth,
        args.timeout_seconds,
    )?;
    validate_numpyro(
        &numpyro,
        &prepared,
        &request_sha,
        &expected_backend,
        args.numpyro_maximum_tree_depth,
    )?;
    let comparison = compare(
        &pymc,
        &numpyro,
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    );
    let complete = pymc.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_arbitrary_window_lgcp_inferred_kernel_backend_agreement",
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
            },
            statistical_unit: "patient",
            assumptions: [
                "same_exact_data_priors_draws_target_accept_and_seed",
                "independent_pymc_and_numpyro_dynamic_kernel_implementations",
                "sampler_tree_depth_capacities_are_explicit",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: "experimental_cross_backend_validation",
        },
    )
}

fn request_value(
    prepared: &replicated_arbitrary_window_lgcp_inferred_kernel::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    backend: BackendContract,
    depth: u32,
) -> Result<serde_json::Value, BayesCliError> {
    Ok(serde_json::json!({
        "format": "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_request",
        "version": 1,
        "backend": backend,
        "jax_version": JAX_VERSION,
        "source_request_sha256": prepared.request_sha256(),
        "source_request": serde_json::from_slice::<serde_json::Value>(prepared.request_bytes())?,
        "maximum_tree_depth": depth,
    }))
}

fn execute_numpyro(
    prepared: &replicated_arbitrary_window_lgcp_inferred_kernel::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    depth: u32,
    timeout: u64,
) -> Result<(NumpyroResult, String, BackendContract), BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(&directory.join(
            "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        ))?),
    };
    let bytes = serde_json::to_vec(&request_value(prepared, backend.clone(), depth)?)?;
    let request_sha = sha256_hex(&bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        &bytes,
        timeout,
    )?;
    Ok((serde_json::from_slice(&output)?, request_sha, backend))
}

fn validate_numpyro(
    result: &NumpyroResult,
    prepared: &replicated_arbitrary_window_lgcp_inferred_kernel::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    request_sha: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesCliError> {
    let source = prepared.source_request();
    let policy = source.diagnostic_policy();
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
        &result.posterior.field_amplitude,
        &result.posterior.field_length_scale_um,
    ];
    let complete = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != prepared.request_sha256()
        || result.input_sha256 != source.input_sha256()
        || result.maximum_tree_depth != depth
        || result.sampling.completed_draws != source.completed_draws()
        || summaries.into_iter().any(|value| !summary_valid(value))
        || !source.patient_effects_valid(&result.patient_effects)
        || !source.pattern_effects_valid(&result.pattern_effects)
        || !source.nodes_valid(&result.nodes)
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "NumPyro inferred-kernel result differs".into(),
        ));
    }
    Ok(())
}

fn compare(
    left: &ResultDocument,
    right: &NumpyroResult,
    maximum: f64,
    parameter_tolerance: f64,
    field_tolerance: f64,
) -> Comparison {
    let scalar = |a, b| {
        compare_scalar(
            a,
            b,
            left.diagnostics.ess_bulk,
            right.diagnostics.ess_bulk,
            maximum,
            parameter_tolerance,
        )
    };
    let intercept = scalar(&left.posterior.intercept, &right.posterior.intercept);
    let group_effect = scalar(&left.posterior.group_effect, &right.posterior.group_effect);
    let covariate_effect = scalar(
        &left.posterior.covariate_effect,
        &right.posterior.covariate_effect,
    );
    let patient_sd = scalar(&left.posterior.patient_sd, &right.posterior.patient_sd);
    let pattern_sd = scalar(&left.posterior.pattern_sd, &right.posterior.pattern_sd);
    let field_amplitude = scalar(
        &left.posterior.field_amplitude,
        &right.posterior.field_amplitude,
    );
    let field_length_scale_um = scalar(
        &left.posterior.field_length_scale_um,
        &right.posterior.field_length_scale_um,
    );
    let vector = |a: Vec<_>, b: Vec<_>| {
        compare_vector(
            a.into_iter(),
            b.into_iter(),
            left.diagnostics.ess_bulk,
            right.diagnostics.ess_bulk,
            maximum,
            field_tolerance,
        )
    };
    let patient_effects = vector(
        left.patient_effects.iter().map(|row| &row.effect).collect(),
        right
            .patient_effects
            .iter()
            .map(|row| &row.effect)
            .collect(),
    );
    let pattern_effects = vector(
        left.pattern_effects.iter().map(|row| &row.effect).collect(),
        right
            .pattern_effects
            .iter()
            .map(|row| &row.effect)
            .collect(),
    );
    let latent_effect = vector(
        left.nodes.iter().map(|row| &row.latent_effect).collect(),
        right.nodes.iter().map(|row| &row.latent_effect).collect(),
    );
    let expected_count = vector(
        left.nodes.iter().map(|row| &row.expected_count).collect(),
        right.nodes.iter().map(|row| &row.expected_count).collect(),
    );
    let all_pass = [
        intercept.passes,
        group_effect.passes,
        covariate_effect.passes,
        patient_sd.passes,
        pattern_sd.passes,
        field_amplitude.passes,
        field_length_scale_um.passes,
        patient_effects.passes,
        pattern_effects.passes,
        latent_effect.passes,
        expected_count.passes,
    ]
    .into_iter()
    .all(|value| value);
    Comparison {
        intercept,
        group_effect,
        covariate_effect,
        patient_sd,
        pattern_sd,
        field_amplitude,
        field_length_scale_um,
        patient_effects,
        pattern_effects,
        latent_effect,
        expected_count,
        patient_count: left.patient_effects.len(),
        pattern_count: left.pattern_effects.len(),
        node_count: left.nodes.len(),
        all_pass,
    }
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}
