use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipe {
    pub format: String,
    pub version: u32,
    pub study_id: String,
    pub architecture: Architecture,
    pub epithelium: Layer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_front: Option<Layer>,
    pub design: Design,
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Architecture {
    pub classes: Vec<String>,
    pub measurement_status: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Layer {
    pub measurement_status: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Design {
    pub closing_radius_um: f64,
    pub minimum_bulk_area_um2: f64,
    pub tissue_edge_tolerance_um: f64,
    pub band_edges_um: Vec<f64>,
    pub contour_sample_spacing_um: f64,
    pub curvature_smoothing_um: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slide {
    pub slide_id: String,
    pub patient_id: String,
    pub coordinate_frame_id: String,
    pub window: serde_json::Value,
    pub epithelium: serde_json::Value,
    pub boundary_uncertainty_um: f64,
    pub architecture_regions: Vec<Region>,
    #[serde(default)]
    pub lumens: Option<serde_json::Value>,
    #[serde(default)]
    pub mucin: Option<serde_json::Value>,
    #[serde(default)]
    pub front_exclusions: Option<serde_json::Value>,
    #[serde(default)]
    pub declared_front: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Region {
    pub id: String,
    pub class: String,
    pub geometry: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Limits {
    pub maximum_geometry_vertices: usize,
    pub maximum_components: usize,
    pub maximum_geometry_work: usize,
    pub memory_budget_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            maximum_geometry_vertices: 2_000_000,
            maximum_components: 200_000,
            maximum_geometry_work: 2_000_000_000,
            memory_budget_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}
