use std::path::{Path, PathBuf};

use marklab_topology::{
    sha256_hex, WitnessDimensionStabilityResult, WitnessPersistenceResult, WitnessPersistenceSpec,
    WitnessPersistenceStabilityResult, WitnessPersistenceStabilitySpec,
    WitnessPersistenceWorkerRequest, WitnessPerturbationStabilityResult,
};

use super::{
    publish_json, read_input, read_required,
    witness_persistence::{execute_witness_persistence, PreparedWitnessPersistence},
    TopologyCliError,
};

pub(crate) struct PreparedWitnessPersistenceStability {
    pub(crate) spec: WitnessPersistenceStabilitySpec,
    pub(crate) runs: Vec<PreparedWitnessPersistence>,
}
pub(crate) fn prepare_witness_persistence_stability(
    input: &Path,
) -> Result<PreparedWitnessPersistenceStability, TopologyCliError> {
    let bytes = read_input(input)?;
    let spec: WitnessPersistenceStabilitySpec = serde_json::from_slice(&bytes)?;
    prepare_witness_persistence_stability_spec(spec)
}

pub(super) fn prepare_witness_persistence_stability_spec(
    spec: WitnessPersistenceStabilitySpec,
) -> Result<PreparedWitnessPersistenceStability, TopologyCliError> {
    validate_witness_stability_controls(&spec)?;
    let repository = marklab::python_backend_assets_root()?;
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker_path = repository.join("workers/python/marklab_gudhi_witness_persistence_worker.py");
    let worker = read_required(&worker_path)?;
    let lock_digest = sha256_hex(&lock);
    let worker_digest = sha256_hex(&worker);
    let mut runs = Vec::with_capacity(spec.perturbation_replicates + 1);
    for run in 0..=spec.perturbation_replicates {
        let points = if run == 0 {
            spec.points.clone()
        } else {
            perturb_witness_points(&spec, run - 1)?.0
        };
        let request = WitnessPersistenceWorkerRequest::new(
            witness_spec(&spec, points),
            lock_digest.clone(),
            worker_digest.clone(),
        )
        .map_err(|error| TopologyCliError::Input(error.to_string()))?;
        let request_bytes = serde_json::to_vec(&request)?;
        runs.push(PreparedWitnessPersistence {
            request,
            request_bytes,
            timeout_seconds: spec.timeout_seconds,
        });
    }
    Ok(PreparedWitnessPersistenceStability { spec, runs })
}

pub(crate) fn execute_witness_persistence_stability(
    prepared: &PreparedWitnessPersistenceStability,
) -> Result<WitnessPersistenceStabilityResult, TopologyCliError> {
    let results = execute_witness_persistence_stability_runs(prepared)?;
    summarize_witness_persistence_stability(prepared, &results)
}

pub(super) fn execute_witness_persistence_stability_runs(
    prepared: &PreparedWitnessPersistenceStability,
) -> Result<Vec<WitnessPersistenceResult>, TopologyCliError> {
    let mut results = Vec::with_capacity(prepared.runs.len());
    for (index, run) in prepared.runs.iter().enumerate() {
        results.push(execute_witness_persistence(run).map_err(|error| {
            TopologyCliError::Backend(format!(
                "witness stability backend execution {index} failed: {error}"
            ))
        })?);
    }
    Ok(results)
}

pub(super) fn summarize_witness_persistence_stability(
    prepared: &PreparedWitnessPersistenceStability,
    results: &[WitnessPersistenceResult],
) -> Result<WitnessPersistenceStabilityResult, TopologyCliError> {
    if results.len() != prepared.runs.len() {
        return Err(TopologyCliError::Backend(
            "witness stability backend result count differs".into(),
        ));
    }
    let baseline = &results[0];
    let mut perturbations = Vec::with_capacity(results.len());
    for (replicate, result) in results.iter().skip(1).enumerate() {
        let (_, maximum_coordinate_displacement_um) =
            perturb_witness_points(&prepared.spec, replicate)?;
        perturbations.push(summarize_witness_perturbation(
            replicate,
            maximum_coordinate_displacement_um,
            baseline,
            result,
        )?);
    }
    let minimum_landmark_id_match_fraction = perturbations
        .iter()
        .map(|row| row.landmark_id_match_fraction)
        .fold(1.0_f64, f64::min);
    let maximum_coverage_radius_change_um = perturbations
        .iter()
        .map(|row| row.coverage_radius_change_um)
        .fold(0.0_f64, f64::max);
    let maximum_simplex_count_l1_change = perturbations
        .iter()
        .map(|row| row.simplex_count_l1_change)
        .max()
        .unwrap_or(0);
    let maximum_total_persistence_change_um_squared = perturbations
        .iter()
        .map(|row| row.maximum_total_persistence_change_um_squared)
        .fold(0.0_f64, f64::max);
    let backend_executions = prepared.runs.len() as u64;
    let total_point_work = backend_executions * prepared.spec.points.len() as u64;
    let total_simplex_budget = backend_executions * prepared.spec.maximum_simplices as u64;
    let total_timeout_seconds = backend_executions * prepared.spec.timeout_seconds;
    let result = WitnessPersistenceStabilityResult {
        format: "marklab.witness_persistence_stability".into(),
        version: 1,
        statistical_unit: "one_specimen_point_pattern".into(),
        perturbation_rule: "sha256_uniform_independent_axis_jitter".into(),
        finite_result_policy: "reject_non_finite_input_or_output".into(),
        seed: prepared.spec.seed,
        perturbation_replicates: prepared.spec.perturbation_replicates,
        maximum_coordinate_jitter_um: prepared.spec.maximum_coordinate_jitter_um,
        minimum_landmark_id_match_fraction_allowed: prepared
            .spec
            .minimum_landmark_id_match_fraction,
        maximum_coverage_radius_change_um_allowed: prepared.spec.maximum_coverage_radius_change_um,
        maximum_simplex_count_l1_change_allowed: prepared.spec.maximum_simplex_count_l1_change,
        maximum_total_persistence_change_um_squared_allowed: prepared
            .spec
            .maximum_total_persistence_change_um_squared,
        baseline: baseline.clone(),
        perturbations,
        minimum_landmark_id_match_fraction,
        maximum_coverage_radius_change_um,
        maximum_simplex_count_l1_change,
        maximum_total_persistence_change_um_squared,
        backend_executions,
        total_point_work,
        total_simplex_budget,
        total_timeout_seconds,
        stable_under_declared_thresholds: minimum_landmark_id_match_fraction
            >= prepared.spec.minimum_landmark_id_match_fraction
            && maximum_coverage_radius_change_um <= prepared.spec.maximum_coverage_radius_change_um
            && maximum_simplex_count_l1_change <= prepared.spec.maximum_simplex_count_l1_change
            && maximum_total_persistence_change_um_squared
                <= prepared.spec.maximum_total_persistence_change_um_squared,
        claim_status: "witness_coordinate_perturbation_stability_diagnostic".into(),
    };
    validate_witness_persistence_stability_result(prepared, &result)?;
    Ok(result)
}

pub(crate) fn validate_witness_persistence_stability_result(
    prepared: &PreparedWitnessPersistenceStability,
    result: &WitnessPersistenceStabilityResult,
) -> Result<(), TopologyCliError> {
    result
        .baseline
        .validate(&prepared.runs[0].request, &prepared.runs[0].request_bytes)
        .map_err(|error| TopologyCliError::Backend(error.to_string()))?;
    let minimum_match = result
        .perturbations
        .iter()
        .map(|row| row.landmark_id_match_fraction)
        .fold(1.0_f64, f64::min);
    let maximum_coverage = result
        .perturbations
        .iter()
        .map(|row| row.coverage_radius_change_um)
        .fold(0.0_f64, f64::max);
    let maximum_simplex = result
        .perturbations
        .iter()
        .map(|row| row.simplex_count_l1_change)
        .max()
        .unwrap_or(0);
    let maximum_persistence = result
        .perturbations
        .iter()
        .map(|row| row.maximum_total_persistence_change_um_squared)
        .fold(0.0_f64, f64::max);
    let executions = prepared.runs.len() as u64;
    let point_work = executions * prepared.spec.points.len() as u64;
    let simplex_budget = executions * prepared.spec.maximum_simplices as u64;
    let timeout = executions * prepared.spec.timeout_seconds;
    if result.format != "marklab.witness_persistence_stability"
        || result.version != 1
        || result.statistical_unit != "one_specimen_point_pattern"
        || result.perturbation_rule != "sha256_uniform_independent_axis_jitter"
        || result.finite_result_policy != "reject_non_finite_input_or_output"
        || result.claim_status != "witness_coordinate_perturbation_stability_diagnostic"
        || result.seed != prepared.spec.seed
        || result.perturbation_replicates != prepared.spec.perturbation_replicates
        || result.perturbations.len() != prepared.spec.perturbation_replicates
        || result.maximum_coordinate_jitter_um.to_bits()
            != prepared.spec.maximum_coordinate_jitter_um.to_bits()
        || result.minimum_landmark_id_match_fraction_allowed.to_bits()
            != prepared.spec.minimum_landmark_id_match_fraction.to_bits()
        || result.maximum_coverage_radius_change_um_allowed.to_bits()
            != prepared.spec.maximum_coverage_radius_change_um.to_bits()
        || result.maximum_simplex_count_l1_change_allowed
            != prepared.spec.maximum_simplex_count_l1_change
        || result
            .maximum_total_persistence_change_um_squared_allowed
            .to_bits()
            != prepared
                .spec
                .maximum_total_persistence_change_um_squared
                .to_bits()
        || result.backend_executions != executions
        || result.total_point_work != point_work
        || result.total_simplex_budget != simplex_budget
        || result.total_timeout_seconds != timeout
        || minimum_match.to_bits() != result.minimum_landmark_id_match_fraction.to_bits()
        || maximum_coverage.to_bits() != result.maximum_coverage_radius_change_um.to_bits()
        || maximum_simplex != result.maximum_simplex_count_l1_change
        || maximum_persistence.to_bits()
            != result.maximum_total_persistence_change_um_squared.to_bits()
        || result.stable_under_declared_thresholds
            != (minimum_match >= prepared.spec.minimum_landmark_id_match_fraction
                && maximum_coverage <= prepared.spec.maximum_coverage_radius_change_um
                && maximum_simplex <= prepared.spec.maximum_simplex_count_l1_change
                && maximum_persistence <= prepared.spec.maximum_total_persistence_change_um_squared)
        || result.perturbations.iter().enumerate().any(|(index, row)| {
            row.replicate != index
                || row.request_sha256 != sha256_hex(&prepared.runs[index + 1].request_bytes)
                || !row.maximum_coordinate_displacement_um.is_finite()
                || !row.landmark_id_match_fraction.is_finite()
                || !(0.0..=1.0).contains(&row.landmark_id_match_fraction)
                || !row.coverage_radius_change_um.is_finite()
                || !row.maximum_total_persistence_change_um_squared.is_finite()
                || row
                    .by_dimension
                    .iter()
                    .any(|dimension| !dimension.total_persistence_change_um_squared.is_finite())
        })
    {
        return Err(TopologyCliError::Backend(
            "witness stability result identity or contract mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_witness_stability_controls(
    spec: &WitnessPersistenceStabilitySpec,
) -> Result<(), TopologyCliError> {
    if !(1..=32).contains(&spec.perturbation_replicates)
        || !spec.maximum_coordinate_jitter_um.is_finite()
        || spec.maximum_coordinate_jitter_um < 0.0
        || !spec.minimum_landmark_id_match_fraction.is_finite()
        || !(0.0..=1.0).contains(&spec.minimum_landmark_id_match_fraction)
        || !spec.maximum_coverage_radius_change_um.is_finite()
        || spec.maximum_coverage_radius_change_um < 0.0
        || !spec.maximum_total_persistence_change_um_squared.is_finite()
        || spec.maximum_total_persistence_change_um_squared < 0.0
    {
        return Err(TopologyCliError::Input(
            "witness stability perturbation controls or thresholds are invalid".into(),
        ));
    }
    let executions = (spec.perturbation_replicates as u64)
        .checked_add(1)
        .ok_or_else(|| TopologyCliError::Input("backend execution count overflow".into()))?;
    if executions > spec.maximum_backend_executions {
        return Err(TopologyCliError::Input(
            "backend executions exceed caller maximum".into(),
        ));
    }
    let point_work = executions
        .checked_mul(spec.points.len() as u64)
        .ok_or_else(|| TopologyCliError::Input("aggregate point work overflow".into()))?;
    if point_work > spec.maximum_total_point_work {
        return Err(TopologyCliError::Input(
            "aggregate point work exceeds caller maximum".into(),
        ));
    }
    let simplex_budget = executions
        .checked_mul(spec.maximum_simplices as u64)
        .ok_or_else(|| TopologyCliError::Input("aggregate simplex budget overflow".into()))?;
    if simplex_budget > spec.maximum_total_simplex_budget {
        return Err(TopologyCliError::Input(
            "aggregate simplex budget exceeds caller maximum".into(),
        ));
    }
    let timeout = executions
        .checked_mul(spec.timeout_seconds)
        .ok_or_else(|| TopologyCliError::Input("aggregate timeout overflow".into()))?;
    if timeout > spec.maximum_total_timeout_seconds {
        return Err(TopologyCliError::Input(
            "aggregate timeout exceeds caller maximum".into(),
        ));
    }
    Ok(())
}

fn witness_spec(
    spec: &WitnessPersistenceStabilitySpec,
    points: Vec<marklab_topology::TopologyPointInput>,
) -> WitnessPersistenceSpec {
    WitnessPersistenceSpec {
        points,
        landmark_method: spec.landmark_method,
        landmark_count: spec.landmark_count,
        maximum_dimension: spec.maximum_dimension,
        nu: spec.nu,
        max_scale_um: spec.max_scale_um,
        coefficient_field: spec.coefficient_field,
        maximum_simplices: spec.maximum_simplices,
        timeout_seconds: spec.timeout_seconds,
    }
}

fn perturb_witness_points(
    spec: &WitnessPersistenceStabilitySpec,
    replicate: usize,
) -> Result<(Vec<marklab_topology::TopologyPointInput>, f64), TopologyCliError> {
    let mut points = spec.points.clone();
    let mut maximum_displacement = 0.0_f64;
    for point in &mut points {
        let mut squared_displacement = 0.0;
        for (axis, coordinate) in point.coordinates_um.iter_mut().enumerate() {
            let jitter = deterministic_witness_jitter(
                spec.seed,
                replicate,
                &point.id,
                axis,
                spec.maximum_coordinate_jitter_um,
            );
            *coordinate += jitter;
            squared_displacement += jitter * jitter;
        }
        if point.coordinates_um.iter().any(|value| !value.is_finite()) {
            return Err(TopologyCliError::Input(
                "witness perturbation produced a non-finite coordinate".into(),
            ));
        }
        maximum_displacement = maximum_displacement.max(squared_displacement.sqrt());
    }
    Ok((points, maximum_displacement))
}

fn deterministic_witness_jitter(
    seed: u64,
    replicate: usize,
    point_id: &str,
    axis: usize,
    maximum: f64,
) -> f64 {
    if maximum == 0.0 {
        return 0.0;
    }
    let mut framed = Vec::with_capacity(96 + point_id.len());
    framed.extend_from_slice(b"marklab-witness-persistence-stability-jitter-v1\0");
    framed.extend_from_slice(&seed.to_be_bytes());
    framed.extend_from_slice(&(replicate as u64).to_be_bytes());
    framed.extend_from_slice(&(point_id.len() as u64).to_be_bytes());
    framed.extend_from_slice(point_id.as_bytes());
    framed.extend_from_slice(&(axis as u64).to_be_bytes());
    let digest = sha256_hex(&framed);
    let bits = u64::from_str_radix(&digest[..16], 16).expect("SHA-256 prefix is hexadecimal");
    let unit = (bits >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64));
    (2.0 * unit - 1.0) * maximum
}

fn summarize_witness_perturbation(
    replicate: usize,
    maximum_coordinate_displacement_um: f64,
    baseline: &WitnessPersistenceResult,
    perturbed: &WitnessPersistenceResult,
) -> Result<WitnessPerturbationStabilityResult, TopologyCliError> {
    let baseline_landmarks = baseline
        .landmark_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let matching_landmarks = perturbed
        .landmark_ids
        .iter()
        .filter(|id| baseline_landmarks.contains(id.as_str()))
        .count();
    let landmark_id_match_fraction = matching_landmarks as f64 / baseline.landmark_ids.len() as f64;
    let coverage_radius_change_um = (perturbed.approximation.coverage_radius_um
        - baseline.approximation.coverage_radius_um)
        .abs();
    let simplex_dimensions = baseline
        .filtration
        .simplex_counts_by_dimension
        .len()
        .max(perturbed.filtration.simplex_counts_by_dimension.len());
    let simplex_count_l1_change = (0..simplex_dimensions).try_fold(0_u64, |total, dimension| {
        let left = baseline
            .filtration
            .simplex_counts_by_dimension
            .get(dimension)
            .copied()
            .unwrap_or(0) as u64;
        let right = perturbed
            .filtration
            .simplex_counts_by_dimension
            .get(dimension)
            .copied()
            .unwrap_or(0) as u64;
        total.checked_add(left.abs_diff(right))
    });
    let simplex_count_l1_change = simplex_count_l1_change
        .ok_or_else(|| TopologyCliError::Backend("witness simplex-count change overflow".into()))?;
    let baseline_dimensions = persistence_dimension_summaries(baseline)?;
    let perturbed_dimensions = persistence_dimension_summaries(perturbed)?;
    let dimensions = baseline_dimensions
        .keys()
        .chain(perturbed_dimensions.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut by_dimension = Vec::with_capacity(dimensions.len());
    for dimension in dimensions {
        let left = baseline_dimensions
            .get(&dimension)
            .copied()
            .unwrap_or((0, 0, 0.0));
        let right = perturbed_dimensions
            .get(&dimension)
            .copied()
            .unwrap_or((0, 0, 0.0));
        by_dimension.push(WitnessDimensionStabilityResult {
            dimension,
            finite_pair_count_change: left.0.abs_diff(right.0),
            essential_count_change: left.1.abs_diff(right.1),
            total_persistence_change_um_squared: (left.2 - right.2).abs(),
        });
    }
    let maximum_total_persistence_change_um_squared = by_dimension
        .iter()
        .map(|row| row.total_persistence_change_um_squared)
        .fold(0.0_f64, f64::max);
    if !landmark_id_match_fraction.is_finite()
        || !coverage_radius_change_um.is_finite()
        || !maximum_total_persistence_change_um_squared.is_finite()
    {
        return Err(TopologyCliError::Backend(
            "witness stability summary is non-finite".into(),
        ));
    }
    Ok(WitnessPerturbationStabilityResult {
        replicate,
        request_sha256: perturbed.request_sha256.clone(),
        maximum_coordinate_displacement_um,
        landmark_id_match_fraction,
        coverage_radius_change_um,
        simplex_count_l1_change,
        by_dimension,
        maximum_total_persistence_change_um_squared,
    })
}

fn persistence_dimension_summaries(
    result: &WitnessPersistenceResult,
) -> Result<std::collections::BTreeMap<usize, (u64, u64, f64)>, TopologyCliError> {
    let mut summaries = std::collections::BTreeMap::new();
    for dimension in &result.persistence.by_dimension {
        let total_persistence = dimension
            .finite_pairs
            .iter()
            .map(|pair| {
                if !pair.birth.is_finite() || !pair.death.is_finite() || pair.death < pair.birth {
                    Err(TopologyCliError::Backend(
                        "witness persistence pair is invalid".into(),
                    ))
                } else {
                    Ok(pair.death - pair.birth)
                }
            })
            .sum::<Result<f64, _>>()?;
        if !total_persistence.is_finite()
            || summaries
                .insert(
                    dimension.dimension,
                    (
                        dimension.finite_pairs.len() as u64,
                        dimension.essential_count as u64,
                        total_persistence,
                    ),
                )
                .is_some()
        {
            return Err(TopologyCliError::Backend(
                "witness persistence dimensions are invalid".into(),
            ));
        }
    }
    Ok(summaries)
}
pub(super) fn run_witness_persistence_stability(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let prepared = prepare_witness_persistence_stability(&input)?;
    let result = execute_witness_persistence_stability(&prepared)?;
    publish_json(&out, &result)
}
