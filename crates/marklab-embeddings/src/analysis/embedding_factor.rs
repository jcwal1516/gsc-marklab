use marklab_core::backend::ModelMaturity;
use marklab_data::{JointLocationGridRow, RectangularWindow};
use serde::Serialize;
use std::collections::HashSet;
use thiserror::Error;
/// One point with location predictors and a multivariate embedding.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingFactorPoint {
    /// Stable spatial point identity.
    pub point_id: String,
    /// Object x coordinate, in micrometres.
    pub x_um: f64,
    /// Object y coordinate, in micrometres.
    pub y_um: f64,
    /// Location-model covariate value.
    pub location_covariate: f64,
    /// Location-model log-intensity offset.
    pub location_offset: f64,
    /// Finite feature vector in declared feature order.
    pub embedding: Vec<f64>,
}
/// Validated construction descriptor for a joint location/embedding latent-factor model.
///
/// This constructor validates and records points, feature dimensions, grid geometry, fixed
/// spatial-kernel parameters, loading-prior scale, and noise-prior scale. It emits an
/// `experimental_model_construction` document; no current Python worker consumes this type or
/// evaluates its declared Gaussian embedding likelihood or priors. The factor count and
/// positive scales therefore specify a proposed model, not an estimated or calibrated
/// posterior. Separate spatial-factor and replicated-location/embedding fitting workers have
/// different contracts.
///
#[derive(Clone, Debug)]
pub struct EmbeddingFactorSpec {
    /// Input spatial points and their embedding vectors.
    pub points: Vec<EmbeddingFactorPoint>,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Location-model covariates and offsets on the declared grid.
    pub grid: Vec<JointLocationGridRow>,
    /// Declared rectangular observation window.
    pub window: RectangularWindow,
    /// Number of grid cells along x.
    pub grid_x: u32,
    /// Number of grid cells along y.
    pub grid_y: u32,
    /// Number of declared latent factors.
    pub factors: u32,
    /// Fixed spatial-field amplitude.
    pub field_amplitude: f64,
    /// Fixed spatial-kernel length scale, in micrometres.
    pub field_length_scale_um: f64,
    /// Positive diagonal regularization of the spatial covariance.
    pub jitter: f64,
    /// Declared prior standard deviation for factor loadings.
    pub loading_prior_sd: f64,
    /// Declared prior scale for embedding observation noise.
    pub noise_prior_sd: f64,
}
/// Invalid joint location/embedding model construction.
#[derive(Debug, Error)]
pub enum EmbeddingFactorError {
    /// The supplied specification or data is invalid.
    #[error("invalid joint location-embedding factor model: {0}")]
    Invalid(String),
}
/// Declared likelihood, kernel, and identifiability choices for a factor model.
#[derive(Debug, Serialize)]
pub struct EmbeddingFactorModelIr {
    /// Declared scientific model family.
    pub family: &'static str,
    /// Physical coordinate unit recorded by the result.
    pub coordinate_unit: &'static str,
    /// Declared embedding observation likelihood.
    pub embedding_likelihood: &'static str,
    /// Declared spatial construction of the latent factors.
    pub factor_fields: &'static str,
    /// Declared spatial kernel or fitted embedding-kernel artifact.
    pub kernel: &'static str,
    /// Fixed spatial-field amplitude.
    pub field_amplitude: f64,
    /// Fixed spatial-kernel length scale, in micrometres.
    pub field_length_scale_um: f64,
    /// Positive diagonal regularization of the spatial covariance.
    pub jitter: f64,
    /// Declared location-model component of the joint construction.
    pub location_component: &'static str,
    /// Declared convention for resolving factor rotational ambiguity.
    pub rotational_identifiability: &'static str,
    /// Declared shrinkage structure for factor loadings.
    pub shrinkage: &'static str,
    /// Declared prior standard deviation for factor loadings.
    pub loading_prior_sd: f64,
    /// Declared prior scale for embedding observation noise.
    pub noise_prior_sd: f64,
    /// Inference approach recommended by the construction descriptor.
    pub inference_recommendation: &'static str,
    /// Declared reference methods for assessing the model construction.
    pub validation_comparators: &'static str,
    /// Declared implementation maturity of the construction.
    pub maturity: ModelMaturity,
}
/// Validated construction document for a joint location/embedding factor model.
#[derive(Debug, Serialize)]
pub struct EmbeddingFactorModel {
    /// Validated model-construction descriptor.
    pub model: EmbeddingFactorModelIr,
    /// Declared rectangular observation window.
    pub window: RectangularWindow,
    /// Number of grid cells along x.
    pub grid_x: u32,
    /// Number of grid cells along y.
    pub grid_y: u32,
    /// Number of validated spatial points.
    pub point_count: u32,
    /// Number of features in each embedding vector.
    pub embedding_dimension: u32,
    /// Number of declared latent factors.
    pub factor_count: u32,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Input spatial points and their embedding vectors.
    pub points: Vec<EmbeddingFactorPoint>,
    /// Location-model covariates and offsets on the declared grid.
    pub grid: Vec<JointLocationGridRow>,
}
/// Validate and describe the joint factor-model construction without fitting a posterior.
pub fn joint_location_embedding_latent_factor_model(
    mut s: EmbeddingFactorSpec,
) -> Result<EmbeddingFactorModel, EmbeddingFactorError> {
    let n = s.points.len();
    let d = s.feature_names.len();
    if !(8..=2000).contains(&n)
        || !(2..=128).contains(&d)
        || !(1..=16).contains(&s.factors)
        || s.factors as usize >= d
        || s.factors as usize >= n
        || s.grid_x == 0
        || s.grid_y == 0
        || s.grid.len() != s.grid_x as usize * s.grid_y as usize
        || [
            s.window.xmin_um,
            s.window.ymin_um,
            s.window.xmax_um,
            s.window.ymax_um,
            s.field_amplitude,
            s.field_length_scale_um,
            s.jitter,
            s.loading_prior_sd,
            s.noise_prior_sd,
        ]
        .into_iter()
        .any(|v| !v.is_finite())
        || s.window.xmin_um >= s.window.xmax_um
        || s.window.ymin_um >= s.window.ymax_um
        || [
            s.field_amplitude,
            s.field_length_scale_um,
            s.jitter,
            s.loading_prior_sd,
            s.noise_prior_sd,
        ]
        .into_iter()
        .any(|v| v <= 0.0)
    {
        return Err(EmbeddingFactorError::Invalid(
            "dimensions, window, factors, or scales are invalid".into(),
        ));
    }
    let mut features = HashSet::new();
    if s.feature_names
        .iter()
        .any(|f| f.is_empty() || f.trim() != f || !features.insert(f.as_str()))
    {
        return Err(EmbeddingFactorError::Invalid(
            "embedding feature names must be exact and unique".into(),
        ));
    }
    s.points.sort_by(|a, b| a.point_id.cmp(&b.point_id));
    let mut ids = HashSet::new();
    for p in &s.points {
        if p.point_id.is_empty()
            || !ids.insert(p.point_id.as_str())
            || p.embedding.len() != d
            || [p.x_um, p.y_um, p.location_covariate, p.location_offset]
                .into_iter()
                .any(|v| !v.is_finite())
            || p.embedding.iter().any(|v| !v.is_finite())
            || !(s.window.xmin_um..s.window.xmax_um).contains(&p.x_um)
            || !(s.window.ymin_um..s.window.ymax_um).contains(&p.y_um)
        {
            return Err(EmbeddingFactorError::Invalid(
                "embedding points are invalid or outside window".into(),
            ));
        }
    }
    for j in 0..d {
        let min = s
            .points
            .iter()
            .map(|p| p.embedding[j])
            .fold(f64::INFINITY, f64::min);
        let max = s
            .points
            .iter()
            .map(|p| p.embedding[j])
            .fold(f64::NEG_INFINITY, f64::max);
        if max - min <= f64::EPSILON.sqrt() * min.abs().max(max.abs()).max(1.0) {
            return Err(EmbeddingFactorError::Invalid(
                "every embedding dimension must vary materially".into(),
            ));
        }
    }
    let lmin = s
        .points
        .iter()
        .map(|p| p.location_covariate)
        .fold(f64::INFINITY, f64::min);
    let lmax = s
        .points
        .iter()
        .map(|p| p.location_covariate)
        .fold(f64::NEG_INFINITY, f64::max);
    if lmax - lmin <= f64::EPSILON.sqrt() * lmin.abs().max(lmax.abs()).max(1.0) {
        return Err(EmbeddingFactorError::Invalid(
            "location covariate must vary materially".into(),
        ));
    }
    s.grid.sort_by_key(|r| (r.iy, r.ix));
    for (i, r) in s.grid.iter().enumerate() {
        if r.ix != i as u32 % s.grid_x
            || r.iy != i as u32 / s.grid_x
            || [r.location_covariate, r.location_offset]
                .into_iter()
                .any(|v| !v.is_finite())
        {
            return Err(EmbeddingFactorError::Invalid(
                "location grid is incomplete or non-finite".into(),
            ));
        }
    }
    let work = n as u64 * d as u64 * s.factors as u64;
    let inference = if work <= 100_000 {
        "hmc_or_nuts"
    } else {
        "variational_with_multi_start_stability"
    };
    Ok(EmbeddingFactorModel {
        model: EmbeddingFactorModelIr {
            family: "joint_location_embedding_latent_factor",
            coordinate_unit: "micrometer",
            embedding_likelihood:
                "embedding_equals_loadings_times_spatial_factors_plus_diagonal_noise",
            factor_fields: "zero_mean_independent_spatial_latent_factors",
            kernel: "matern_3_2_euclidean_2d",
            field_amplitude: s.field_amplitude,
            field_length_scale_um: s.field_length_scale_um,
            jitter: s.jitter,
            location_component: "exact_grid_log_intensity_with_shrunk_factor_coefficients",
            rotational_identifiability:
                "lower_triangular_first_k_loading_rows_with_positive_diagonal",
            shrinkage: "regularized_global_local_loadings_and_location_factor_coefficients",
            loading_prior_sd: s.loading_prior_sd,
            noise_prior_sd: s.noise_prior_sd,
            inference_recommendation: inference,
            validation_comparators: "vector_variogram_and_kernel_mark_correlation",
            maturity: ModelMaturity::ExperimentalModelConstruction,
        },
        window: s.window,
        grid_x: s.grid_x,
        grid_y: s.grid_y,
        point_count: n as u32,
        embedding_dimension: d as u32,
        factor_count: s.factors,
        feature_names: s.feature_names,
        points: s.points,
        grid: s.grid,
    })
}
