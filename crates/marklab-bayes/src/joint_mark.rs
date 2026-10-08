use crate::RectangularWindow;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JointCategoricalPoint {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub mark_id: String,
    pub location_covariate: f64,
    pub location_offset: f64,
    pub mark_covariate: f64,
    pub neighborhood_effect: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JointLocationGridRow {
    pub ix: u32,
    pub iy: u32,
    pub location_covariate: f64,
    pub location_offset: f64,
}
#[derive(Clone, Debug)]
pub struct JointCategoricalMarkSpec {
    pub points: Vec<JointCategoricalPoint>,
    pub grid: Vec<JointLocationGridRow>,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub reference_mark: String,
    pub location_prior_sd: f64,
    pub mark_coefficient_prior_sd: f64,
    pub mark_field_prior_sd: f64,
}
#[derive(Debug, Error)]
pub enum JointMarkError {
    #[error("invalid joint location-mark model: {0}")]
    Invalid(String),
}
#[derive(Debug, Serialize)]
pub struct JointCategoricalMarkModelIr {
    pub family: &'static str,
    pub coordinate_unit: &'static str,
    pub location_component: &'static str,
    pub mark_component: &'static str,
    pub joint_likelihood: &'static str,
    pub reference_mark: String,
    pub softmax_identifiability: &'static str,
    pub mark_latent_fields: &'static str,
    pub location_prior_sd: f64,
    pub mark_coefficient_prior_sd: f64,
    pub mark_field_prior_sd: f64,
    pub random_labeling_comparison: &'static str,
    pub maturity: &'static str,
}
#[derive(Debug, Serialize)]
pub struct JointCategoricalMarkModel {
    pub model: JointCategoricalMarkModelIr,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_area_um2: f64,
    pub point_count: u32,
    pub mark_counts: BTreeMap<String, u32>,
    pub points: Vec<JointCategoricalPoint>,
    pub grid: Vec<JointLocationGridRow>,
}
pub fn build_joint_location_mark_model(
    mut s: JointCategoricalMarkSpec,
) -> Result<JointCategoricalMarkModel, JointMarkError> {
    if s.points.len() < 4
        || s.points.len() > 10_000
        || s.grid_x == 0
        || s.grid_y == 0
        || s.grid_x as u64 * s.grid_y as u64 > 4096
        || s.grid.len() != s.grid_x as usize * s.grid_y as usize
        || ![
            s.window.xmin_um,
            s.window.ymin_um,
            s.window.xmax_um,
            s.window.ymax_um,
            s.location_prior_sd,
            s.mark_coefficient_prior_sd,
            s.mark_field_prior_sd,
        ]
        .into_iter()
        .all(f64::is_finite)
        || s.window.xmin_um >= s.window.xmax_um
        || s.window.ymin_um >= s.window.ymax_um
        || s.location_prior_sd <= 0.0
        || s.mark_coefficient_prior_sd <= 0.0
        || s.mark_field_prior_sd <= 0.0
    {
        return Err(JointMarkError::Invalid(
            "dimensions, window, or prior scales are invalid".into(),
        ));
    }
    s.points.sort_by(|a, b| a.point_id.cmp(&b.point_id));
    let mut ids = HashSet::new();
    let mut counts = BTreeMap::<String, u32>::new();
    for p in &s.points {
        if p.point_id.is_empty()
            || !ids.insert(p.point_id.as_str())
            || p.mark_id.is_empty()
            || ![
                p.x_um,
                p.y_um,
                p.location_covariate,
                p.location_offset,
                p.mark_covariate,
                p.neighborhood_effect,
            ]
            .into_iter()
            .all(f64::is_finite)
            || !(s.window.xmin_um..s.window.xmax_um).contains(&p.x_um)
            || !(s.window.ymin_um..s.window.ymax_um).contains(&p.y_um)
        {
            return Err(JointMarkError::Invalid(
                "point identity, values, or window membership are invalid".into(),
            ));
        }
        *counts.entry(p.mark_id.clone()).or_default() += 1;
    }
    if !(2..=16).contains(&counts.len())
        || counts.values().any(|n| *n < 2)
        || !counts.contains_key(&s.reference_mark)
    {
        return Err(JointMarkError::Invalid(
            "marks require 2-16 types, replication, and an exact reference".into(),
        ));
    }
    s.grid.sort_by_key(|r| (r.iy, r.ix));
    for (index, row) in s.grid.iter().enumerate() {
        if row.ix != index as u32 % s.grid_x
            || row.iy != index as u32 / s.grid_x
            || ![row.location_covariate, row.location_offset]
                .into_iter()
                .all(f64::is_finite)
        {
            return Err(JointMarkError::Invalid(
                "location grid must be complete, ordered, and finite".into(),
            ));
        }
    }
    let cell_area = (s.window.xmax_um - s.window.xmin_um) * (s.window.ymax_um - s.window.ymin_um)
        / (s.grid.len() as f64);
    Ok(JointCategoricalMarkModel {
        model: JointCategoricalMarkModelIr {
            family: "joint_location_categorical_mark",
            coordinate_unit: "micrometer",
            location_component: "exact_rectangle_log_linear_point_process",
            mark_component:
                "reference_category_softmax_mark_covariate_neighborhood_and_latent_field",
            joint_likelihood: "point_process_location_plus_conditional_categorical_mark",
            reference_mark: s.reference_mark,
            softmax_identifiability: "reference_intercept_coefficients_and_field_exactly_zero",
            mark_latent_fields: "independent_mark_specific_zero_mean_fields",
            location_prior_sd: s.location_prior_sd,
            mark_coefficient_prior_sd: s.mark_coefficient_prior_sd,
            mark_field_prior_sd: s.mark_field_prior_sd,
            random_labeling_comparison: "required_nested_zero_neighborhood_and_mark_fields",
            maturity: "experimental_model_construction",
        },
        window: s.window,
        grid_x: s.grid_x,
        grid_y: s.grid_y,
        cell_area_um2: cell_area,
        point_count: s.points.len() as u32,
        mark_counts: counts,
        points: s.points,
        grid: s.grid,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JointContinuousPoint {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub mark_y: f64,
    pub location_covariate: f64,
    pub location_offset: f64,
    pub mark_covariate: f64,
}
#[derive(Clone, Debug)]
pub struct JointContinuousMarkSpec {
    pub points: Vec<JointContinuousPoint>,
    pub grid: Vec<JointLocationGridRow>,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub known_mark_noise_sd: f64,
    pub shared_field_amplitude: f64,
    pub shared_field_length_scale_um: f64,
    pub jitter: f64,
    pub mark_loading_prior_sd: f64,
    pub private_field_prior_sd: f64,
}
#[derive(Debug, Serialize)]
pub struct JointContinuousMarkModelIr {
    pub family: &'static str,
    pub coordinate_unit: &'static str,
    pub location_component: &'static str,
    pub mark_component: &'static str,
    pub joint_likelihood: &'static str,
    pub kernel: &'static str,
    pub shared_field_amplitude: f64,
    pub shared_field_length_scale_um: f64,
    pub jitter: f64,
    pub location_shared_loading: &'static str,
    pub mark_shared_loading: &'static str,
    pub mark_loading_prior_sd: f64,
    pub private_fields: &'static str,
    pub private_field_prior_sd: f64,
    pub known_mark_noise_sd: f64,
    pub identifiability: &'static str,
    pub separate_model_comparison: &'static str,
    pub maturity: &'static str,
}
#[derive(Debug, Serialize)]
pub struct JointContinuousMarkModel {
    pub model: JointContinuousMarkModelIr,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_area_um2: f64,
    pub point_count: u32,
    pub points: Vec<JointContinuousPoint>,
    pub grid: Vec<JointLocationGridRow>,
}
pub fn build_joint_continuous_mark_model(
    mut s: JointContinuousMarkSpec,
) -> Result<JointContinuousMarkModel, JointMarkError> {
    if !(8..=10_000).contains(&s.points.len())
        || s.grid_x == 0
        || s.grid_y == 0
        || s.grid.len() != s.grid_x as usize * s.grid_y as usize
        || [
            s.window.xmin_um,
            s.window.ymin_um,
            s.window.xmax_um,
            s.window.ymax_um,
            s.known_mark_noise_sd,
            s.shared_field_amplitude,
            s.shared_field_length_scale_um,
            s.jitter,
            s.mark_loading_prior_sd,
            s.private_field_prior_sd,
        ]
        .into_iter()
        .any(|v| !v.is_finite())
        || s.window.xmin_um >= s.window.xmax_um
        || s.window.ymin_um >= s.window.ymax_um
        || [
            s.known_mark_noise_sd,
            s.shared_field_amplitude,
            s.shared_field_length_scale_um,
            s.jitter,
            s.mark_loading_prior_sd,
            s.private_field_prior_sd,
        ]
        .into_iter()
        .any(|v| v <= 0.0)
    {
        return Err(JointMarkError::Invalid(
            "continuous joint model dimensions, window, or scales are invalid".into(),
        ));
    }
    s.points.sort_by(|a, b| a.point_id.cmp(&b.point_id));
    let mut ids = HashSet::new();
    for p in &s.points {
        if p.point_id.is_empty()
            || !ids.insert(p.point_id.as_str())
            || [
                p.x_um,
                p.y_um,
                p.mark_y,
                p.location_covariate,
                p.location_offset,
                p.mark_covariate,
            ]
            .into_iter()
            .any(|v| !v.is_finite())
            || !(s.window.xmin_um..s.window.xmax_um).contains(&p.x_um)
            || !(s.window.ymin_um..s.window.ymax_um).contains(&p.y_um)
        {
            return Err(JointMarkError::Invalid(
                "continuous points are invalid or outside window".into(),
            ));
        }
    }
    for values in [
        s.points.iter().map(|p| p.mark_y).collect::<Vec<_>>(),
        s.points.iter().map(|p| p.location_covariate).collect(),
        s.points.iter().map(|p| p.mark_covariate).collect(),
    ] {
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if max - min <= f64::EPSILON.sqrt() * min.abs().max(max.abs()).max(1.0) {
            return Err(JointMarkError::Invalid(
                "mark and both covariates must vary materially".into(),
            ));
        }
    }
    s.grid.sort_by_key(|r| (r.iy, r.ix));
    for (i, r) in s.grid.iter().enumerate() {
        if r.ix != i as u32 % s.grid_x
            || r.iy != i as u32 / s.grid_x
            || [r.location_covariate, r.location_offset]
                .into_iter()
                .any(|v| !v.is_finite())
        {
            return Err(JointMarkError::Invalid(
                "continuous model location grid is incomplete".into(),
            ));
        }
    }
    let area = (s.window.xmax_um - s.window.xmin_um) * (s.window.ymax_um - s.window.ymin_um)
        / s.grid.len() as f64;
    Ok(JointContinuousMarkModel {
        model: JointContinuousMarkModelIr {
            family: "joint_location_continuous_mark",
            coordinate_unit: "micrometer",
            location_component:
                "exact_rectangle_log_linear_point_process_with_shared_and_private_field",
            mark_component: "gaussian_identity_continuous_mark_with_shared_and_private_field",
            joint_likelihood: "point_process_location_plus_conditional_gaussian_mark",
            kernel: "matern_3_2_euclidean_2d",
            shared_field_amplitude: s.shared_field_amplitude,
            shared_field_length_scale_um: s.shared_field_length_scale_um,
            jitter: s.jitter,
            location_shared_loading: "fixed_one",
            mark_shared_loading: "positive_half_normal",
            mark_loading_prior_sd: s.mark_loading_prior_sd,
            private_fields: "independent_location_and_mark_zero_mean_matern",
            private_field_prior_sd: s.private_field_prior_sd,
            known_mark_noise_sd: s.known_mark_noise_sd,
            identifiability: "fixed_shared_scale_and_location_loading_plus_positive_mark_loading",
            separate_model_comparison: "required_nested_zero_shared_loading",
            maturity: "experimental_model_construction",
        },
        window: s.window,
        grid_x: s.grid_x,
        grid_y: s.grid_y,
        cell_area_um2: area,
        point_count: s.points.len() as u32,
        points: s.points,
        grid: s.grid,
    })
}
