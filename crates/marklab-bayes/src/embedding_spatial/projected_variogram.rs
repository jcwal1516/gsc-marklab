use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, WorkerBackend};

use super::{
    approximately_equal, optional_approximately_equal, projected_pair_counts,
    projected_semivariogram_values, projected_sum_error, valid_optional_nonnegative,
    valid_optional_positive, validate_bins, validate_projected_feature_names,
    validate_projected_points, CompensatedSum, EmbeddingDistanceBin, SCIPY_VERSION,
};

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
