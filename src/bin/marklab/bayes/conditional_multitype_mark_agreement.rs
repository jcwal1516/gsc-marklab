pub(crate) use marklab_bayes::protocol::conditional_multitype_mark_agreement::{
    compare, validate_numpyro, FitView, NumpyroResult, Output,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;

use super::conditional_multitype_mark;
use super::conditional_multitype_mark::Args as FitArgs;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_bayes::AgreementStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// CSV of points: point_id or cell_id, x_um, y_um, and type_id or histologic_compartment.
    #[arg(long)]
    input: PathBuf,
    /// Point-type label used as the model reference.
    #[arg(long)]
    reference_type: String,
    /// Spatial radius in micrometers.
    #[arg(long)]
    radius_um: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for interaction.
    #[arg(long)]
    interaction_prior_sd: f64,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    seed: u64,
    /// Upper bound on points.
    #[arg(long)]
    maximum_points: usize,
    /// Upper bound on types.
    #[arg(long)]
    maximum_types: usize,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    maximum_neighbor_visits: u64,
    /// Upper bound on edges.
    #[arg(long)]
    maximum_edges: usize,
    /// Upper bound on draw parameter work.
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    maximum_working_bytes: usize,
    /// Maximum NUTS tree depth for the PyMC fit.
    #[arg(long)]
    pymc_maximum_tree_depth: u32,
    /// Maximum NUTS tree depth for the NumPyro fit.
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for absolute.
    #[arg(long)]
    minimum_absolute_tolerance: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(args: Args) -> Result<(), CliError> {
    if !(10..=14).contains(&args.pymc_maximum_tree_depth)
        || !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || !args.maximum_standardized_difference.is_finite()
        || args.maximum_standardized_difference <= 0.0
        || !args.minimum_absolute_tolerance.is_finite()
        || args.minimum_absolute_tolerance <= 0.0
    {
        return Err(CliError::Input(
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
        prepared.request_sha256(),
        &numpyro_request_sha,
        &numpyro_backend,
        args.numpyro_maximum_tree_depth,
    )
    .map_err(super::protocol_error)?;
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
                AgreementStatus::AgreeWithinMonteCarloError
            } else {
                AgreementStatus::DiagnosticOnlyDisagreementOrNonconvergence
            },
            statistical_unit: StatisticalUnit::OneFixedLocationPattern,
            assumptions: [
                "same_fixed_location_graph_types_priors_draws_and_seed",
                "independent_pymc_and_numpyro_conditional_implementations",
                "composite_pseudolikelihood_is_not_a_normalized_joint_likelihood",
                "one_pattern_does_not_support_patient_population_inference",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: ClaimStatus::ExperimentalCrossBackendValidation,
        },
    )
}

fn execute_numpyro(
    prepared: &conditional_multitype_mark::Prepared,
    depth: u32,
    timeout: u64,
) -> Result<(NumpyroResult, String, BackendContract), CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(
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
