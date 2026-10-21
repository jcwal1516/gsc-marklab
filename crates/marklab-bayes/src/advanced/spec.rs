use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Number;

fn u64_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Number, D::Error> {
    let number = Number::deserialize(deserializer)?;
    if number.as_u64().is_none() {
        return Err(serde::de::Error::custom("expected an unsigned integer"));
    }
    Ok(number)
}

fn u32_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Number, D::Error> {
    let number = u64_number(deserializer)?;
    if number
        .as_u64()
        .is_none_or(|value| value > u64::from(u32::MAX))
    {
        return Err(serde::de::Error::custom(
            "expected a 32-bit unsigned integer",
        ));
    }
    Ok(number)
}

fn usize_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Number, D::Error> {
    let number = u64_number(deserializer)?;
    if number
        .as_u64()
        .is_none_or(|value| usize::try_from(value).is_err())
    {
        return Err(serde::de::Error::custom(
            "expected a pointer-sized unsigned integer",
        ));
    }
    Ok(number)
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HmcNormalSpec {
    pub observations: Vec<Number>,
    pub observation_standard_deviation: Number,
    pub prior_mean: Number,
    pub prior_standard_deviation: Number,
    pub initial_state: Number,
    pub mass: Number,
    pub step_size: Number,
    pub leapfrog_steps: Number,
    pub warmup: Number,
    pub draws: Number,
    pub seed: Number,
    pub timeout_seconds: Number,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularSpdeSpec {
    pub window: RectangularWindow,
    pub mesh_resolutions: Vec<Number>,
    pub alpha: Number,
    pub kappa: Number,
    pub tau: Number,
    pub lgcp_points: Vec<[Number; 2]>,
    pub lgcp_beta_prior_sd: Number,
    pub factor_observations: Vec<RectangularFactorObservation>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub factor_count: Option<Number>,
    pub factor_noise_sd: Number,
    pub maximum_iterations: Number,
    pub timeout_seconds: Number,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularWindow {
    pub frame: String,
    pub bounds: [Number; 4],
    pub holes: Vec<Vec<[Number; 2]>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularFactorObservation {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub region_id: Option<String>,
    pub coordinates: [Number; 2],
    pub values: Vec<Number>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiPolygon {
    #[serde(rename = "type")]
    pub geometry_type: MultiPolygonType,
    /// Optional GeoJSON bounding box (RFC 7946, section 5); the window is defined by
    /// `coordinates` alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<Vec<Number>>,
    pub coordinates: Vec<Vec<Vec<[Number; 2]>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MultiPolygonType {
    MultiPolygon,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveWindowSpdeSpec {
    pub window_frame: String,
    pub window: MultiPolygon,
    #[serde(deserialize_with = "u32_number")]
    pub base_resolution: Number,
    #[serde(deserialize_with = "u32_number")]
    pub boundary_refinement_levels: Number,
    pub maximum_relative_area_error: Number,
    #[serde(deserialize_with = "u32_number")]
    pub alpha: Number,
    pub kappa: Number,
    pub tau: Number,
    pub factor_observations: Vec<AdaptiveWindowSpdeObservation>,
    #[serde(deserialize_with = "u32_number")]
    pub factor_count: Number,
    pub factor_noise_sd: Number,
    #[serde(deserialize_with = "u32_number")]
    pub maximum_iterations: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_vertices: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_triangles: Number,
    #[serde(deserialize_with = "u64_number")]
    pub maximum_boundary_segment_triangle_checks: Number,
    #[serde(deserialize_with = "u64_number")]
    pub maximum_projection_triangle_visits: Number,
    #[serde(deserialize_with = "usize_number")]
    pub memory_budget_mib: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_result_bytes: Number,
    pub timeout_seconds: Number,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveWindowSpdeObservation {
    pub region_id: String,
    pub coordinates: [Number; 2],
    pub values: Vec<Number>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationaryAdaptiveWindowSpdeSpec {
    pub window_frame: String,
    pub window: MultiPolygon,
    #[serde(deserialize_with = "u32_number")]
    pub base_resolution: Number,
    #[serde(deserialize_with = "u32_number")]
    pub boundary_refinement_levels: Number,
    pub maximum_relative_area_error: Number,
    pub background: NonstationarySpdeParameters,
    pub parameter_regions: Vec<NonstationarySpdeRegion>,
    #[serde(deserialize_with = "u32_number")]
    pub alpha: Number,
    pub factor_observations: Vec<AdaptiveWindowSpdeObservation>,
    #[serde(deserialize_with = "u32_number")]
    pub factor_count: Number,
    pub factor_noise_sd: Number,
    #[serde(deserialize_with = "u32_number")]
    pub maximum_iterations: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_vertices: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_triangles: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_candidate_points: Number,
    #[serde(deserialize_with = "u64_number")]
    pub maximum_boundary_segment_triangle_checks: Number,
    #[serde(deserialize_with = "u64_number")]
    pub maximum_projection_triangle_visits: Number,
    #[serde(deserialize_with = "usize_number")]
    pub memory_budget_mib: Number,
    #[serde(deserialize_with = "usize_number")]
    pub maximum_result_bytes: Number,
    pub timeout_seconds: Number,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationarySpdeParameters {
    pub kappa: Number,
    pub tau: Number,
    pub anisotropy_ratio: Number,
    pub major_axis_degrees: Number,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonstationarySpdeRegion {
    pub region_id: String,
    pub center: [Number; 2],
    pub radius: Number,
    pub kappa: Number,
    pub tau: Number,
    pub anisotropy_ratio: Number,
    pub major_axis_degrees: Number,
    #[serde(deserialize_with = "u32_number")]
    pub interior_refinement_levels: Number,
}
