use marklab_numerics::relative_eq;
use serde::Deserialize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use marklab_core::backend::versions::SCIPY_VERSION;
use marklab_core::backend::WorkerError;

const RELATIVE_TOLERANCE: f64 = 1e-10;
mod cross_covariance;
mod projected_variogram;
mod vector_semivariogram;

pub use cross_covariance::embedding_cross_covariance_by_distance;
pub use projected_variogram::{
    ProjectedEmbeddingInputIdentity, ProjectedEmbeddingPoint, ProjectedEmbeddingVariogramResult,
    ProjectedEmbeddingVariogramSpec, ProjectedEmbeddingVariogramWorkerRequest,
    ProjectedEmbeddingVariogramWorkerResult, ProjectedSplitCurve, ProjectedVariogramRow,
    ProjectionArtifact,
};
pub use vector_semivariogram::vector_semivariogram;

pub(crate) use marklab_numerics::summation::{FiniteNeumaierError, FiniteNeumaierSum};

type CompensatedSum = FiniteNeumaierSum;

/// One finite embedding vector at a spatial coordinate.
#[derive(Clone, Debug)]
pub struct EmbeddingSpatialPoint {
    /// Stable object identity.
    pub object_id: String,
    /// Object x coordinate, in micrometres.
    pub x_um: f64,
    /// Object y coordinate, in micrometres.
    pub y_um: f64,
    /// Finite feature vector in declared feature order.
    pub embedding: Vec<f64>,
}

/// Distance interval in micrometres with an explicit upper-endpoint policy.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmbeddingDistanceBin {
    /// Stable identity of the distance bin.
    pub bin_id: String,
    /// Inclusive lower distance bound, in micrometres.
    pub lower_um: f64,
    /// Upper distance bound, in micrometres.
    pub upper_um: f64,
    /// Whether the upper distance endpoint belongs to this bin.
    pub upper_inclusive: bool,
}

/// Explicit nonnegative contribution weight for an identified object pair.
#[derive(Clone, Debug)]
pub struct EmbeddingPairWeight {
    /// Identity of the left object in the weighted pair.
    pub left_object_id: String,
    /// Identity of the right object in the weighted pair.
    pub right_object_id: String,
    /// Nonnegative contribution weight for this pair.
    pub weight: f64,
}

/// Spatial vectors, distance bins, optional pair weights, and a pair-work bound.
#[derive(Clone, Debug)]
pub struct VectorSemivariogramSpec {
    /// Input spatial points and their embedding vectors.
    pub points: Vec<EmbeddingSpatialPoint>,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Ordered, nonoverlapping distance bins.
    pub bins: Vec<EmbeddingDistanceBin>,
    /// Optional explicitly supplied object-pair weights.
    pub weights: Option<Vec<EmbeddingPairWeight>>,
    /// Maximum permitted spatial pair evaluations.
    pub maximum_pair_visits: u64,
}

/// Weighted squared embedding differences for one distance bin.
#[derive(Clone, Debug, Serialize)]
pub struct VectorSemivariogramRow {
    /// Stable identity of the distance bin.
    pub bin_id: String,
    /// Inclusive lower distance bound, in micrometres.
    pub lower_um: f64,
    /// Upper distance bound, in micrometres.
    pub upper_um: f64,
    /// Whether the upper distance endpoint belongs to this bin.
    pub upper_inclusive: bool,
    /// Number of contributing pairs.
    pub pair_count: u64,
    /// Sum of contributing pair weights.
    pub weight_sum: f64,
    /// Half the mean squared vector difference, absent when no pairs contribute.
    pub semivariance: Option<f64>,
    /// Whether this bin contains enough valid information for the declared inference.
    pub inference_eligible: bool,
}

/// Rotation-invariant vector semivariogram with pair-work accounting.
#[derive(Clone, Debug, Serialize)]
pub struct VectorSemivariogramResult {
    /// Physical coordinate unit recorded by the result.
    pub coordinate_unit: &'static str,
    /// Number of input objects represented by this result.
    pub object_count: u32,
    /// Number of features in each embedding vector.
    pub embedding_dimension: u32,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Number of spatial pairs examined.
    pub pair_visits: u64,
    /// Recorded pair-weighting convention.
    pub weighting: &'static str,
    /// Whether orthogonal rotation of embedding coordinates leaves the statistic unchanged.
    pub rotation_invariant: bool,
    /// Ordered distance-bin summaries.
    pub curve: Vec<VectorSemivariogramRow>,
}

/// Spatial embedding rows and resource bounds for cross-covariance calculation.
#[derive(Clone, Debug)]
pub struct EmbeddingCrossCovarianceSpec {
    /// Input spatial points and their embedding vectors.
    pub points: Vec<EmbeddingSpatialPoint>,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Ordered, nonoverlapping distance bins.
    pub bins: Vec<EmbeddingDistanceBin>,
    /// Maximum permitted spatial pair evaluations.
    pub maximum_pair_visits: u64,
    /// Maximum number of retained covariance-matrix elements.
    pub maximum_matrix_elements: u64,
}

/// Trace and Frobenius norm of one distance-binned covariance matrix.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceSummary {
    /// Stable identity of the distance bin.
    pub bin_id: String,
    /// Inclusive lower distance bound, in micrometres.
    pub lower_um: f64,
    /// Upper distance bound, in micrometres.
    pub upper_um: f64,
    /// Whether the upper distance endpoint belongs to this bin.
    pub upper_inclusive: bool,
    /// Number of contributing pairs.
    pub pair_count: u64,
    /// Covariance-matrix trace, absent when no pairs contribute.
    pub trace: Option<f64>,
    /// Frobenius norm of the covariance matrix, absent when no pairs contribute.
    pub frobenius_norm: Option<f64>,
}

/// Symmetrized embedding covariance matrix for one distance bin.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceMatrix {
    /// Stable identity of the distance bin.
    pub bin_id: String,
    /// Number of contributing pairs.
    pub pair_count: u64,
    /// Feature covariance matrix, absent when no pairs contribute.
    pub matrix: Option<Vec<Vec<f64>>>,
}

/// Versioned collection of covariance matrices in declared feature order.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceMatrixArtifact {
    /// Versioned document-format identifier.
    pub format: &'static str,
    /// Version of the recorded document format.
    pub version: u32,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Covariance matrices in distance-bin order.
    pub matrices: Vec<EmbeddingCrossCovarianceMatrix>,
}

/// Distance-binned embedding cross-covariance summaries and matrices.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceResult {
    /// Physical coordinate unit recorded by the result.
    pub coordinate_unit: &'static str,
    /// Number of input objects represented by this result.
    pub object_count: u32,
    /// Number of features in each embedding vector.
    pub embedding_dimension: u32,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Number of spatial pairs examined.
    pub pair_visits: u64,
    /// Feature means used to center the spatial embedding vectors.
    pub global_mean: Vec<f64>,
    /// Recorded convention for symmetrizing cross-covariance.
    pub symmetrization: &'static str,
    /// Distance-bin summaries in the declared bin order.
    pub summaries: Vec<EmbeddingCrossCovarianceSummary>,
    /// Versioned artifact containing the full covariance matrices.
    pub matrix_artifact: EmbeddingCrossCovarianceMatrixArtifact,
}

/// Invalid spatial embedding input or non-finite pair arithmetic.
#[derive(Debug, Error)]
pub enum EmbeddingSpatialError {
    /// The supplied specification or data is invalid.
    #[error("invalid embedding spatial analysis: {0}")]
    Invalid(String),
    /// A numerical contribution or accumulation is non-finite.
    #[error("embedding spatial analysis exceeded its numeric range: {0}")]
    Numeric(String),
}

impl From<FiniteNeumaierError> for EmbeddingSpatialError {
    fn from(error: FiniteNeumaierError) -> Self {
        let message = match error {
            FiniteNeumaierError::NonFiniteInput => "a weighted pair contribution is non-finite",
            FiniteNeumaierError::SumOverflow => "a compensated pair sum overflowed",
            FiniteNeumaierError::CorrectionOverflow => "a compensated pair correction overflowed",
            FiniteNeumaierError::TotalOverflow => "a compensated pair total overflowed",
        };
        Self::Numeric(message.into())
    }
}

fn validate_points_and_features(
    points: &mut [EmbeddingSpatialPoint],
    feature_names: &[String],
    maximum_points: usize,
) -> Result<usize, EmbeddingSpatialError> {
    if !(2..=maximum_points).contains(&points.len()) || !(2..=4_096).contains(&feature_names.len())
    {
        return Err(EmbeddingSpatialError::Invalid(
            "object count or embedding dimension is outside the supported bounds".into(),
        ));
    }
    let mut names = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !names.insert(name.as_str())
    }) {
        return Err(EmbeddingSpatialError::Invalid(
            "embedding feature names must be exact, unique embedding_* names".into(),
        ));
    }
    points.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let mut object_ids = HashSet::new();
    for point in points.iter() {
        if point.object_id.is_empty()
            || point.object_id.trim() != point.object_id
            || !object_ids.insert(point.object_id.as_str())
            || point.embedding.len() != feature_names.len()
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || point.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(EmbeddingSpatialError::Invalid(
                "embedding rows require unique exact IDs, finite coordinates, and complete finite vectors"
                    .into(),
            ));
        }
    }
    Ok(feature_names.len())
}

fn validate_bins(bins: &mut [EmbeddingDistanceBin]) -> Result<(), EmbeddingSpatialError> {
    if !(1..=256).contains(&bins.len()) {
        return Err(EmbeddingSpatialError::Invalid(
            "distance bin count is outside 1..=256".into(),
        ));
    }
    let mut identifiers = HashSet::new();
    for index in 0..bins.len() {
        let bin = &bins[index];
        if bin.bin_id.is_empty()
            || bin.bin_id.trim() != bin.bin_id
            || !identifiers.insert(bin.bin_id.as_str())
            || !bin.lower_um.is_finite()
            || !bin.upper_um.is_finite()
            || bin.lower_um < 0.0
            || bin.lower_um >= bin.upper_um
            || (index > 0 && bin.lower_um.to_bits() != bins[index - 1].upper_um.to_bits())
        {
            return Err(EmbeddingSpatialError::Invalid(
                "distance bins must have unique IDs and contiguous increasing nonnegative boundaries"
                    .into(),
            ));
        }
    }
    let last = bins.len() - 1;
    for (index, bin) in bins.iter_mut().enumerate() {
        bin.upper_inclusive = index == last;
    }
    Ok(())
}

fn build_weight_map(
    weights: Option<Vec<EmbeddingPairWeight>>,
    points: &[EmbeddingSpatialPoint],
) -> Result<HashMap<(String, String), f64>, EmbeddingSpatialError> {
    let object_ids = points
        .iter()
        .map(|point| point.object_id.as_str())
        .collect::<HashSet<_>>();
    let mut map = HashMap::new();
    for row in weights.unwrap_or_default() {
        if row.left_object_id == row.right_object_id
            || !object_ids.contains(row.left_object_id.as_str())
            || !object_ids.contains(row.right_object_id.as_str())
            || !row.weight.is_finite()
            || row.weight <= 0.0
        {
            return Err(EmbeddingSpatialError::Invalid(
                "pair weights require two known distinct IDs and a finite positive value".into(),
            ));
        }
        let pair = if row.left_object_id < row.right_object_id {
            (row.left_object_id, row.right_object_id)
        } else {
            (row.right_object_id, row.left_object_id)
        };
        if map.insert(pair, row.weight).is_some() {
            return Err(EmbeddingSpatialError::Invalid(
                "pair weights contain a duplicate unordered pair".into(),
            ));
        }
    }
    Ok(map)
}

pub(crate) fn find_bin(distance: f64, bins: &[EmbeddingDistanceBin]) -> Option<usize> {
    bins.iter().position(|bin| {
        distance >= bin.lower_um
            && (distance < bin.upper_um || (bin.upper_inclusive && distance <= bin.upper_um))
    })
}

fn stable_squared_distance(left: &[f64], right: &[f64]) -> Result<f64, EmbeddingSpatialError> {
    let mut sum = CompensatedSum::default();
    for (&left_value, &right_value) in left.iter().zip(right) {
        let difference = left_value - right_value;
        let square = difference * difference;
        sum.add(square)?;
    }
    Ok(sum.total()?)
}

fn projected_sum_error(error: FiniteNeumaierError) -> WorkerError {
    WorkerError::WorkerContract(EmbeddingSpatialError::from(error).to_string())
}

fn validate_projected_feature_names(feature_names: &[String]) -> Result<(), WorkerError> {
    let mut names = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !names.insert(name.as_str())
    }) {
        return Err(WorkerError::InvalidSpec(
            "projected variogram feature names must be unique exact embedding_* names".into(),
        ));
    }
    Ok(())
}

fn validate_projected_points(
    points: &[ProjectedEmbeddingPoint],
    dimension: usize,
) -> Result<(), WorkerError> {
    let mut object_ids = HashSet::new();
    for point in points {
        if point.object_id.is_empty()
            || point.object_id.trim() != point.object_id
            || !object_ids.insert(point.object_id.as_str())
            || point.biological_unit.is_empty()
            || point.biological_unit.trim() != point.biological_unit
            || !matches!(point.split.as_str(), "train" | "validation" | "test")
            || point.permutation_stratum.is_empty()
            || point.permutation_stratum.trim() != point.permutation_stratum
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || point.embedding.len() != dimension
            || point.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(WorkerError::InvalidSpec(
                "projected variogram rows require unique exact IDs, closed splits, finite coordinates, and complete finite vectors"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn projected_pair_counts(
    points: &[&ProjectedEmbeddingPoint],
    bins: &[EmbeddingDistanceBin],
) -> Result<Vec<u64>, WorkerError> {
    let mut counts = vec![0_u64; bins.len()];
    for left in 0..points.len() {
        for right in &points[(left + 1)..] {
            if points[left].biological_unit != right.biological_unit {
                continue;
            }
            let distance = (points[left].x_um - right.x_um).hypot(points[left].y_um - right.y_um);
            if !distance.is_finite() {
                return Err(WorkerError::WorkerContract(
                    "projected variogram pair distance is non-finite".into(),
                ));
            }
            if let Some(bin) = find_bin(distance, bins) {
                counts[bin] += 1;
            }
        }
    }
    Ok(counts)
}

fn projected_semivariogram_values(
    points: &[&ProjectedEmbeddingPoint],
    bins: &[EmbeddingDistanceBin],
    artifact: &ProjectionArtifact,
) -> Result<Vec<Vec<Option<f64>>>, WorkerError> {
    let component_count = artifact.components.len();
    let mut projected = vec![vec![0.0; component_count]; points.len()];
    for (point_index, point) in points.iter().enumerate() {
        for (component_index, component) in artifact.components.iter().enumerate() {
            let mut sum = CompensatedSum::default();
            for (feature, loading) in component.iter().enumerate() {
                sum.add((point.embedding[feature] - artifact.center[feature]) * loading)
                    .map_err(projected_sum_error)?;
            }
            projected[point_index][component_index] = sum.total().map_err(projected_sum_error)?;
        }
    }
    let mut counts = vec![0_u64; bins.len()];
    let mut sums = vec![vec![CompensatedSum::default(); bins.len()]; component_count];
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            if points[left].biological_unit != points[right].biological_unit {
                continue;
            }
            let distance = (points[left].x_um - points[right].x_um)
                .hypot(points[left].y_um - points[right].y_um);
            let Some(bin) = find_bin(distance, bins) else {
                continue;
            };
            counts[bin] += 1;
            for component in 0..component_count {
                let difference = projected[left][component] - projected[right][component];
                sums[component][bin]
                    .add(0.5 * difference * difference)
                    .map_err(projected_sum_error)?;
            }
        }
    }
    let mut result = vec![vec![None; bins.len()]; component_count];
    for component in 0..component_count {
        for bin in 0..bins.len() {
            if counts[bin] > 0 {
                result[component][bin] = Some(
                    sums[component][bin].total().map_err(projected_sum_error)? / counts[bin] as f64,
                );
            }
        }
    }
    Ok(result)
}

fn valid_optional_nonnegative(value: Option<f64>) -> bool {
    value.is_none_or(|value| value.is_finite() && value >= 0.0)
}

fn valid_optional_positive(value: Option<f64>) -> bool {
    value.is_none_or(|value| value.is_finite() && value > 0.0)
}

fn optional_approximately_equal(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => relative_eq(left, right, RELATIVE_TOLERANCE),
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bin_is_unavailable_instead_of_nan() {
        let result = vector_semivariogram(VectorSemivariogramSpec {
            points: vec![
                EmbeddingSpatialPoint {
                    object_id: "a".into(),
                    x_um: 0.0,
                    y_um: 0.0,
                    embedding: vec![0.0, 0.0],
                },
                EmbeddingSpatialPoint {
                    object_id: "b".into(),
                    x_um: 1.0,
                    y_um: 0.0,
                    embedding: vec![1.0, 1.0],
                },
            ],
            feature_names: vec!["embedding_0".into(), "embedding_1".into()],
            bins: vec![EmbeddingDistanceBin {
                bin_id: "empty".into(),
                lower_um: 2.0,
                upper_um: 3.0,
                upper_inclusive: false,
            }],
            weights: None,
            maximum_pair_visits: 1,
        })
        .unwrap();
        assert_eq!(result.curve[0].pair_count, 0);
        assert_eq!(result.curve[0].semivariance, None);
        assert_eq!(result.curve[0].weight_sum, 0.0);
    }

    #[test]
    fn whole_slide_vectors_are_bounded_by_declared_pair_work() {
        let error = vector_semivariogram(VectorSemivariogramSpec {
            points: (0..10_001)
                .map(|i| EmbeddingSpatialPoint {
                    object_id: format!("p{i}"),
                    x_um: i as f64,
                    y_um: 0.0,
                    embedding: vec![0.0, 1.0],
                })
                .collect(),
            feature_names: vec!["embedding_0".into(), "embedding_1".into()],
            bins: vec![EmbeddingDistanceBin {
                bin_id: "all".into(),
                lower_um: 0.0,
                upper_um: 1.0,
                upper_inclusive: true,
            }],
            weights: None,
            maximum_pair_visits: 1,
        })
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("50005000 unordered pair visits exceed maximum_pair_visits 1"));
    }
}
