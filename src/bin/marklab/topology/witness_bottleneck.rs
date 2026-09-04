use std::path::{Path, PathBuf};

use marklab_topology::{
    sha256_hex, TopologyBackendContract, WitnessBottleneckDimensionResult,
    WitnessBottleneckPerturbationResult, WitnessPersistenceBottleneckStabilityResult,
    WitnessPersistenceBottleneckStabilitySpec, WitnessPersistenceResult,
};
use serde::Deserialize;

use super::{
    publish_json, read_input, read_required, run_worker,
    witness_stability::{
        execute_witness_persistence_stability_runs, prepare_witness_persistence_stability_spec,
        summarize_witness_persistence_stability, validate_witness_persistence_stability_result,
        PreparedWitnessPersistenceStability,
    },
    TopologyCliError,
};

pub(crate) struct PreparedWitnessPersistenceBottleneckStability {
    pub(crate) spec: WitnessPersistenceBottleneckStabilitySpec,
    pub(crate) stability: PreparedWitnessPersistenceStability,
    pub(crate) backend: TopologyBackendContract,
    pub(super) worker_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WitnessBottleneckWorkerComparison {
    replicate: usize,
    dimension: usize,
    status: String,
    bottleneck_distance_um_squared: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WitnessBottleneckWorkerResult {
    format: String,
    version: u32,
    backend: TopologyBackendContract,
    request_sha256: String,
    metric: String,
    essential_interval_policy: String,
    comparisons: Vec<WitnessBottleneckWorkerComparison>,
    comparison_count: u64,
    interval_count: u64,
    maximum_finite_bottleneck_distance_um_squared: f64,
    has_infinite_essential_mismatch: bool,
    claim_status: String,
}

pub(crate) fn prepare_witness_persistence_bottleneck_stability(
    input: &Path,
) -> Result<PreparedWitnessPersistenceBottleneckStability, TopologyCliError> {
    let bytes = read_input(input)?;
    let spec: WitnessPersistenceBottleneckStabilitySpec = serde_json::from_slice(&bytes)?;
    validate_witness_bottleneck_controls(&spec)?;
    let stability = prepare_witness_persistence_stability_spec(spec.stability.clone())?;
    let repository = marklab::python_backend_assets_root()?;
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_bottleneck_worker.py");
    let worker = read_required(&worker_path)?;
    let backend = TopologyBackendContract {
        name: "gudhi".into(),
        version: marklab_topology::GUDHI_VERSION.into(),
        python_version: "3.12".into(),
        license: "MIT_with_GPLv3_CGAL_alpha_complex_dependency".into(),
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    Ok(PreparedWitnessPersistenceBottleneckStability {
        spec,
        stability,
        backend,
        worker_path,
    })
}

pub(crate) fn execute_witness_persistence_bottleneck_stability(
    prepared: &PreparedWitnessPersistenceBottleneckStability,
) -> Result<WitnessPersistenceBottleneckStabilityResult, TopologyCliError> {
    let witness_results = execute_witness_persistence_stability_runs(&prepared.stability)?;
    let stability = summarize_witness_persistence_stability(&prepared.stability, &witness_results)?;
    let baseline = &witness_results[0];
    let mut comparisons = Vec::new();
    for (replicate, perturbed) in witness_results.iter().skip(1).enumerate() {
        for dimension in 0..=prepared.spec.stability.maximum_dimension {
            let baseline_dimension = persistence_dimension(baseline, dimension)?;
            let perturbed_dimension = persistence_dimension(perturbed, dimension)?;
            comparisons.push(serde_json::json!({
                "replicate": replicate,
                "dimension": dimension,
                "baseline": {
                    "finite_pairs": baseline_dimension.finite_pairs,
                    "essential_births": baseline_dimension.essential_births,
                },
                "perturbed": {
                    "finite_pairs": perturbed_dimension.finite_pairs,
                    "essential_births": perturbed_dimension.essential_births,
                },
            }));
        }
    }
    let request = serde_json::json!({
        "format": "marklab.gudhi_witness_bottleneck_request",
        "version": 1,
        "backend": prepared.backend,
        "metric": "bottleneck_linf",
        "coefficient": 0.0,
        "comparisons": comparisons,
        "maximum_comparisons": prepared.spec.maximum_bottleneck_comparisons,
        "maximum_interval_budget": prepared.spec.maximum_bottleneck_interval_budget,
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let repository = marklab::python_backend_assets_root()?;
    let response = run_worker(
        &repository,
        &prepared.worker_path,
        &request_bytes,
        prepared.spec.bottleneck_timeout_seconds,
    )?;
    let worker: WitnessBottleneckWorkerResult = serde_json::from_slice(&response)?;
    validate_witness_bottleneck_worker_result(prepared, &worker, &request_bytes)?;

    let mut perturbations = Vec::with_capacity(prepared.spec.stability.perturbation_replicates);
    for replicate in 0..prepared.spec.stability.perturbation_replicates {
        let by_dimension = worker
            .comparisons
            .iter()
            .filter(|row| row.replicate == replicate)
            .map(|row| WitnessBottleneckDimensionResult {
                dimension: row.dimension,
                status: row.status.clone(),
                bottleneck_distance_um_squared: row.bottleneck_distance_um_squared,
            })
            .collect::<Vec<_>>();
        let maximum_finite = by_dimension
            .iter()
            .filter_map(|row| row.bottleneck_distance_um_squared)
            .fold(0.0_f64, f64::max);
        let has_infinite = by_dimension
            .iter()
            .any(|row| row.status == "infinite_essential_count_mismatch");
        perturbations.push(WitnessBottleneckPerturbationResult {
            replicate,
            by_dimension,
            maximum_finite_bottleneck_distance_um_squared: maximum_finite,
            has_infinite_essential_mismatch: has_infinite,
        });
    }
    let maximum_finite_bottleneck_distance_um_squared = perturbations
        .iter()
        .map(|row| row.maximum_finite_bottleneck_distance_um_squared)
        .fold(0.0_f64, f64::max);
    let has_infinite_essential_mismatch = perturbations
        .iter()
        .any(|row| row.has_infinite_essential_mismatch);
    let stable_under_bottleneck_threshold = !has_infinite_essential_mismatch
        && maximum_finite_bottleneck_distance_um_squared
            <= prepared.spec.maximum_bottleneck_distance_um_squared;
    let total_backend_executions = stability
        .backend_executions
        .checked_add(1)
        .ok_or_else(|| TopologyCliError::Backend("total backend execution overflow".into()))?;
    let stable_under_all_declared_thresholds =
        stable_under_bottleneck_threshold && stability.stable_under_declared_thresholds;
    let result = WitnessPersistenceBottleneckStabilityResult {
        format: "marklab.witness_persistence_bottleneck_stability".into(),
        version: 1,
        backend: worker.backend,
        bottleneck_request_sha256: worker.request_sha256,
        bottleneck_metric: worker.metric,
        essential_interval_policy: worker.essential_interval_policy,
        stability,
        perturbations,
        maximum_bottleneck_distance_um_squared_allowed: prepared
            .spec
            .maximum_bottleneck_distance_um_squared,
        maximum_finite_bottleneck_distance_um_squared,
        has_infinite_essential_mismatch,
        bottleneck_comparisons: worker.comparison_count,
        bottleneck_interval_count: worker.interval_count,
        bottleneck_backend_executions: 1,
        total_backend_executions,
        stable_under_bottleneck_threshold,
        stable_under_all_declared_thresholds,
        claim_status: "witness_coordinate_bottleneck_stability_diagnostic".into(),
    };
    validate_witness_persistence_bottleneck_stability_result(prepared, &result)?;
    Ok(result)
}

fn persistence_dimension(
    result: &WitnessPersistenceResult,
    dimension: usize,
) -> Result<&marklab_topology::PersistenceDimensionResult, TopologyCliError> {
    result
        .persistence
        .by_dimension
        .iter()
        .find(|row| row.dimension == dimension)
        .ok_or_else(|| TopologyCliError::Backend("witness persistence dimension is missing".into()))
}

fn validate_witness_bottleneck_worker_result(
    prepared: &PreparedWitnessPersistenceBottleneckStability,
    result: &WitnessBottleneckWorkerResult,
    request_bytes: &[u8],
) -> Result<(), TopologyCliError> {
    let expected_comparisons = (prepared.spec.stability.perturbation_replicates as u64)
        * (prepared.spec.stability.maximum_dimension as u64 + 1);
    let maximum_finite = result
        .comparisons
        .iter()
        .filter_map(|row| row.bottleneck_distance_um_squared)
        .fold(0.0_f64, f64::max);
    let has_infinite = result
        .comparisons
        .iter()
        .any(|row| row.status == "infinite_essential_count_mismatch");
    let mut identities = std::collections::BTreeSet::new();
    if result.format != "marklab.gudhi_witness_bottleneck_result"
        || result.version != 1
        || result.metric != "gudhi_exact_linf"
        || result.essential_interval_policy != "infinite_death_equal_counts_else_infinite_mismatch"
        || result.claim_status != "exact_witness_diagram_bottleneck_stability_diagnostic"
        || result.request_sha256 != sha256_hex(request_bytes)
        || !same_topology_backend(&result.backend, &prepared.backend)
        || result.comparison_count != expected_comparisons
        || result.comparisons.len() as u64 != expected_comparisons
        || result.comparison_count > prepared.spec.maximum_bottleneck_comparisons
        || result.interval_count > prepared.spec.maximum_bottleneck_interval_budget
        || maximum_finite.to_bits()
            != result
                .maximum_finite_bottleneck_distance_um_squared
                .to_bits()
        || has_infinite != result.has_infinite_essential_mismatch
        || result.comparisons.iter().any(|row| {
            !identities.insert((row.replicate, row.dimension))
                || row.replicate >= prepared.spec.stability.perturbation_replicates
                || row.dimension > prepared.spec.stability.maximum_dimension
                || match row.status.as_str() {
                    "finite" => row
                        .bottleneck_distance_um_squared
                        .is_none_or(|value| !value.is_finite() || value < 0.0),
                    "infinite_essential_count_mismatch" => {
                        row.bottleneck_distance_um_squared.is_some()
                    }
                    _ => true,
                }
        })
    {
        return Err(TopologyCliError::Backend(
            "witness bottleneck worker result identity or contract mismatch".into(),
        ));
    }
    Ok(())
}

fn same_topology_backend(left: &TopologyBackendContract, right: &TopologyBackendContract) -> bool {
    left.name == right.name
        && left.version == right.version
        && left.python_version == right.python_version
        && left.license == right.license
        && left.environment_lock_sha256 == right.environment_lock_sha256
        && left.worker_sha256 == right.worker_sha256
}

pub(crate) fn validate_witness_persistence_bottleneck_stability_result(
    prepared: &PreparedWitnessPersistenceBottleneckStability,
    result: &WitnessPersistenceBottleneckStabilityResult,
) -> Result<(), TopologyCliError> {
    validate_witness_persistence_stability_result(&prepared.stability, &result.stability)?;
    if result.perturbations.len() != prepared.spec.stability.perturbation_replicates {
        return Err(TopologyCliError::Backend(
            "witness bottleneck perturbation count differs".into(),
        ));
    }
    let expected_dimensions = prepared.spec.stability.maximum_dimension + 1;
    let expected_comparisons = (prepared.spec.stability.perturbation_replicates as u64)
        .checked_mul(expected_dimensions as u64)
        .ok_or_else(|| TopologyCliError::Backend("bottleneck comparison overflow".into()))?;
    let maximum_finite = result
        .perturbations
        .iter()
        .map(|row| row.maximum_finite_bottleneck_distance_um_squared)
        .fold(0.0_f64, f64::max);
    let has_infinite = result
        .perturbations
        .iter()
        .any(|row| row.has_infinite_essential_mismatch);
    let stable_bottleneck =
        !has_infinite && maximum_finite <= prepared.spec.maximum_bottleneck_distance_um_squared;
    let expected_total_executions = result
        .stability
        .backend_executions
        .checked_add(1)
        .ok_or_else(|| TopologyCliError::Backend("total backend execution overflow".into()))?;
    if result.format != "marklab.witness_persistence_bottleneck_stability"
        || result.version != 1
        || result.bottleneck_metric != "gudhi_exact_linf"
        || result.essential_interval_policy != "infinite_death_equal_counts_else_infinite_mismatch"
        || result.claim_status != "witness_coordinate_bottleneck_stability_diagnostic"
        || !same_topology_backend(&result.backend, &prepared.backend)
        || result.bottleneck_request_sha256.len() != 64
        || !result
            .bottleneck_request_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || result
            .maximum_bottleneck_distance_um_squared_allowed
            .to_bits()
            != prepared
                .spec
                .maximum_bottleneck_distance_um_squared
                .to_bits()
        || maximum_finite.to_bits()
            != result
                .maximum_finite_bottleneck_distance_um_squared
                .to_bits()
        || has_infinite != result.has_infinite_essential_mismatch
        || result.bottleneck_comparisons != expected_comparisons
        || result.bottleneck_comparisons > prepared.spec.maximum_bottleneck_comparisons
        || result.bottleneck_interval_count > prepared.spec.maximum_bottleneck_interval_budget
        || result.bottleneck_backend_executions != 1
        || result.bottleneck_backend_executions
            > prepared.spec.maximum_bottleneck_backend_executions
        || result.total_backend_executions != expected_total_executions
        || result.total_backend_executions > prepared.spec.maximum_total_backend_executions
        || result.stable_under_bottleneck_threshold != stable_bottleneck
        || result.stable_under_all_declared_thresholds
            != (stable_bottleneck && result.stability.stable_under_declared_thresholds)
        || result
            .perturbations
            .iter()
            .enumerate()
            .any(|(replicate, row)| {
                let row_maximum = row
                    .by_dimension
                    .iter()
                    .filter_map(|dimension| dimension.bottleneck_distance_um_squared)
                    .fold(0.0_f64, f64::max);
                let row_infinite = row
                    .by_dimension
                    .iter()
                    .any(|dimension| dimension.status == "infinite_essential_count_mismatch");
                let mut dimensions = std::collections::BTreeSet::new();
                row.replicate != replicate
                    || row.by_dimension.len() != expected_dimensions
                    || row_maximum.to_bits()
                        != row.maximum_finite_bottleneck_distance_um_squared.to_bits()
                    || row_infinite != row.has_infinite_essential_mismatch
                    || row.by_dimension.iter().any(|dimension| {
                        !dimensions.insert(dimension.dimension)
                            || dimension.dimension > prepared.spec.stability.maximum_dimension
                            || match dimension.status.as_str() {
                                "finite" => dimension
                                    .bottleneck_distance_um_squared
                                    .is_none_or(|value| !value.is_finite() || value < 0.0),
                                "infinite_essential_count_mismatch" => {
                                    dimension.bottleneck_distance_um_squared.is_some()
                                }
                                _ => true,
                            }
                    })
            })
    {
        return Err(TopologyCliError::Backend(
            "witness bottleneck stability result identity or contract mismatch".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_witness_bottleneck_controls(
    spec: &WitnessPersistenceBottleneckStabilitySpec,
) -> Result<(), TopologyCliError> {
    if !spec.maximum_bottleneck_distance_um_squared.is_finite()
        || spec.maximum_bottleneck_distance_um_squared < 0.0
        || spec.maximum_bottleneck_comparisons == 0
        || spec.maximum_bottleneck_interval_budget == 0
        || spec.maximum_bottleneck_backend_executions == 0
        || !(1..=3_600).contains(&spec.bottleneck_timeout_seconds)
        || spec.maximum_total_backend_executions == 0
    {
        return Err(TopologyCliError::Input(
            "witness bottleneck controls or thresholds are invalid".into(),
        ));
    }
    let comparisons = (spec.stability.perturbation_replicates as u64)
        .checked_mul(spec.stability.maximum_dimension as u64 + 1)
        .ok_or_else(|| TopologyCliError::Input("bottleneck comparison overflow".into()))?;
    if comparisons > spec.maximum_bottleneck_comparisons {
        return Err(TopologyCliError::Input(
            "bottleneck comparisons exceed caller maximum".into(),
        ));
    }
    let interval_budget = comparisons
        .checked_mul(2)
        .and_then(|value| value.checked_mul(spec.stability.maximum_simplices as u64))
        .ok_or_else(|| TopologyCliError::Input("bottleneck interval budget overflow".into()))?;
    if interval_budget > spec.maximum_bottleneck_interval_budget {
        return Err(TopologyCliError::Input(
            "bottleneck interval budget exceeds caller maximum".into(),
        ));
    }
    let total_executions = (spec.stability.perturbation_replicates as u64)
        .checked_add(2)
        .ok_or_else(|| TopologyCliError::Input("total backend execution overflow".into()))?;
    if 1 > spec.maximum_bottleneck_backend_executions
        || total_executions > spec.maximum_total_backend_executions
    {
        return Err(TopologyCliError::Input(
            "bottleneck or total backend executions exceed caller maximum".into(),
        ));
    }
    Ok(())
}
pub(super) fn run_witness_persistence_bottleneck_stability(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let prepared = prepare_witness_persistence_bottleneck_stability(&input)?;
    let result = execute_witness_persistence_bottleneck_stability(&prepared)?;
    publish_json(&out, &result)
}
