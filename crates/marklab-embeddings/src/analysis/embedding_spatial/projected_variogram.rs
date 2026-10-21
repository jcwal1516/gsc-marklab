use marklab_core::backend::is_worker_timeout;
use marklab_numerics::relative_eq;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use marklab_core::backend::{BackendContract, WorkerBackend, WorkerError};

use super::{
    optional_approximately_equal, projected_pair_counts, projected_semivariogram_values,
    projected_sum_error, valid_optional_nonnegative, valid_optional_positive, validate_bins,
    validate_projected_feature_names, validate_projected_points, CompensatedSum,
    EmbeddingDistanceBin, SCIPY_VERSION,
};
use marklab_core::backend::versions::PYTHON_VERSION;

/// One spatial embedding row with biological-unit, split, and permutation identities.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectedEmbeddingPoint {
    /// Stable object identity.
    pub object_id: String,
    /// Biological unit used to isolate spatial pairs and prevent training/held-out leakage.
    pub biological_unit: String,
    /// Declared training, validation, or held-out data split.
    pub split: String,
    /// Group within which embedding rows may be permuted.
    pub permutation_stratum: String,
    /// Object x coordinate, in micrometres.
    pub x_um: f64,
    /// Object y coordinate, in micrometres.
    pub y_um: f64,
    /// Finite feature vector in declared feature order.
    pub embedding: Vec<f64>,
}

/// Training projection and component-wise spatial-permutation specification.
#[derive(Clone, Debug)]
pub struct ProjectedEmbeddingVariogramSpec {
    /// Input spatial points and their embedding vectors.
    pub points: Vec<ProjectedEmbeddingPoint>,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Ordered, nonoverlapping distance bins.
    pub bins: Vec<EmbeddingDistanceBin>,
    /// Number of retained principal components.
    pub components: u32,
    /// Number of declared random permutations.
    pub permutations: u32,
    /// Deterministic random seed for the declared analysis.
    pub seed: u64,
    /// Maximum permitted spatial pair evaluations.
    pub maximum_pair_visits: u64,
    /// Maximum wall-clock execution time of the worker, in seconds.
    pub timeout_seconds: u64,
}

/// Bounds on projection dimension, pair work, and worker execution.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectedEmbeddingResourceLimits {
    /// Maximum permitted number of input objects.
    pub maximum_objects: u32,
    /// Maximum permitted embedding-vector dimension.
    pub maximum_embedding_dimension: u32,
    /// Maximum number of retained projection components.
    pub maximum_components: u32,
    /// Maximum permitted spatial pair evaluations.
    pub maximum_pair_visits: u64,
    /// Maximum permitted declared computation units.
    pub maximum_work_units: u64,
    /// Maximum permitted worker response size, in bytes.
    pub maximum_output_bytes: u64,
    /// Maximum wall-clock execution time of the worker, in seconds.
    pub timeout_seconds: u64,
}

/// Validated request for projected-embedding variograms.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectedEmbeddingVariogramWorkerRequest {
    /// Versioned document-format identifier.
    pub format: &'static str,
    /// Version of the recorded document format.
    pub version: u32,
    /// Pinned worker implementation and asset identity.
    pub backend: BackendContract,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Input spatial points and their embedding vectors.
    pub points: Vec<ProjectedEmbeddingPoint>,
    /// Ordered, nonoverlapping distance bins.
    pub bins: Vec<EmbeddingDistanceBin>,
    /// Number of retained principal components.
    pub components: u32,
    /// Number of declared random permutations.
    pub permutations: u32,
    /// Deterministic random seed for the declared analysis.
    pub seed: u64,
    /// Declared data, computation, and execution limits.
    pub resources: ProjectedEmbeddingResourceLimits,
}

impl ProjectedEmbeddingVariogramWorkerRequest {
    /// Validate the specification and bind the request to the supplied worker assets.
    pub fn new(
        mut spec: ProjectedEmbeddingVariogramSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, WorkerError> {
        if !(8..=10_000).contains(&spec.points.len())
            || !(2..=128).contains(&spec.feature_names.len())
            || !(1..=16).contains(&spec.components)
            || spec.components as usize > spec.feature_names.len()
            || !(20..=10_000).contains(&spec.permutations)
            || !is_worker_timeout(spec.timeout_seconds)
        {
            return Err(WorkerError::InvalidSpec(
                "projected variogram dimensions, permutations, or timeout are invalid".into(),
            ));
        }
        validate_projected_feature_names(&spec.feature_names)?;
        validate_bins(&mut spec.bins)
            .map_err(|error| WorkerError::InvalidSpec(error.to_string()))?;
        spec.points
            .sort_by(|left, right| left.object_id.cmp(&right.object_id));
        validate_projected_points(&spec.points, spec.feature_names.len())?;

        let mut split_counts = HashMap::<&str, u64>::new();
        let mut unit_counts = HashMap::<&str, u64>::new();
        let mut unit_splits = HashMap::<&str, &str>::new();
        let mut stratum_counts = HashMap::<(&str, &str), u32>::new();
        for point in &spec.points {
            *split_counts.entry(point.split.as_str()).or_default() += 1;
            *unit_counts
                .entry(point.biological_unit.as_str())
                .or_default() += 1;
            if let Some(previous) = unit_splits.insert(&point.biological_unit, &point.split) {
                if previous != point.split {
                    return Err(WorkerError::InvalidSpec(
                        "each biological unit must belong to exactly one split".into(),
                    ));
                }
            }
            *stratum_counts
                .entry((&point.biological_unit, &point.permutation_stratum))
                .or_default() += 1;
        }
        if !split_counts.contains_key("train")
            || !(split_counts.contains_key("validation") || split_counts.contains_key("test"))
            || stratum_counts.values().any(|count| *count < 2)
        {
            return Err(WorkerError::InvalidSpec(
                "projected variograms require train and held-out splits with at least two rows per biological-unit/permutation-stratum group"
                    .into(),
            ));
        }
        let training_count = split_counts["train"];
        if training_count <= u64::from(spec.components) {
            return Err(WorkerError::InvalidSpec(
                "training rows must exceed retained component count".into(),
            ));
        }
        let pair_visits = unit_counts.values().try_fold(0_u64, |sum, count| {
            count
                .checked_mul(count - 1)
                .and_then(|value| value.checked_div(2))
                .and_then(|value| sum.checked_add(value))
        });
        let pair_visits = pair_visits
            .ok_or_else(|| WorkerError::InvalidSpec("split pair count overflowed".into()))?;
        if spec.maximum_pair_visits == 0 || pair_visits > spec.maximum_pair_visits {
            return Err(WorkerError::InvalidSpec(format!(
                "{pair_visits} within-biological-unit pair visits exceed maximum_pair_visits {}",
                spec.maximum_pair_visits
            )));
        }
        let work_units = pair_visits
            .checked_mul(u64::from(spec.components))
            .and_then(|value| value.checked_mul(u64::from(spec.permutations) + 1))
            .ok_or_else(|| {
                WorkerError::InvalidSpec("projected variogram work overflowed".into())
            })?;
        const MAXIMUM_WORK_UNITS: u64 = 250_000_000;
        if work_units > MAXIMUM_WORK_UNITS {
            return Err(WorkerError::InvalidSpec(
                "projected variogram work exceeds 250000000 component-pair visits".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_projected_embedding_variograms_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: PYTHON_VERSION,
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

/// Training-only principal-component projection and orientation convention.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionArtifact {
    /// Data split used to fit preprocessing or projection.
    pub fit_split: String,
    /// Biological units admitted to the training fit.
    pub training_biological_units: Vec<String>,
    /// Training-split feature means used for centering.
    pub center: Vec<f64>,
    /// Training covariance eigenvalues for the retained components.
    pub eigenvalues: Vec<f64>,
    /// Loading vectors in retained-component order.
    pub components: Vec<Vec<f64>>,
    /// Recorded projection-fitting algorithm.
    pub method: String,
    /// Recorded convention for the signs of projection components.
    pub sign_orientation: String,
}

/// One projected component semivariogram and adjusted permutation result.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedVariogramRow {
    /// Index of the retained projection component.
    pub component: u32,
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
    /// Half the mean squared vector difference, absent when no pairs contribute.
    pub semivariance: Option<f64>,
    /// Mean statistic over the declared label permutations.
    pub permutation_mean: Option<f64>,
    /// Standard deviation over the declared label permutations.
    pub permutation_sd: Option<f64>,
    /// Max-T adjusted permutation p-value, absent when inference is unavailable.
    pub max_t_adjusted_p: Option<f64>,
}

/// Component and distance-bin inference rows for one data split.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedSplitCurve {
    /// Declared training, validation, or held-out data split.
    pub split: String,
    /// Number of rows in this data split.
    pub row_count: u32,
    /// Number of spatial pairs examined.
    pub pair_visits: u64,
    /// Number of component/bin hypotheses in the adjusted inference family.
    pub family_size: u32,
    /// Ordered input or result rows.
    pub rows: Vec<ProjectedVariogramRow>,
}

/// Projection and variogram payload returned by the pinned worker.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedEmbeddingVariogramWorkerResult {
    format: String,
    version: u32,
    /// Pinned worker implementation and asset identity.
    pub backend: WorkerBackend,
    /// SHA-256 of the exact request bytes accepted by the worker.
    pub request_sha256: String,
    /// Training-fitted principal-component projection.
    pub projection_artifact: ProjectionArtifact,
    /// Recorded method for simultaneous hypothesis control.
    pub multiplicity_control: String,
    /// Number of declared random permutations.
    pub permutations: u32,
    /// Deterministic random seed for the declared analysis.
    pub seed: u64,
    /// Distance curves grouped by data split.
    pub curves: Vec<ProjectedSplitCurve>,
}

impl ProjectedEmbeddingVariogramWorkerResult {
    /// Check the worker identity, dimensions, and numerical summaries against the exact request.
    pub fn validate(
        &self,
        request: &ProjectedEmbeddingVariogramWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), WorkerError> {
        if self.format != "marklab.scipy_projected_embedding_variograms_worker_result"
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.request_sha256 != request_sha256
            || self.multiplicity_control != "single_step_max_t"
            || self.permutations != request.permutations
            || self.seed != request.seed
        {
            return Err(WorkerError::WorkerContract(
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
    ) -> Result<(), WorkerError> {
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
            return Err(WorkerError::WorkerContract(
                "projected variogram projection artifact dimensions mismatch".into(),
            ));
        }
        for feature in 0..dimension {
            let expected = training
                .iter()
                .map(|point| point.embedding[feature])
                .sum::<f64>()
                / training.len() as f64;
            if !relative_eq(
                artifact.center[feature],
                expected,
                super::RELATIVE_TOLERANCE,
            ) {
                return Err(WorkerError::WorkerContract(
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
                .max_by(|left, right| {
                    left.1
                        .abs()
                        .total_cmp(&right.1.abs())
                        .then_with(|| right.0.cmp(&left.0))
                })
                .expect("validated nonempty projection component")
                .1;
            if !artifact.eigenvalues[component].is_finite()
                || artifact.eigenvalues[component] <= 0.0
                || !relative_eq(norm, 1.0, super::RELATIVE_TOLERANCE)
                || !pivot.is_finite()
                || *pivot < 0.0
                || (component > 0
                    && artifact.eigenvalues[component] > artifact.eigenvalues[component - 1])
            {
                return Err(WorkerError::WorkerContract(
                    "projection components are not ordered positive orthonormal directions".into(),
                ));
            }
            for previous in &artifact.components[..component] {
                let dot = vector
                    .iter()
                    .zip(previous)
                    .map(|(left, right)| left * right)
                    .sum::<f64>();
                if !relative_eq(dot, 0.0, super::RELATIVE_TOLERANCE) {
                    return Err(WorkerError::WorkerContract(
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
                        .map_err(projected_sum_error)?;
                }
                let score = score.total().map_err(projected_sum_error)?;
                for (feature, product) in covariance_product.iter_mut().enumerate() {
                    product
                        .add((point.embedding[feature] - artifact.center[feature]) * score)
                        .map_err(projected_sum_error)?;
                }
            }
            for feature in 0..dimension {
                let actual = covariance_product[feature]
                    .total()
                    .map_err(projected_sum_error)?
                    / (training.len() - 1) as f64;
                let expected = artifact.eigenvalues[component] * vector[feature];
                if (actual - expected).abs() > 1e-8 * actual.abs().max(expected.abs()).max(1.0) {
                    return Err(WorkerError::WorkerContract(
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
    ) -> Result<(), WorkerError> {
        let expected_splits = ["train", "validation", "test"]
            .into_iter()
            .filter(|split| request.points.iter().any(|point| point.split == *split))
            .collect::<Vec<_>>();
        if self.curves.len() != expected_splits.len() {
            return Err(WorkerError::WorkerContract(
                "projected variogram split count mismatch".into(),
            ));
        }
        for (curve, expected_split) in self.curves.iter().zip(expected_splits) {
            let points = request
                .points
                .iter()
                .filter(|point| point.split == expected_split)
                .collect::<Vec<_>>();
            let mut unit_counts = HashMap::<&str, u64>::new();
            for point in &points {
                *unit_counts.entry(&point.biological_unit).or_default() += 1;
            }
            let pair_visits = unit_counts
                .values()
                .map(|count| count * (count - 1) / 2)
                .sum::<u64>();
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
                return Err(WorkerError::WorkerContract(
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
                        return Err(WorkerError::WorkerContract(
                            "projected variogram curve row is invalid".into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// File identities bound into a projected-embedding result.
#[derive(Debug, Serialize)]
pub struct ProjectedEmbeddingInputIdentity {
    /// Reported source path for the analysis input.
    pub input_path: String,
    /// SHA-256 of the exact input bytes.
    pub input_sha256: String,
    /// Reported source path for the bin specification.
    pub bins_path: String,
    /// SHA-256 of the exact bin-specification bytes.
    pub bins_sha256: String,
}

/// Projection artifact and split-specific component semivariograms.
#[derive(Debug, Serialize)]
pub struct ProjectedEmbeddingVariogramResult {
    /// Versioned document-format identifier.
    pub format: &'static str,
    /// Version of the recorded document format.
    pub version: u32,
    /// Pinned worker implementation and asset identity.
    pub backend: WorkerBackend,
    /// Bound identity of the supplied analysis input.
    pub input: ProjectedEmbeddingInputIdentity,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Training-fitted principal-component projection.
    pub projection_artifact: ProjectionArtifact,
    /// Ordered, nonoverlapping distance bins.
    pub bins: Vec<EmbeddingDistanceBin>,
    /// Recorded method for simultaneous hypothesis control.
    pub multiplicity_control: String,
    /// Number of declared random permutations.
    pub permutations: u32,
    /// Deterministic random seed for the declared analysis.
    pub seed: u64,
    /// Distance curves grouped by data split.
    pub curves: Vec<ProjectedSplitCurve>,
    /// SHA-256 of the exact request bytes accepted by the worker.
    pub request_sha256: String,
}

impl ProjectedEmbeddingVariogramWorkerResult {
    /// Build the result document from an already validated worker response and its input identity.
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
