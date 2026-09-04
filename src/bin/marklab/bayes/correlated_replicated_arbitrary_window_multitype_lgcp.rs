use std::{collections::BTreeSet, fs, path::PathBuf};

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
        pair_differences_valid, summary_valid, type_posteriors_valid, PairDifference,
        PatternTypePredictive, TypePosterior,
    },
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{
        self as inferred_kernel, Args as SourceArgs,
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
    FitCorrelatedReplicatedArbitraryWindowMultitypeLgcp(Box<Args>),
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
    pub(crate) lkj_concentration: f64,
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
    pub(crate) maximum_coregionalization_work: u64,
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorrelationPrior {
    lkj_concentration: f64,
    marginal_field_scale: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    coregionalization_work: u64,
    maximum_coregionalization_work: u64,
    maximum_output_bytes: usize,
    timeout_seconds: u64,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: serde_json::Value,
    correlation_prior: CorrelationPrior,
    resources: Resources,
    maximum_tree_depth: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TypeFieldScale {
    type_id: String,
    scale: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CrossTypeCorrelation {
    type_a: String,
    type_b: String,
    correlation: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CoregionalizationOracle {
    target_correlation_matrix: Vec<f64>,
    reconstructed_correlation_matrix: Vec<f64>,
    maximum_absolute_error: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    field_length_scale_um: SarScalarSummary,
    type_field_scales: Vec<TypeFieldScale>,
    cross_type_correlations: Vec<CrossTypeCorrelation>,
    posterior_mean_correlation_matrix: Vec<f64>,
    type_posteriors: Vec<TypePosterior>,
    group_effect_differences: Vec<PairDifference>,
    pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    diagnostics: NormalMeanDiagnostics,
    coregionalization_oracle: CoregionalizationOracle,
    coregionalization_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    field_length_scale_um: SarScalarSummary,
    type_field_scales: Vec<TypeFieldScale>,
    cross_type_correlations: Vec<CrossTypeCorrelation>,
    posterior_mean_correlation_matrix: Vec<f64>,
    type_posteriors: Vec<TypePosterior>,
    group_effect_differences: Vec<PairDifference>,
    pattern_type_posterior_predictive: Vec<PatternTypePredictive>,
    diagnostics: NormalMeanDiagnostics,
    coregionalization_oracle: CoregionalizationOracle,
    correlation_prior: CorrelationPrior,
    resources: Resources,
    patient_count: usize,
    pattern_count: usize,
    type_count: usize,
    total_node_count: usize,
    total_event_count: u64,
    statistical_unit: String,
    pattern_unit: String,
    likelihood: String,
    null_model: String,
    correlation_parameterization: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct Prepared {
    source: inferred_kernel::Prepared,
    source_value: serde_json::Value,
    backend: BackendContract,
    correlation_prior: CorrelationPrior,
    resources: Resources,
    request_bytes: Vec<u8>,
    request_sha256: String,
    input_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::FitCorrelatedReplicatedArbitraryWindowMultitypeLgcp(args) = command;
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, BayesCliError> {
    if !args.lkj_concentration.is_finite()
        || args.lkj_concentration < 1.0
        || args.maximum_coregionalization_work == 0
    {
        return Err(BayesCliError::Input(
            "correlated multitype LGCP controls are invalid".into(),
        ));
    }
    let completed_draws = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .ok_or_else(|| BayesCliError::Input("correlated draw count overflows".into()))?;
    let source = inferred_kernel::prepare(SourceArgs {
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
        field_amplitude_prior_scale: args.field_amplitude_prior_scale,
        field_length_scale_prior_scale_um: args.field_length_scale_prior_scale_um,
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
        maximum_kernel_cube_work: args.maximum_kernel_cube_work,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let source_value: serde_json::Value = serde_json::from_slice(source.request_bytes())?;
    let fixed = source_value["source_request"]
        .as_object()
        .ok_or_else(|| BayesCliError::Input("correlated source request differs".into()))?;
    let types = fixed["type_ids"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("correlated type identities differ".into()))?
        .len() as u64;
    let nodes = fixed["nodes"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("correlated nodes differ".into()))?
        .len() as u64;
    let coregionalization_work = completed_draws
        .checked_mul(types)
        .and_then(|value| value.checked_mul(types))
        .and_then(|value| value.checked_mul(nodes))
        .ok_or_else(|| BayesCliError::Input("coregionalization work overflows".into()))?;
    if coregionalization_work > args.maximum_coregionalization_work {
        return Err(BayesCliError::Input(format!(
            "coregionalization work exceeds maximum: {coregionalization_work} > {}",
            args.maximum_coregionalization_work
        )));
    }
    let input_sha256 = fixed["input_sha256"]
        .as_str()
        .ok_or_else(|| BayesCliError::Input("correlated input digest differs".into()))?
        .to_owned();
    let correlation_prior = CorrelationPrior {
        lkj_concentration: args.lkj_concentration,
        marginal_field_scale: args.field_amplitude_prior_scale,
    };
    let resources = Resources {
        coregionalization_work,
        maximum_coregionalization_work: args.maximum_coregionalization_work,
        maximum_output_bytes: 16 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(&directory.join(
            "marklab_numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_worker.py",
        ))?),
    };
    let source_request_sha256 = sha256_hex(source.request_bytes());
    let request = WorkerRequest {
        format: "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: &source_request_sha256,
        source_request: source_value.clone(),
        correlation_prior: correlation_prior.clone(),
        resources: resources.clone(),
        maximum_tree_depth: args.maximum_tree_depth,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        source,
        source_value,
        backend,
        correlation_prior,
        resources,
        request_bytes,
        request_sha256,
        input_sha256,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, prepared)?;
    let fixed = &prepared.source_value["source_request"];
    let patients = fixed["patients"].as_array().unwrap();
    let patterns = fixed["patterns"].as_array().unwrap();
    let types = fixed["type_ids"].as_array().unwrap();
    let nodes = fixed["nodes"].as_array().unwrap();
    let counts = fixed["node_type_counts"].as_array().unwrap();
    Ok(Output {
        format: "marklab.correlated_replicated_arbitrary_window_multitype_lgcp".into(),
        version: 1,
        backend: result.backend,
        jax_version: result.jax_version,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        source_request_sha256: result.source_request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        field_length_scale_um: result.field_length_scale_um,
        type_field_scales: result.type_field_scales,
        cross_type_correlations: result.cross_type_correlations,
        posterior_mean_correlation_matrix: result.posterior_mean_correlation_matrix,
        type_posteriors: result.type_posteriors,
        group_effect_differences: result.group_effect_differences,
        pattern_type_posterior_predictive: result.pattern_type_posterior_predictive,
        diagnostics: result.diagnostics,
        coregionalization_oracle: result.coregionalization_oracle,
        correlation_prior: prepared.correlation_prior.clone(),
        resources: prepared.resources.clone(),
        patient_count: patients.len(),
        pattern_count: patterns.len(),
        type_count: types.len(),
        total_node_count: nodes.len(),
        total_event_count: counts.iter().filter_map(|row| row["count"].as_u64()).sum(),
        statistical_unit: "patient".into(),
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        likelihood: "correlated_type_specific_log_gaussian_cox_intensities_over_one_exact_window"
            .into(),
        null_model: "every_off_diagonal_latent_field_correlation_equals_zero".into(),
        correlation_parameterization:
            "lower_triangular_lkj_cholesky_with_positive_marginal_field_scales".into(),
        assumptions: [
            "one_shared_isotropic_matern_length_scale_across_types_and_patterns",
            "cross_type_dependence_is_spatial_latent_covariance_not_pairwise_interaction",
            "lkj_cholesky_and_positive_field_scales_identify_the_coregionalization",
            "patients_not_patterns_nodes_types_or_events_are_population_replicates",
            "exact_windows_and_complete_typed_quadrature_counts_are_provenance_complete",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if result.fit_state == FitState::Complete {
            "experimental_correlated_multitype_latent_field"
        } else {
            "diagnostic_only_nonconverged_correlated_multitype_latent_field"
        }
        .into(),
    })
}

impl Output {
    pub(crate) fn validate_for(&self, prepared: &Prepared) -> Result<(), BayesCliError> {
        if self.format != "marklab.correlated_replicated_arbitrary_window_multitype_lgcp"
            || self.version != 1
            || self.request_sha256 != prepared.request_sha256
            || self.source_request_sha256 != sha256_hex(prepared.source.request_bytes())
            || self.input_sha256 != prepared.input_sha256
            || self.correlation_prior != prepared.correlation_prior
            || self.resources != prepared.resources
            || self.statistical_unit != "patient"
            || self.correlation_parameterization
                != "lower_triangular_lkj_cholesky_with_positive_marginal_field_scales"
        {
            return Err(BayesCliError::Backend(
                "durable correlated multitype LGCP result differs".into(),
            ));
        }
        validate_correlation_matrix(&self.posterior_mean_correlation_matrix, self.type_count)
    }
}

fn validate_worker(result: &WorkerResult, prepared: &Prepared) -> Result<(), BayesCliError> {
    let fixed = &prepared.source_value["source_request"];
    let types = fixed["type_ids"]
        .as_array()
        .ok_or_else(|| BayesCliError::Backend("correlated type identities differ".into()))?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let complete_draws = fixed["sampling"]["chains"].as_u64().unwrap_or(0)
        * fixed["sampling"]["draws_per_chain"].as_u64().unwrap_or(0);
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
    let expected_pairs = types.len() * (types.len() - 1) / 2;
    let correlation_pairs = result.cross_type_correlations.len() == expected_pairs
        && result.cross_type_correlations.iter().all(|row| {
            types.contains(&row.type_a)
                && types.contains(&row.type_b)
                && row.type_a < row.type_b
                && summary_valid(&row.correlation)
                && (-1.0..=1.0).contains(&row.correlation.mean)
                && (-1.0..=1.0).contains(&row.correlation.interval_lower)
                && (-1.0..=1.0).contains(&row.correlation.interval_upper)
        });
    let scale_types = result
        .type_field_scales
        .iter()
        .map(|row| row.type_id.as_str())
        .collect::<BTreeSet<_>>();
    if result.format
        != "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_result"
        || result.version != 1
        || !backend_matches(&result.backend, &prepared.backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != prepared.input_sha256
        || result.request_sha256 != prepared.request_sha256
        || result.source_request_sha256 != sha256_hex(prepared.source.request_bytes())
        || result.sampling.completed_draws != complete_draws
        || result.coregionalization_work != prepared.resources.coregionalization_work
        || !summary_valid(&result.field_length_scale_um)
        || result.field_length_scale_um.mean <= 0.0
        || result.type_field_scales.len() != types.len()
        || scale_types.len() != types.len()
        || !result.type_field_scales.iter().all(|row| {
            types.contains(&row.type_id) && summary_valid(&row.scale) && row.scale.mean > 0.0
        })
        || !correlation_pairs
        || validate_correlation_matrix(&result.posterior_mean_correlation_matrix, types.len())
            .is_err()
        || !type_posteriors_valid(&result.type_posteriors, &types)
        || !pair_differences_valid(&result.group_effect_differences, &types)
        || !inferred_kernel::predictive_valid(
            &result.pattern_type_posterior_predictive,
            fixed,
            complete_draws,
        )
        || result
            .coregionalization_oracle
            .target_correlation_matrix
            .len()
            != 9
        || result
            .coregionalization_oracle
            .reconstructed_correlation_matrix
            .len()
            != 9
        || !result
            .coregionalization_oracle
            .maximum_absolute_error
            .is_finite()
        || result.coregionalization_oracle.maximum_absolute_error > 1e-12
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "correlated multitype LGCP backend result differs".into(),
        ));
    }
    Ok(())
}

fn validate_correlation_matrix(matrix: &[f64], dimension: usize) -> Result<(), BayesCliError> {
    if !(2..=8).contains(&dimension) || matrix.len() != dimension * dimension {
        return Err(BayesCliError::Backend(
            "correlation matrix dimensions differ".into(),
        ));
    }
    let mut cholesky = vec![0.0; matrix.len()];
    for row in 0..dimension {
        for column in 0..=row {
            let value = matrix[row * dimension + column];
            if !value.is_finite()
                || (matrix[column * dimension + row] - value).abs() > 1e-10
                || (row == column && (value - 1.0).abs() > 1e-10)
            {
                return Err(BayesCliError::Backend(
                    "correlation matrix is not finite symmetric unit diagonal".into(),
                ));
            }
            let prior = (0..column)
                .map(|index| {
                    cholesky[row * dimension + index] * cholesky[column * dimension + index]
                })
                .sum::<f64>();
            if row == column {
                let diagonal = value - prior;
                if diagonal <= 1e-12 {
                    return Err(BayesCliError::Backend(
                        "correlation matrix is not positive definite".into(),
                    ));
                }
                cholesky[row * dimension + column] = diagonal.sqrt();
            } else {
                cholesky[row * dimension + column] =
                    (value - prior) / cholesky[column * dimension + column];
            }
        }
    }
    Ok(())
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
