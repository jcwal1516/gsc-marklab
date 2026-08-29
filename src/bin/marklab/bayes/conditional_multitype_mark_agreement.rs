use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, SamplingSummary,
    SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    conditional_multitype_mark::{self, Args as FitArgs, Output as FitOutput},
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::{
        backend_matches, compare_scalar, compare_vector, summary_valid, ScalarAgreement,
        VectorAgreement,
    },
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
    ConditionalMultitypeMarkAgreement(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    radius_um: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
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
    maximum_points: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_neighbor_visits: u64,
    #[arg(long)]
    maximum_edges: usize,
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    #[arg(long)]
    maximum_working_bytes: usize,
    #[arg(long)]
    pymc_maximum_tree_depth: u32,
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    #[arg(long)]
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_absolute_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct TypeRow {
    type_id: String,
    intercept: SarScalarSummary,
}
#[derive(Debug, Deserialize, Serialize)]
struct PotentialRow {
    type_a: String,
    type_b: String,
    potential: SarScalarSummary,
}
#[derive(Debug, Deserialize, Serialize)]
struct AffinityRow {
    type_a: String,
    type_b: String,
    contrast: SarScalarSummary,
}
#[derive(Debug, Deserialize, Serialize)]
struct ScoreRows {
    independent_label_composite_log_score: f64,
    spatial_composite_log_score: SarScalarSummary,
    mean_composite_log_score_improvement: f64,
}
#[derive(Debug, Deserialize, Serialize)]
struct PpcRows {
    mode: String,
    observed_same_type_edges: u64,
    expected_same_type_edges: SarScalarSummary,
    observed_type_counts: Vec<u64>,
    expected_type_counts: Vec<SarScalarSummary>,
}
#[derive(Debug, Deserialize)]
struct FitView {
    input_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    type_intercepts: Vec<TypeRow>,
    pair_potentials: Vec<PotentialRow>,
    pair_affinity_contrasts: Vec<AffinityRow>,
    comparison: ScoreRows,
    posterior_predictive: PpcRows,
    diagnostics: NormalMeanDiagnostics,
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
    type_intercepts: Vec<TypeRow>,
    pair_potentials: Vec<PotentialRow>,
    pair_affinity_contrasts: Vec<AffinityRow>,
    comparison: ScoreRows,
    posterior_predictive: PpcRows,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Serialize)]
struct Comparison {
    intercepts: VectorAgreement,
    pair_potentials: VectorAgreement,
    pair_affinities: VectorAgreement,
    composite_log_score: ScalarAgreement,
    expected_same_type_edges: ScalarAgreement,
    expected_type_counts: VectorAgreement,
    all_pass: bool,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    pymc: FitOutput,
    numpyro: NumpyroResult,
    comparison: Comparison,
    fit_state: FitState,
    agreement_status: &'static str,
    statistical_unit: &'static str,
    assumptions: [&'static str; 4],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ConditionalMultitypeMarkAgreement(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    if !(10..=14).contains(&args.pymc_maximum_tree_depth)
        || !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || !args.maximum_standardized_difference.is_finite()
        || args.maximum_standardized_difference <= 0.0
        || !args.minimum_absolute_tolerance.is_finite()
        || args.minimum_absolute_tolerance <= 0.0
    {
        return Err(BayesCliError::Input(
            "conditional multitype agreement controls are invalid".into(),
        ));
    }
    let prepared = conditional_multitype_mark::prepare(FitArgs {
        input: args.input,
        reference_type: args.reference_type,
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_points: args.maximum_points,
        maximum_types: args.maximum_types,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        maximum_edges: args.maximum_edges,
        maximum_draw_parameter_work: args.maximum_draw_parameter_work,
        maximum_working_bytes: args.maximum_working_bytes,
        maximum_tree_depth: args.pymc_maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let pymc = conditional_multitype_mark::execute_prepared(&prepared)?;
    let pymc_view: FitView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    let (numpyro, numpyro_request_sha, numpyro_backend) = execute_numpyro(
        &prepared,
        args.numpyro_maximum_tree_depth,
        args.timeout_seconds,
    )?;
    validate_numpyro(
        &numpyro,
        &pymc_view,
        &prepared,
        &numpyro_request_sha,
        &numpyro_backend,
        args.numpyro_maximum_tree_depth,
    )?;
    let comparison = compare(
        &pymc_view,
        &numpyro,
        args.maximum_standardized_difference,
        args.minimum_absolute_tolerance,
    );
    let complete = pymc_view.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.conditional_multitype_mark_backend_agreement",
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
            statistical_unit: "one_fixed_location_pattern",
            assumptions: [
                "same_fixed_location_graph_types_priors_draws_and_seed",
                "independent_pymc_and_numpyro_conditional_implementations",
                "composite_pseudolikelihood_is_not_a_normalized_joint_likelihood",
                "one_pattern_does_not_support_patient_population_inference",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: "experimental_cross_backend_validation",
        },
    )
}

fn execute_numpyro(
    prepared: &conditional_multitype_mark::Prepared,
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
        worker_sha256: sha256_hex(&read(
            &directory.join("marklab_numpyro_conditional_multitype_mark_worker.py"),
        )?),
    };
    let request = serde_json::json!({
        "format": "marklab.numpyro_conditional_multitype_mark_request",
        "version": 1,
        "backend": backend,
        "jax_version": JAX_VERSION,
        "source_request_sha256": prepared.request_sha256(),
        "source_request": serde_json::from_slice::<serde_json::Value>(prepared.request_bytes())?,
        "maximum_tree_depth": depth,
    });
    let bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_conditional_multitype_mark_worker.py",
        &bytes,
        timeout,
    )?;
    Ok((serde_json::from_slice(&output)?, request_sha, backend))
}

fn validate_numpyro(
    result: &NumpyroResult,
    pymc: &FitView,
    prepared: &conditional_multitype_mark::Prepared,
    request_sha: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesCliError> {
    let rows_valid = result
        .type_intercepts
        .iter()
        .all(|row| summary_valid(&row.intercept))
        && result
            .pair_potentials
            .iter()
            .all(|row| summary_valid(&row.potential))
        && result
            .pair_affinity_contrasts
            .iter()
            .all(|row| summary_valid(&row.contrast))
        && summary_valid(&result.comparison.spatial_composite_log_score)
        && summary_valid(&result.posterior_predictive.expected_same_type_edges)
        && result
            .posterior_predictive
            .expected_type_counts
            .iter()
            .all(summary_valid);
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
    if result.format != "marklab.numpyro_conditional_multitype_mark_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != pymc.input_sha256
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != prepared.request_sha256()
        || result.maximum_tree_depth != depth
        || result.sampling.completed_draws != pymc.sampling.completed_draws
        || result.type_intercepts.len() != pymc.type_intercepts.len()
        || result.pair_potentials.len() != pymc.pair_potentials.len()
        || result.pair_affinity_contrasts.len() != pymc.pair_affinity_contrasts.len()
        || result.posterior_predictive.mode != "one_step_conditionals_given_observed_neighbors"
        || result.posterior_predictive.observed_same_type_edges
            != pymc.posterior_predictive.observed_same_type_edges
        || result.posterior_predictive.observed_type_counts
            != pymc.posterior_predictive.observed_type_counts
        || !rows_valid
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "conditional multitype NumPyro result differs".into(),
        ));
    }
    Ok(())
}

fn compare(left: &FitView, right: &NumpyroResult, standardized: f64, tolerance: f64) -> Comparison {
    let left_ess = left.diagnostics.ess_bulk;
    let right_ess = right.diagnostics.ess_bulk;
    let intercepts = compare_vector(
        left.type_intercepts.iter().map(|row| &row.intercept),
        right.type_intercepts.iter().map(|row| &row.intercept),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let pair_potentials = compare_vector(
        left.pair_potentials.iter().map(|row| &row.potential),
        right.pair_potentials.iter().map(|row| &row.potential),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let pair_affinities = compare_vector(
        left.pair_affinity_contrasts.iter().map(|row| &row.contrast),
        right
            .pair_affinity_contrasts
            .iter()
            .map(|row| &row.contrast),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let composite_log_score = compare_scalar(
        &left.comparison.spatial_composite_log_score,
        &right.comparison.spatial_composite_log_score,
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let expected_same_type_edges = compare_scalar(
        &left.posterior_predictive.expected_same_type_edges,
        &right.posterior_predictive.expected_same_type_edges,
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let expected_type_counts = compare_vector(
        left.posterior_predictive.expected_type_counts.iter(),
        right.posterior_predictive.expected_type_counts.iter(),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let all_pass = intercepts.passes
        && pair_potentials.passes
        && pair_affinities.passes
        && composite_log_score.passes
        && expected_same_type_edges.passes
        && expected_type_counts.passes;
    Comparison {
        intercepts,
        pair_potentials,
        pair_affinities,
        composite_log_score,
        expected_same_type_edges,
        expected_type_counts,
        all_pass,
    }
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}
