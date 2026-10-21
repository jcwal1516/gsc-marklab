pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement::{FitView, NumpyroView, Output, WorkerRequest, validate_numpyro, compare};

use std::fs;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Args as FitArgs;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Output as FitOutput;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_bayes::AgreementStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_RESULT_BYTES: u64 = 16 * 1_048_576;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// Previously fitted PyMC result to use in the comparison.
    #[arg(long)]
    pymc_result: PathBuf,
    #[command(flatten)]
    fit: AgreementFitArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for parameter.
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    /// Minimum absolute comparison tolerance for field.
    #[arg(long)]
    minimum_field_tolerance: f64,
    /// Maximum NUTS tree depth for the NumPyro fit.
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct AgreementFitArgs {
    /// Per-node, per-type quadrature CSV (pattern_id, patient_id, group, type_id, count, ...).
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    comparison_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    reference_type: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    /// Prior scale for field amplitude.
    #[arg(long)]
    field_amplitude_prior_scale: f64,
    /// Prior scale, in micrometers, for field length scale.
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    jitter: f64,
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
    /// Upper bound on patients.
    #[arg(long)]
    maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    maximum_patterns: usize,
    /// Upper bound on types.
    #[arg(long)]
    maximum_types: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    maximum_total_nodes: usize,
    /// Upper bound on total node type rows.
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    /// Upper bound on total events.
    #[arg(long)]
    maximum_total_events: u64,
    /// Upper bound on draw node type work.
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    /// Upper bound on kernel cube work.
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    /// Upper bound on tree depth.
    #[arg(long)]
    maximum_tree_depth: u32,
    /// Maximum worker execution time in seconds.
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

pub(super) fn run(args: Args) -> Result<(), CliError> {
    for value in [
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(CliError::Input(
                "inferred multitype agreement controls are invalid".into(),
            ));
        }
    }
    if !(10..=14).contains(&args.numpyro_maximum_tree_depth) {
        return Err(CliError::Input(
            "NumPyro inferred multitype tree depth is invalid".into(),
        ));
    }
    let timeout = args.fit.timeout_seconds;
    let prepared =
        replicated_arbitrary_window_multitype_lgcp_inferred_kernel::prepare(args.fit.into_fit())?;
    let pymc: FitOutput = serde_json::from_slice(&read_bounded(&args.pymc_result)?)?;
    prepared.validate_result(&pymc)?;
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
    )
    .map_err(super::protocol_error)?;
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
            format: "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_backend_agreement",
            version: 1,
            pymc,
            numpyro: result,
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
            statistical_unit: StatisticalUnit::Patient,
            assumptions: [
                "same_exact_patients_patterns_windows_types_priors_draws_target_accept_and_seed",
                "numpyro_tree_depth_capacity_is_explicit_and_sampler_specific",
                "independent_dynamic_block_cholesky_hierarchy_and_likelihood_implementations",
                "patients_not_patterns_nodes_types_or_cells_are_population_replicates",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: ClaimStatus::ExperimentalCrossBackendValidation,
        },
    )
}

fn execute_numpyro(
    prepared: &replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Prepared,
    depth: u32,
    timeout: u64,
) -> Result<(serde_json::Value, BackendContract, String), CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let workers = repository.join("workers/python");
    let worker_name =
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py";
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&fs::read(workers.join("uv.lock")).map_err(
            |source| CliError::Io {
                path: workers.join("uv.lock"),
                source,
            },
        )?),
        worker_sha256: sha256_hex(&fs::read(workers.join(worker_name)).map_err(|source| {
            CliError::Io {
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

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_RESULT_BYTES,
        "PyMC inferred multitype result is absent or oversized",
    )
}
