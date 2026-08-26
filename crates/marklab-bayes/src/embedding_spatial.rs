use serde::Deserialize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use crate::{BackendContract, BayesError, WorkerBackend};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug)]
pub struct EmbeddingSpatialPoint {
    pub object_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub embedding: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingDistanceBin {
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
}

#[derive(Clone, Debug)]
pub struct EmbeddingPairWeight {
    pub left_object_id: String,
    pub right_object_id: String,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct VectorSemivariogramSpec {
    pub points: Vec<EmbeddingSpatialPoint>,
    pub feature_names: Vec<String>,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub weights: Option<Vec<EmbeddingPairWeight>>,
    pub maximum_pair_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VectorSemivariogramRow {
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
    pub pair_count: u64,
    pub weight_sum: f64,
    pub semivariance: Option<f64>,
    pub inference_eligible: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct VectorSemivariogramResult {
    pub coordinate_unit: &'static str,
    pub object_count: u32,
    pub embedding_dimension: u32,
    pub feature_names: Vec<String>,
    pub pair_visits: u64,
    pub weighting: &'static str,
    pub rotation_invariant: bool,
    pub curve: Vec<VectorSemivariogramRow>,
}

#[derive(Clone, Debug)]
pub struct EmbeddingCrossCovarianceSpec {
    pub points: Vec<EmbeddingSpatialPoint>,
    pub feature_names: Vec<String>,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub maximum_pair_visits: u64,
    pub maximum_matrix_elements: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceSummary {
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
    pub pair_count: u64,
    pub trace: Option<f64>,
    pub frobenius_norm: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceMatrix {
    pub bin_id: String,
    pub pair_count: u64,
    pub matrix: Option<Vec<Vec<f64>>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceMatrixArtifact {
    pub format: &'static str,
    pub version: u32,
    pub feature_names: Vec<String>,
    pub matrices: Vec<EmbeddingCrossCovarianceMatrix>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingCrossCovarianceResult {
    pub coordinate_unit: &'static str,
    pub object_count: u32,
    pub embedding_dimension: u32,
    pub feature_names: Vec<String>,
    pub pair_visits: u64,
    pub global_mean: Vec<f64>,
    pub symmetrization: &'static str,
    pub summaries: Vec<EmbeddingCrossCovarianceSummary>,
    pub matrix_artifact: EmbeddingCrossCovarianceMatrixArtifact,
}

#[derive(Debug, Error)]
pub enum EmbeddingSpatialError {
    #[error("invalid embedding spatial analysis: {0}")]
    Invalid(String),
    #[error("embedding spatial analysis exceeded its numeric range: {0}")]
    Numeric(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectedEmbeddingPoint {
    pub object_id: String,
    pub biological_unit: String,
    pub split: String,
    pub permutation_stratum: String,
    pub x_um: f64,
    pub y_um: f64,
    pub embedding: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct ProjectedEmbeddingVariogramSpec {
    pub points: Vec<ProjectedEmbeddingPoint>,
    pub feature_names: Vec<String>,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub components: u32,
    pub permutations: u32,
    pub seed: u64,
    pub maximum_pair_visits: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectedEmbeddingResourceLimits {
    pub maximum_objects: u32,
    pub maximum_embedding_dimension: u32,
    pub maximum_components: u32,
    pub maximum_pair_visits: u64,
    pub maximum_work_units: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectedEmbeddingVariogramWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub feature_names: Vec<String>,
    pub points: Vec<ProjectedEmbeddingPoint>,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub components: u32,
    pub permutations: u32,
    pub seed: u64,
    pub resources: ProjectedEmbeddingResourceLimits,
}

impl ProjectedEmbeddingVariogramWorkerRequest {
    pub fn new(
        mut spec: ProjectedEmbeddingVariogramSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !(8..=10_000).contains(&spec.points.len())
            || !(2..=128).contains(&spec.feature_names.len())
            || !(1..=16).contains(&spec.components)
            || spec.components as usize > spec.feature_names.len()
            || !(20..=10_000).contains(&spec.permutations)
            || !(1..=3_600).contains(&spec.timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "projected variogram dimensions, permutations, or timeout are invalid".into(),
            ));
        }
        validate_projected_feature_names(&spec.feature_names)?;
        validate_bins(&mut spec.bins)
            .map_err(|error| BayesError::InvalidSpec(error.to_string()))?;
        spec.points
            .sort_by(|left, right| left.object_id.cmp(&right.object_id));
        validate_projected_points(&spec.points, spec.feature_names.len())?;

        let mut split_counts = HashMap::<&str, u64>::new();
        let mut unit_splits = HashMap::<&str, &str>::new();
        let mut stratum_counts = HashMap::<(&str, &str), u32>::new();
        for point in &spec.points {
            *split_counts.entry(point.split.as_str()).or_default() += 1;
            if let Some(previous) = unit_splits.insert(&point.biological_unit, &point.split) {
                if previous != point.split {
                    return Err(BayesError::InvalidSpec(
                        "each biological unit must belong to exactly one split".into(),
                    ));
                }
            }
            *stratum_counts
                .entry((&point.split, &point.permutation_stratum))
                .or_default() += 1;
        }
        if !split_counts.contains_key("train")
            || !(split_counts.contains_key("validation") || split_counts.contains_key("test"))
            || stratum_counts.values().any(|count| *count < 2)
        {
            return Err(BayesError::InvalidSpec(
                "projected variograms require train and held-out splits with at least two rows per permutation stratum"
                    .into(),
            ));
        }
        let training_count = split_counts["train"];
        if training_count <= u64::from(spec.components) {
            return Err(BayesError::InvalidSpec(
                "training rows must exceed retained component count".into(),
            ));
        }
        let pair_visits = split_counts.values().try_fold(0_u64, |sum, count| {
            count
                .checked_mul(count - 1)
                .and_then(|value| value.checked_div(2))
                .and_then(|value| sum.checked_add(value))
        });
        let pair_visits = pair_visits
            .ok_or_else(|| BayesError::InvalidSpec("split pair count overflowed".into()))?;
        if spec.maximum_pair_visits == 0 || pair_visits > spec.maximum_pair_visits {
            return Err(BayesError::InvalidSpec(format!(
                "{pair_visits} split-specific pair visits exceed maximum_pair_visits {}",
                spec.maximum_pair_visits
            )));
        }
        let work_units = pair_visits
            .checked_mul(u64::from(spec.components))
            .and_then(|value| value.checked_mul(u64::from(spec.permutations) + 1))
            .ok_or_else(|| BayesError::InvalidSpec("projected variogram work overflowed".into()))?;
        const MAXIMUM_WORK_UNITS: u64 = 250_000_000;
        if work_units > MAXIMUM_WORK_UNITS {
            return Err(BayesError::InvalidSpec(
                "projected variogram work exceeds 250000000 component-pair visits".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_projected_embedding_variograms_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            feature_names: spec.feature_names,
            points: spec.points,
            bins: spec.bins,
            components: spec.components,
            permutations: spec.permutations,
            seed: spec.seed,
            resources: ProjectedEmbeddingResourceLimits {
                maximum_objects: 10_000,
                maximum_embedding_dimension: 128,
                maximum_components: 16,
                maximum_pair_visits: spec.maximum_pair_visits,
                maximum_work_units: MAXIMUM_WORK_UNITS,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionArtifact {
    pub fit_split: String,
    pub training_biological_units: Vec<String>,
    pub center: Vec<f64>,
    pub eigenvalues: Vec<f64>,
    pub components: Vec<Vec<f64>>,
    pub method: String,
    pub sign_orientation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedVariogramRow {
    pub component: u32,
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
    pub pair_count: u64,
    pub semivariance: Option<f64>,
    pub permutation_mean: Option<f64>,
    pub permutation_sd: Option<f64>,
    pub max_t_adjusted_p: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedSplitCurve {
    pub split: String,
    pub row_count: u32,
    pub pair_visits: u64,
    pub family_size: u32,
    pub rows: Vec<ProjectedVariogramRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedEmbeddingVariogramWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub projection_artifact: ProjectionArtifact,
    pub multiplicity_control: String,
    pub permutations: u32,
    pub seed: u64,
    pub curves: Vec<ProjectedSplitCurve>,
}

impl ProjectedEmbeddingVariogramWorkerResult {
    pub fn validate(
        &self,
        request: &ProjectedEmbeddingVariogramWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.scipy_projected_embedding_variograms_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.multiplicity_control != "single_step_max_t"
            || self.permutations != request.permutations
            || self.seed != request.seed
        {
            return Err(BayesError::WorkerContract(
                "projected variogram result identity mismatch".into(),
            ));
        }
        self.validate_projection(request)?;
        self.validate_curves(request)?;
        Ok(())
    }

    fn validate_projection(
        &self,
        request: &ProjectedEmbeddingVariogramWorkerRequest,
    ) -> Result<(), BayesError> {
        let artifact = &self.projection_artifact;
        let training = request
            .points
            .iter()
            .filter(|point| point.split == "train")
            .collect::<Vec<_>>();
        let mut units = training
            .iter()
            .map(|point| point.biological_unit.clone())
            .collect::<Vec<_>>();
        units.sort();
        units.dedup();
        let dimension = request.feature_names.len();
        if artifact.fit_split != "train"
            || artifact.training_biological_units != units
            || artifact.method != "training_only_centered_pca_scipy_eigh"
            || artifact.sign_orientation != "largest_absolute_loading_positive"
            || artifact.center.len() != dimension
            || artifact.eigenvalues.len() != request.components as usize
            || artifact.components.len() != request.components as usize
            || artifact
                .components
                .iter()
                .any(|component| component.len() != dimension)
        {
            return Err(BayesError::WorkerContract(
                "projected variogram projection artifact dimensions mismatch".into(),
            ));
        }
        for feature in 0..dimension {
            let expected = training
                .iter()
                .map(|point| point.embedding[feature])
                .sum::<f64>()
                / training.len() as f64;
            if !approximately_equal(artifact.center[feature], expected) {
                return Err(BayesError::WorkerContract(
                    "projection center was not fitted from training rows".into(),
                ));
            }
        }
        for component in 0..artifact.components.len() {
            let vector = &artifact.components[component];
            let norm = vector.iter().map(|value| value * value).sum::<f64>();
            let pivot = vector
                .iter()
                .enumerate()
                .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
                .expect("validated nonempty projection component")
                .1;
            if !artifact.eigenvalues[component].is_finite()
                || artifact.eigenvalues[component] <= 0.0
                || !approximately_equal(norm, 1.0)
                || !pivot.is_finite()
                || *pivot < 0.0
                || (component > 0
                    && artifact.eigenvalues[component] > artifact.eigenvalues[component - 1])
            {
                return Err(BayesError::WorkerContract(
                    "projection components are not ordered positive orthonormal directions".into(),
                ));
            }
            for previous in &artifact.components[..component] {
                let dot = vector
                    .iter()
                    .zip(previous)
                    .map(|(left, right)| left * right)
                    .sum::<f64>();
                if !approximately_equal(dot, 0.0) {
                    return Err(BayesError::WorkerContract(
                        "projection components are not orthogonal".into(),
                    ));
                }
            }
            let mut covariance_product = vec![CompensatedSum::default(); dimension];
            for point in &training {
                let mut score = CompensatedSum::default();
                for (feature, loading) in vector.iter().enumerate() {
                    score
                        .add((point.embedding[feature] - artifact.center[feature]) * loading)
                        .map_err(|error| BayesError::WorkerContract(error.to_string()))?;
                }
                let score = score
                    .total()
                    .map_err(|error| BayesError::WorkerContract(error.to_string()))?;
                for (feature, product) in covariance_product.iter_mut().enumerate() {
                    product
                        .add((point.embedding[feature] - artifact.center[feature]) * score)
                        .map_err(|error| BayesError::WorkerContract(error.to_string()))?;
                }
            }
            for feature in 0..dimension {
                let actual = covariance_product[feature]
                    .total()
                    .map_err(|error| BayesError::WorkerContract(error.to_string()))?
                    / (training.len() - 1) as f64;
                let expected = artifact.eigenvalues[component] * vector[feature];
                if (actual - expected).abs() > 1e-8 * actual.abs().max(expected.abs()).max(1.0) {
                    return Err(BayesError::WorkerContract(
                        "projection component does not satisfy the training covariance eigenproblem"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_curves(
        &self,
        request: &ProjectedEmbeddingVariogramWorkerRequest,
    ) -> Result<(), BayesError> {
        let expected_splits = ["train", "validation", "test"]
            .into_iter()
            .filter(|split| request.points.iter().any(|point| point.split == *split))
            .collect::<Vec<_>>();
        if self.curves.len() != expected_splits.len() {
            return Err(BayesError::WorkerContract(
                "projected variogram split count mismatch".into(),
            ));
        }
        for (curve, expected_split) in self.curves.iter().zip(expected_splits) {
            let points = request
                .points
                .iter()
                .filter(|point| point.split == expected_split)
                .collect::<Vec<_>>();
            let pair_visits = points.len() as u64 * (points.len() as u64 - 1) / 2;
            let pair_counts = projected_pair_counts(&points, &request.bins)?;
            let expected_semivariances =
                projected_semivariogram_values(&points, &request.bins, &self.projection_artifact)?;
            let family_size =
                pair_counts.iter().filter(|count| **count > 0).count() as u32 * request.components;
            if curve.split != expected_split
                || curve.row_count != points.len() as u32
                || curve.pair_visits != pair_visits
                || curve.family_size != family_size
                || curve.rows.len() != request.components as usize * request.bins.len()
            {
                return Err(BayesError::WorkerContract(
                    "projected variogram split dimensions mismatch".into(),
                ));
            }
            for (component, expected_component) in expected_semivariances.iter().enumerate() {
                for (bin_index, bin) in request.bins.iter().enumerate() {
                    let row = &curve.rows[component * request.bins.len() + bin_index];
                    if row.component != component as u32
                        || row.bin_id != bin.bin_id
                        || row.lower_um.to_bits() != bin.lower_um.to_bits()
                        || row.upper_um.to_bits() != bin.upper_um.to_bits()
                        || row.upper_inclusive != bin.upper_inclusive
                        || row.pair_count != pair_counts[bin_index]
                        || !optional_approximately_equal(
                            row.semivariance,
                            expected_component[bin_index],
                        )
                        || !valid_optional_nonnegative(row.semivariance)
                        || !valid_optional_nonnegative(row.permutation_mean)
                        || !valid_optional_positive(row.permutation_sd)
                        || row.max_t_adjusted_p.is_some_and(|value| {
                            !value.is_finite() || !(0.0..=1.0).contains(&value)
                        })
                        || row.max_t_adjusted_p.is_some_and(|value| {
                            let scaled = value * f64::from(request.permutations + 1);
                            (scaled - scaled.round()).abs() > 1e-10
                        })
                        || (row.pair_count == 0
                            && (row.semivariance.is_some()
                                || row.permutation_mean.is_some()
                                || row.permutation_sd.is_some()
                                || row.max_t_adjusted_p.is_some()))
                    {
                        return Err(BayesError::WorkerContract(
                            "projected variogram curve row is invalid".into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ProjectedEmbeddingInputIdentity {
    pub input_path: String,
    pub input_sha256: String,
    pub bins_path: String,
    pub bins_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectedEmbeddingVariogramResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub input: ProjectedEmbeddingInputIdentity,
    pub feature_names: Vec<String>,
    pub projection_artifact: ProjectionArtifact,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub multiplicity_control: String,
    pub permutations: u32,
    pub seed: u64,
    pub curves: Vec<ProjectedSplitCurve>,
    pub request_sha256: String,
}

impl ProjectedEmbeddingVariogramWorkerResult {
    pub fn into_result(
        self,
        request: ProjectedEmbeddingVariogramWorkerRequest,
        input: ProjectedEmbeddingInputIdentity,
    ) -> ProjectedEmbeddingVariogramResult {
        ProjectedEmbeddingVariogramResult {
            format: "marklab.projected_embedding_variograms",
            version: 1,
            backend: self.backend,
            input,
            feature_names: request.feature_names,
            projection_artifact: self.projection_artifact,
            bins: request.bins,
            multiplicity_control: self.multiplicity_control,
            permutations: self.permutations,
            seed: self.seed,
            curves: self.curves,
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct CompensatedSum {
    sum: f64,
    correction: f64,
}

impl CompensatedSum {
    fn add(&mut self, value: f64) -> Result<(), EmbeddingSpatialError> {
        if !value.is_finite() {
            return Err(EmbeddingSpatialError::Numeric(
                "a weighted pair contribution is non-finite".into(),
            ));
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(EmbeddingSpatialError::Numeric(
                "a compensated pair sum overflowed".into(),
            ));
        }
        if self.sum.abs() >= value.abs() {
            self.correction += (self.sum - next) + value;
        } else {
            self.correction += (value - next) + self.sum;
        }
        if !self.correction.is_finite() {
            return Err(EmbeddingSpatialError::Numeric(
                "a compensated pair correction overflowed".into(),
            ));
        }
        self.sum = next;
        Ok(())
    }

    fn total(self) -> Result<f64, EmbeddingSpatialError> {
        let total = self.sum + self.correction;
        if total.is_finite() {
            Ok(total)
        } else {
            Err(EmbeddingSpatialError::Numeric(
                "a compensated pair total overflowed".into(),
            ))
        }
    }
}

pub fn vector_semivariogram(
    mut spec: VectorSemivariogramSpec,
) -> Result<VectorSemivariogramResult, EmbeddingSpatialError> {
    let dimension = validate_points_and_features(&mut spec.points, &spec.feature_names)?;
    validate_bins(&mut spec.bins)?;

    let point_count = spec.points.len() as u64;
    let pair_visits = point_count
        .checked_mul(point_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| EmbeddingSpatialError::Invalid("pair count overflowed".into()))?;
    if spec.maximum_pair_visits == 0 || pair_visits > spec.maximum_pair_visits {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "{pair_visits} unordered pair visits exceed maximum_pair_visits {}",
            spec.maximum_pair_visits
        )));
    }

    let weighted = spec.weights.is_some();
    let mut weights = build_weight_map(spec.weights, &spec.points)?;
    let mut pair_counts = vec![0_u64; spec.bins.len()];
    let mut weight_sums = vec![CompensatedSum::default(); spec.bins.len()];
    let mut weighted_semivariances = vec![CompensatedSum::default(); spec.bins.len()];

    for left_index in 0..spec.points.len() {
        let left = &spec.points[left_index];
        for right in &spec.points[(left_index + 1)..] {
            let distance = (left.x_um - right.x_um).hypot(left.y_um - right.y_um);
            if !distance.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "distance between {} and {} is non-finite",
                    left.object_id, right.object_id
                )));
            }
            let Some(bin_index) = find_bin(distance, &spec.bins) else {
                continue;
            };
            let pair_key = (left.object_id.clone(), right.object_id.clone());
            let weight = if weighted {
                weights.remove(&pair_key).ok_or_else(|| {
                    EmbeddingSpatialError::Invalid(format!(
                        "eligible pair {}--{} has no declared weight",
                        left.object_id, right.object_id
                    ))
                })?
            } else {
                1.0
            };
            let squared_distance = stable_squared_distance(&left.embedding, &right.embedding)?;
            let contribution = 0.5 * weight * squared_distance;
            if !contribution.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "semivariance contribution for {}--{} overflowed",
                    left.object_id, right.object_id
                )));
            }
            pair_counts[bin_index] += 1;
            weight_sums[bin_index].add(weight)?;
            weighted_semivariances[bin_index].add(contribution)?;
        }
    }
    if let Some(((left, right), _)) = weights.into_iter().next() {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "declared weight for ineligible pair {left}--{right}"
        )));
    }

    let mut curve = Vec::with_capacity(spec.bins.len());
    for (index, bin) in spec.bins.into_iter().enumerate() {
        let weight_sum = weight_sums[index].total()?;
        let semivariance = if pair_counts[index] == 0 {
            None
        } else {
            let value = weighted_semivariances[index].total()? / weight_sum;
            if !value.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "semivariance for bin {} is non-finite",
                    bin.bin_id
                )));
            }
            Some(value)
        };
        curve.push(VectorSemivariogramRow {
            bin_id: bin.bin_id,
            lower_um: bin.lower_um,
            upper_um: bin.upper_um,
            upper_inclusive: bin.upper_inclusive,
            pair_count: pair_counts[index],
            weight_sum,
            semivariance,
            inference_eligible: pair_counts[index] >= 2,
        });
    }

    Ok(VectorSemivariogramResult {
        coordinate_unit: "micrometer",
        object_count: point_count as u32,
        embedding_dimension: dimension as u32,
        feature_names: spec.feature_names,
        pair_visits,
        weighting: if weighted {
            "declared_pair_weights"
        } else {
            "unit"
        },
        rotation_invariant: true,
        curve,
    })
}

pub fn embedding_cross_covariance_by_distance(
    mut spec: EmbeddingCrossCovarianceSpec,
) -> Result<EmbeddingCrossCovarianceResult, EmbeddingSpatialError> {
    let dimension = validate_points_and_features(&mut spec.points, &spec.feature_names)?;
    validate_bins(&mut spec.bins)?;
    let point_count = spec.points.len() as u64;
    let pair_visits = point_count
        .checked_mul(point_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| EmbeddingSpatialError::Invalid("pair count overflowed".into()))?;
    if spec.maximum_pair_visits == 0 || pair_visits > spec.maximum_pair_visits {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "{pair_visits} unordered pair visits exceed maximum_pair_visits {}",
            spec.maximum_pair_visits
        )));
    }
    let elements_per_matrix = (dimension as u64)
        .checked_mul(dimension as u64)
        .ok_or_else(|| EmbeddingSpatialError::Invalid("matrix size overflowed".into()))?;
    let stored_elements = elements_per_matrix
        .checked_mul(spec.bins.len() as u64)
        .ok_or_else(|| EmbeddingSpatialError::Invalid("matrix artifact size overflowed".into()))?;
    let matrix_operations = elements_per_matrix
        .checked_mul(pair_visits)
        .ok_or_else(|| EmbeddingSpatialError::Invalid("matrix work overflowed".into()))?;
    if spec.maximum_matrix_elements == 0
        || stored_elements > 1_000_000
        || matrix_operations > spec.maximum_matrix_elements
        || spec.maximum_matrix_elements > 250_000_000
    {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "{matrix_operations} pair-matrix element operations exceed the declared or fixed resource bound"
        )));
    }

    let mut mean_sums = vec![CompensatedSum::default(); dimension];
    for point in &spec.points {
        for (sum, value) in mean_sums.iter_mut().zip(&point.embedding) {
            sum.add(*value)?;
        }
    }
    let global_mean = mean_sums
        .into_iter()
        .map(|sum| sum.total().map(|total| total / point_count as f64))
        .collect::<Result<Vec<_>, _>>()?;
    let centered = spec
        .points
        .iter()
        .map(|point| {
            point
                .embedding
                .iter()
                .zip(&global_mean)
                .map(|(value, mean)| value - mean)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut pair_counts = vec![0_u64; spec.bins.len()];
    let mut accumulators = vec![CompensatedSum::default(); spec.bins.len() * dimension * dimension];
    for left in 0..spec.points.len() {
        for right in (left + 1)..spec.points.len() {
            let distance = (spec.points[left].x_um - spec.points[right].x_um)
                .hypot(spec.points[left].y_um - spec.points[right].y_um);
            if !distance.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "distance between {} and {} is non-finite",
                    spec.points[left].object_id, spec.points[right].object_id
                )));
            }
            let Some(bin) = find_bin(distance, &spec.bins) else {
                continue;
            };
            pair_counts[bin] += 1;
            let offset = bin * dimension * dimension;
            for row in 0..dimension {
                for column in 0..dimension {
                    accumulators[offset + row * dimension + column]
                        .add(centered[left][row] * centered[right][column])?;
                }
            }
        }
    }

    let mut summaries = Vec::with_capacity(spec.bins.len());
    let mut matrices = Vec::with_capacity(spec.bins.len());
    for (bin_index, bin) in spec.bins.iter().enumerate() {
        let matrix = if pair_counts[bin_index] == 0 {
            None
        } else {
            let mut values = vec![vec![0.0; dimension]; dimension];
            let offset = bin_index * dimension * dimension;
            for row in 0..dimension {
                for column in 0..dimension {
                    let forward = accumulators[offset + row * dimension + column].total()?
                        / pair_counts[bin_index] as f64;
                    let reverse = accumulators[offset + column * dimension + row].total()?
                        / pair_counts[bin_index] as f64;
                    let value = 0.5 * (forward + reverse);
                    if !value.is_finite() {
                        return Err(EmbeddingSpatialError::Numeric(format!(
                            "cross-covariance matrix for bin {} is non-finite",
                            bin.bin_id
                        )));
                    }
                    values[row][column] = value;
                }
            }
            Some(values)
        };
        let (trace, frobenius_norm) = if let Some(values) = &matrix {
            let mut trace = CompensatedSum::default();
            let mut squared = CompensatedSum::default();
            for (row, values_row) in values.iter().enumerate() {
                trace.add(values_row[row])?;
                for value in values_row {
                    squared.add(value * value)?;
                }
            }
            let trace = trace.total()?;
            let frobenius = squared.total()?.sqrt();
            if !frobenius.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "Frobenius norm for bin {} is non-finite",
                    bin.bin_id
                )));
            }
            (Some(trace), Some(frobenius))
        } else {
            (None, None)
        };
        summaries.push(EmbeddingCrossCovarianceSummary {
            bin_id: bin.bin_id.clone(),
            lower_um: bin.lower_um,
            upper_um: bin.upper_um,
            upper_inclusive: bin.upper_inclusive,
            pair_count: pair_counts[bin_index],
            trace,
            frobenius_norm,
        });
        matrices.push(EmbeddingCrossCovarianceMatrix {
            bin_id: bin.bin_id.clone(),
            pair_count: pair_counts[bin_index],
            matrix,
        });
    }

    Ok(EmbeddingCrossCovarianceResult {
        coordinate_unit: "micrometer",
        object_count: point_count as u32,
        embedding_dimension: dimension as u32,
        feature_names: spec.feature_names.clone(),
        pair_visits,
        global_mean,
        symmetrization: "undirected_average_c_plus_c_transpose_over_two",
        summaries,
        matrix_artifact: EmbeddingCrossCovarianceMatrixArtifact {
            format: "marklab.embedding_cross_covariance_matrix_artifact",
            version: 1,
            feature_names: spec.feature_names,
            matrices,
        },
    })
}

fn validate_points_and_features(
    points: &mut [EmbeddingSpatialPoint],
    feature_names: &[String],
) -> Result<usize, EmbeddingSpatialError> {
    if !(2..=10_000).contains(&points.len()) || !(2..=4_096).contains(&feature_names.len()) {
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

fn find_bin(distance: f64, bins: &[EmbeddingDistanceBin]) -> Option<usize> {
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
    sum.total()
}

fn validate_projected_feature_names(feature_names: &[String]) -> Result<(), BayesError> {
    let mut names = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !names.insert(name.as_str())
    }) {
        return Err(BayesError::InvalidSpec(
            "projected variogram feature names must be unique exact embedding_* names".into(),
        ));
    }
    Ok(())
}

fn validate_projected_points(
    points: &[ProjectedEmbeddingPoint],
    dimension: usize,
) -> Result<(), BayesError> {
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
            return Err(BayesError::InvalidSpec(
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
) -> Result<Vec<u64>, BayesError> {
    let mut counts = vec![0_u64; bins.len()];
    for left in 0..points.len() {
        for right in &points[(left + 1)..] {
            let distance = (points[left].x_um - right.x_um).hypot(points[left].y_um - right.y_um);
            if !distance.is_finite() {
                return Err(BayesError::WorkerContract(
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
) -> Result<Vec<Vec<Option<f64>>>, BayesError> {
    let component_count = artifact.components.len();
    let mut projected = vec![vec![0.0; component_count]; points.len()];
    for (point_index, point) in points.iter().enumerate() {
        for (component_index, component) in artifact.components.iter().enumerate() {
            let mut sum = CompensatedSum::default();
            for (feature, loading) in component.iter().enumerate() {
                sum.add((point.embedding[feature] - artifact.center[feature]) * loading)
                    .map_err(|error| BayesError::WorkerContract(error.to_string()))?;
            }
            projected[point_index][component_index] = sum
                .total()
                .map_err(|error| BayesError::WorkerContract(error.to_string()))?;
        }
    }
    let mut counts = vec![0_u64; bins.len()];
    let mut sums = vec![vec![CompensatedSum::default(); bins.len()]; component_count];
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
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
                    .map_err(|error| BayesError::WorkerContract(error.to_string()))?;
            }
        }
    }
    let mut result = vec![vec![None; bins.len()]; component_count];
    for component in 0..component_count {
        for bin in 0..bins.len() {
            if counts[bin] > 0 {
                result[component][bin] = Some(
                    sums[component][bin]
                        .total()
                        .map_err(|error| BayesError::WorkerContract(error.to_string()))?
                        / counts[bin] as f64,
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

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * left.abs().max(right.abs()).max(1.0)
}

fn optional_approximately_equal(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => approximately_equal(left, right),
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
}
