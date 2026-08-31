use std::collections::{BTreeMap, HashSet};

use marklab_cohort::InferenceDesign;
use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{geom::spatial_index::SpatialIndex2D, ObservationWindow2D};

/// One complete spatially indexed multivariate mark row.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalMultivariateMoranPoint {
    pub point_id: String,
    pub permutation_stratum: String,
    pub x_um: f64,
    pub y_um: f64,
    pub values: Vec<f64>,
}

/// Prespecified inference and resource controls for local multivariate Moran statistics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalMultivariateMoranLimits {
    pub maximum_points: usize,
    pub maximum_dimension: usize,
    pub maximum_directed_edges: usize,
    pub maximum_permutation_edge_evaluations: usize,
    pub memory_budget_bytes: usize,
}

/// One local statistic with marginal and single-step familywise p-values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalMultivariateMoranLocation {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub statistic: f64,
    pub raw_p_value: f64,
    pub adjusted_p_value: f64,
}

/// Complete result of one bounded within-specimen local multivariate analysis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalMultivariateMoranResult {
    pub format: String,
    pub version: u32,
    pub statistical_unit: String,
    pub population_claim: String,
    pub null: String,
    pub multiplicity: String,
    pub assumptions: Vec<String>,
    pub failure_policy: String,
    pub feature_names: Vec<String>,
    pub feature_means: Vec<f64>,
    pub feature_standard_deviations: Vec<f64>,
    pub radius_um: f64,
    pub point_count: usize,
    pub dimension: usize,
    pub stratum_count: usize,
    pub directed_edge_count: usize,
    pub weights_sha256: String,
    pub window_sha256: String,
    pub permutations_requested: usize,
    pub permutations_completed: usize,
    pub seed: u64,
    pub permutation_edge_evaluations: usize,
    pub retained_memory_bytes: usize,
    pub global_max_abs_statistic: f64,
    pub global_max_abs_p_value: f64,
    pub locations: Vec<LocalMultivariateMoranLocation>,
}

/// Rejected input, inference design, resource request, or numerical state.
#[derive(Debug, Error, PartialEq)]
pub enum LocalMultivariateMoranError {
    #[error("invalid local multivariate Moran input: {0}")]
    Invalid(String),
    #[error("local multivariate Moran resource limit exceeded: {0}")]
    Resource(String),
    #[error("local multivariate Moran geometry failed: {0}")]
    Geometry(String),
    #[error("local multivariate Moran numerical failure: {0}")]
    Numerical(String),
}

/// Compute row-standardized local multivariate Moran statistics and one Max-T family.
///
/// Each complete feature row is standardized once, then moved atomically within
/// its exact declared stratum under the random-labeling null. The result is a
/// within-specimen field diagnostic and does not turn point rows into population
/// replicates.
#[allow(clippy::too_many_arguments)]
pub fn local_multivariate_moran_permutation(
    points: &[LocalMultivariateMoranPoint],
    feature_names: &[String],
    window: &ObservationWindow2D,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    limits: LocalMultivariateMoranLimits,
) -> Result<LocalMultivariateMoranResult, LocalMultivariateMoranError> {
    let dimension = validate(
        points,
        feature_names,
        window,
        radius_um,
        permutations,
        limits,
    )?;
    let retained_memory_bytes =
        retained_memory_estimate(points.len(), dimension, limits.maximum_directed_edges)?;
    if retained_memory_bytes > limits.memory_budget_bytes {
        return Err(LocalMultivariateMoranError::Resource(format!(
            "retained-memory estimate {retained_memory_bytes} exceeds budget {}",
            limits.memory_budget_bytes
        )));
    }

    let x = points.iter().map(|point| point.x_um).collect::<Vec<_>>();
    let y = points.iter().map(|point| point.y_um).collect::<Vec<_>>();
    let index = SpatialIndex2D::new(&x, &y)
        .map_err(|error| LocalMultivariateMoranError::Geometry(error.to_string()))?;
    let mut neighbors = Vec::<Vec<usize>>::with_capacity(points.len());
    let mut directed_edges = 0_usize;
    for row in 0..points.len() {
        let row_neighbors = index
            .within_radius(row, radius_um)
            .map_err(|error| LocalMultivariateMoranError::Geometry(error.to_string()))?
            .into_iter()
            .map(|neighbor| neighbor.index)
            .collect::<Vec<_>>();
        if row_neighbors.is_empty() {
            return Err(LocalMultivariateMoranError::Geometry(format!(
                "point row {row} is isolated under radius {radius_um}"
            )));
        }
        directed_edges = directed_edges
            .checked_add(row_neighbors.len())
            .ok_or_else(|| LocalMultivariateMoranError::Resource("edge count overflowed".into()))?;
        if directed_edges > limits.maximum_directed_edges {
            return Err(LocalMultivariateMoranError::Resource(format!(
                "directed-edge count {directed_edges} exceeds maximum {}",
                limits.maximum_directed_edges
            )));
        }
        neighbors.push(row_neighbors);
    }
    let permutation_edge_evaluations =
        directed_edges.checked_mul(permutations).ok_or_else(|| {
            LocalMultivariateMoranError::Resource("permutation work overflowed".into())
        })?;
    if permutation_edge_evaluations > limits.maximum_permutation_edge_evaluations {
        return Err(LocalMultivariateMoranError::Resource(format!(
            "{permutation_edge_evaluations} permutation edge evaluations exceed maximum {}",
            limits.maximum_permutation_edge_evaluations
        )));
    }

    let (standardized, feature_means, feature_standard_deviations) =
        standardize(points, dimension)?;
    let strata = dense_strata(points)?;
    let design =
        InferenceDesign::stratified_multivariate_random_labeling_max_t(&strata, permutations, seed)
            .map_err(|error| LocalMultivariateMoranError::Invalid(error.to_string()))?;
    let identity = (0..points.len()).collect::<Vec<_>>();
    let observed = evaluate(&standardized, &neighbors, &identity, dimension)?;
    let observed_max = observed.iter().copied().map(f64::abs).fold(0.0, f64::max);
    let mut raw_extreme = vec![0_usize; points.len()];
    let mut adjusted_extreme = vec![0_usize; points.len()];
    let mut global_extreme = 0_usize;
    for replicate in 0..permutations {
        let permutation = design
            .permuted_indices(replicate)
            .map_err(|error| LocalMultivariateMoranError::Invalid(error.to_string()))?;
        let candidate = evaluate(&standardized, &neighbors, &permutation, dimension)?;
        let candidate_max = candidate.iter().copied().map(f64::abs).fold(0.0, f64::max);
        global_extreme += usize::from(candidate_max >= observed_max);
        for row in 0..points.len() {
            raw_extreme[row] += usize::from(candidate[row].abs() >= observed[row].abs());
            adjusted_extreme[row] += usize::from(candidate_max >= observed[row].abs());
        }
    }
    let denominator = permutations as f64 + 1.0;
    let global_max_abs_p_value = (global_extreme as f64 + 1.0) / denominator;
    let locations = points
        .iter()
        .enumerate()
        .map(|(row, point)| LocalMultivariateMoranLocation {
            point_id: point.point_id.clone(),
            x_um: point.x_um,
            y_um: point.y_um,
            statistic: observed[row],
            raw_p_value: (raw_extreme[row] as f64 + 1.0) / denominator,
            adjusted_p_value: (adjusted_extreme[row] as f64 + 1.0) / denominator,
        })
        .collect::<Vec<_>>();
    if locations.iter().any(|row| {
        !row.statistic.is_finite()
            || !row.raw_p_value.is_finite()
            || !row.adjusted_p_value.is_finite()
            || row.raw_p_value > row.adjusted_p_value
            || !(0.0..=1.0).contains(&row.raw_p_value)
            || !(0.0..=1.0).contains(&row.adjusted_p_value)
    }) || !global_max_abs_p_value.is_finite()
    {
        return Err(LocalMultivariateMoranError::Numerical(
            "non-finite or incoherent permutation result".into(),
        ));
    }

    let weights_sha256 = weights_digest(radius_um, &neighbors)?.to_string();
    Ok(LocalMultivariateMoranResult {
        format: "marklab.local_multivariate_moran".into(),
        version: 1,
        statistical_unit: "complete_multivariate_mark_row".into(),
        population_claim: "within_specimen_field_diagnostic_only".into(),
        null: "complete_rows_randomly_labeled_within_declared_strata".into(),
        multiplicity: "single_step_max_abs_over_all_locations".into(),
        assumptions: vec![
            "fixed physical locations and exact observation window".into(),
            "complete feature rows are exchangeable only within declared strata".into(),
            "feature scaling and radius are prespecified before permutation".into(),
        ],
        failure_policy:
            "reject nonfinite, degenerate, isolated, out_of_window, or over_limit inputs".into(),
        feature_names: feature_names.to_vec(),
        feature_means,
        feature_standard_deviations,
        radius_um,
        point_count: points.len(),
        dimension,
        stratum_count: design.block_count(),
        directed_edge_count: directed_edges,
        weights_sha256,
        window_sha256: window.descriptor().logical_digest.to_string(),
        permutations_requested: permutations,
        permutations_completed: permutations,
        seed,
        permutation_edge_evaluations,
        retained_memory_bytes,
        global_max_abs_statistic: observed_max,
        global_max_abs_p_value,
        locations,
    })
}

fn validate(
    points: &[LocalMultivariateMoranPoint],
    feature_names: &[String],
    window: &ObservationWindow2D,
    radius_um: f64,
    permutations: usize,
    limits: LocalMultivariateMoranLimits,
) -> Result<usize, LocalMultivariateMoranError> {
    if points.len() < 3 {
        return Err(LocalMultivariateMoranError::Invalid(
            "at least three point rows are required".into(),
        ));
    }
    if points.len() > limits.maximum_points {
        return Err(LocalMultivariateMoranError::Resource(format!(
            "{} points exceed maximum {}",
            points.len(),
            limits.maximum_points
        )));
    }
    let dimension = feature_names.len();
    if dimension < 2 || dimension > limits.maximum_dimension {
        return Err(LocalMultivariateMoranError::Resource(format!(
            "dimension {dimension} must be between 2 and {}",
            limits.maximum_dimension
        )));
    }
    if !radius_um.is_finite()
        || radius_um <= 0.0
        || permutations == 0
        || limits.maximum_points == 0
        || limits.maximum_directed_edges == 0
        || limits.maximum_permutation_edge_evaluations == 0
        || limits.memory_budget_bytes == 0
    {
        return Err(LocalMultivariateMoranError::Invalid(
            "radius, permutations, and every resource ceiling must be positive".into(),
        ));
    }
    let mut names = HashSet::new();
    if feature_names
        .iter()
        .any(|name| !valid_text(name) || !names.insert(name.as_str()))
    {
        return Err(LocalMultivariateMoranError::Invalid(
            "feature names must be unique, trimmed, non-control text".into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut coordinates = HashSet::new();
    for (row, point) in points.iter().enumerate() {
        if !valid_text(&point.point_id)
            || !valid_text(&point.permutation_stratum)
            || !ids.insert(point.point_id.as_str())
        {
            return Err(LocalMultivariateMoranError::Invalid(format!(
                "point row {row} has an invalid or duplicate identity/stratum"
            )));
        }
        if !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || !window.contains(point.x_um, point.y_um)
        {
            return Err(LocalMultivariateMoranError::Geometry(format!(
                "point row {row} is nonfinite or outside the exact observation window"
            )));
        }
        if !coordinates.insert((point.x_um.to_bits(), point.y_um.to_bits())) {
            return Err(LocalMultivariateMoranError::Geometry(format!(
                "point row {row} duplicates a physical coordinate"
            )));
        }
        if point.values.len() != dimension || point.values.iter().any(|value| !value.is_finite()) {
            return Err(LocalMultivariateMoranError::Invalid(format!(
                "point row {row} does not contain the complete finite feature vector"
            )));
        }
    }
    Ok(dimension)
}

type StandardizedData = (Vec<Vec<f64>>, Vec<f64>, Vec<f64>);

fn standardize(
    points: &[LocalMultivariateMoranPoint],
    dimension: usize,
) -> Result<StandardizedData, LocalMultivariateMoranError> {
    let mut means = vec![0.0; dimension];
    for (feature, mean) in means.iter_mut().enumerate() {
        *mean =
            compensated_sum(points.iter().map(|point| point.values[feature])) / points.len() as f64;
    }
    let mut standard_deviations = vec![0.0; dimension];
    for (feature, standard_deviation) in standard_deviations.iter_mut().enumerate() {
        *standard_deviation = (compensated_sum(points.iter().map(|point| {
            let centered = point.values[feature] - means[feature];
            centered * centered
        })) / points.len() as f64)
            .sqrt();
        if !standard_deviation.is_finite() || *standard_deviation <= 0.0 {
            return Err(LocalMultivariateMoranError::Invalid(format!(
                "feature {} has zero or nonfinite variance",
                feature
            )));
        }
    }
    let standardized = points
        .iter()
        .map(|point| {
            (0..dimension)
                .map(|feature| {
                    (point.values[feature] - means[feature]) / standard_deviations[feature]
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    Ok((standardized, means, standard_deviations))
}

fn dense_strata(
    points: &[LocalMultivariateMoranPoint],
) -> Result<Vec<u32>, LocalMultivariateMoranError> {
    let mut labels = BTreeMap::<&str, u32>::new();
    for point in points {
        if !labels.contains_key(point.permutation_stratum.as_str()) {
            let next = u32::try_from(labels.len()).map_err(|_| {
                LocalMultivariateMoranError::Resource("stratum count exceeds u32".into())
            })?;
            labels.insert(point.permutation_stratum.as_str(), next);
        }
    }
    Ok(points
        .iter()
        .map(|point| labels[point.permutation_stratum.as_str()])
        .collect())
}

fn evaluate(
    standardized: &[Vec<f64>],
    neighbors: &[Vec<usize>],
    permutation: &[usize],
    dimension: usize,
) -> Result<Vec<f64>, LocalMultivariateMoranError> {
    let mut result = Vec::with_capacity(standardized.len());
    for row in 0..standardized.len() {
        let source = &standardized[permutation[row]];
        let inverse_degree = 1.0 / neighbors[row].len() as f64;
        let statistic = compensated_sum((0..dimension).map(|feature| {
            let neighbor_mean = compensated_sum(
                neighbors[row]
                    .iter()
                    .map(|neighbor| standardized[permutation[*neighbor]][feature]),
            ) * inverse_degree;
            source[feature] * neighbor_mean
        })) / dimension as f64;
        if !statistic.is_finite() {
            return Err(LocalMultivariateMoranError::Numerical(format!(
                "location {row} produced a nonfinite statistic"
            )));
        }
        result.push(statistic);
    }
    Ok(result)
}

fn retained_memory_estimate(
    point_count: usize,
    dimension: usize,
    maximum_directed_edges: usize,
) -> Result<usize, LocalMultivariateMoranError> {
    let matrix = point_count
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| LocalMultivariateMoranError::Resource("matrix size overflowed".into()))?;
    let edges = maximum_directed_edges
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| LocalMultivariateMoranError::Resource("edge size overflowed".into()))?;
    matrix
        .checked_mul(2)
        .and_then(|value| value.checked_add(edges))
        .and_then(|value| {
            value.checked_add(SpatialIndex2D::estimated_storage_bytes_for_len(point_count))
        })
        .and_then(|value| value.checked_add(point_count.saturating_mul(64)))
        .ok_or_else(|| LocalMultivariateMoranError::Resource("memory estimate overflowed".into()))
}

fn weights_digest(
    radius_um: f64,
    neighbors: &[Vec<usize>],
) -> Result<ContentDigest, LocalMultivariateMoranError> {
    let mut fields = Vec::<Vec<u8>>::new();
    fields.push(b"marklab-local-multivariate-moran-row-standardized-v1".to_vec());
    fields.push(radius_um.to_bits().to_be_bytes().to_vec());
    for (row, row_neighbors) in neighbors.iter().enumerate() {
        for neighbor in row_neighbors {
            let left = u64::try_from(row).map_err(|_| {
                LocalMultivariateMoranError::Resource("edge index overflowed".into())
            })?;
            let right = u64::try_from(*neighbor).map_err(|_| {
                LocalMultivariateMoranError::Resource("edge index overflowed".into())
            })?;
            let mut edge = Vec::with_capacity(16);
            edge.extend_from_slice(&left.to_be_bytes());
            edge.extend_from_slice(&right.to_be_bytes());
            fields.push(edge);
        }
    }
    Ok(ContentDigest::from_framed(fields.iter().map(Vec::as_slice)))
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObservationWindowLimits;

    fn fixture() -> (
        Vec<LocalMultivariateMoranPoint>,
        Vec<String>,
        ObservationWindow2D,
    ) {
        let points = [[-1.0, -1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, 1.0]]
            .into_iter()
            .enumerate()
            .map(|(row, values)| LocalMultivariateMoranPoint {
                point_id: format!("p{row}"),
                permutation_stratum: "tumor".into(),
                x_um: row as f64,
                y_um: 0.0,
                values: values.to_vec(),
            })
            .collect();
        let window = ObservationWindow2D::from_geojson_str(
            r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
            ObservationWindowLimits::default(),
        )
        .expect("window");
        (
            points,
            vec!["embedding_0".into(), "embedding_1".into()],
            window,
        )
    }

    #[test]
    fn sparse_result_matches_independent_four_point_hand_calculation() {
        let (points, features, window) = fixture();
        let result = local_multivariate_moran_permutation(
            &points,
            &features,
            &window,
            1.1,
            31,
            7103,
            LocalMultivariateMoranLimits {
                maximum_points: 4,
                maximum_dimension: 2,
                maximum_directed_edges: 6,
                maximum_permutation_edge_evaluations: 186,
                memory_budget_bytes: 8 * 1024 * 1024,
            },
        )
        .expect("result");
        let observed = result
            .locations
            .iter()
            .map(|row| row.statistic)
            .collect::<Vec<_>>();
        assert_eq!(observed, vec![1.0, 0.0, 0.0, 1.0]);
        assert!(result
            .locations
            .iter()
            .all(|row| row.raw_p_value <= row.adjusted_p_value));
    }
}
