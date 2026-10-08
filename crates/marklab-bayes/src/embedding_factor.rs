use crate::{JointLocationGridRow, RectangularWindow};
use serde::Serialize;
use std::collections::HashSet;
use thiserror::Error;
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingFactorPoint {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub location_covariate: f64,
    pub location_offset: f64,
    pub embedding: Vec<f64>,
}
#[derive(Clone, Debug)]
pub struct EmbeddingFactorSpec {
    pub points: Vec<EmbeddingFactorPoint>,
    pub feature_names: Vec<String>,
    pub grid: Vec<JointLocationGridRow>,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub factors: u32,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub jitter: f64,
    pub loading_prior_sd: f64,
    pub noise_prior_sd: f64,
}
#[derive(Debug, Error)]
pub enum EmbeddingFactorError {
    #[error("invalid joint location-embedding factor model: {0}")]
    Invalid(String),
}
#[derive(Debug, Serialize)]
pub struct EmbeddingFactorModelIr {
    pub family: &'static str,
    pub coordinate_unit: &'static str,
    pub embedding_likelihood: &'static str,
    pub factor_fields: &'static str,
    pub kernel: &'static str,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub jitter: f64,
    pub location_component: &'static str,
    pub rotational_identifiability: &'static str,
    pub shrinkage: &'static str,
    pub loading_prior_sd: f64,
    pub noise_prior_sd: f64,
    pub inference_recommendation: &'static str,
    pub validation_comparators: &'static str,
    pub maturity: &'static str,
}
#[derive(Debug, Serialize)]
pub struct EmbeddingFactorModel {
    pub model: EmbeddingFactorModelIr,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub point_count: u32,
    pub embedding_dimension: u32,
    pub factor_count: u32,
    pub feature_names: Vec<String>,
    pub points: Vec<EmbeddingFactorPoint>,
    pub grid: Vec<JointLocationGridRow>,
}
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
            maturity: "experimental_model_construction",
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
