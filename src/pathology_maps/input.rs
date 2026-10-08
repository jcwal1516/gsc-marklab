use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipe {
    pub format: String,
    pub version: u32,
    pub study_id: String,
    pub marker: Marker,
    pub phenotypes: Phenotypes,
    pub design: Design,
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Marker {
    pub name: String,
    pub unit: String,
    pub measurement_status: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Phenotypes {
    pub names: Vec<String>,
    pub measurement_status: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Design {
    pub profile_edges_um: Vec<f64>,
    pub neighborhood_radii_um: Vec<f64>,
    pub minimum_neighbors: usize,
    pub dominance_threshold: f64,
    pub local_radius_um: f64,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slide {
    pub slide_id: String,
    pub patient_id: String,
    pub coordinate_frame_id: String,
    pub window: serde_json::Value,
    pub annotations: Vec<Annotation>,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Annotation {
    pub id: String,
    pub geometry: serde_json::Value,
    pub provenance: String,
    pub boundary_uncertainty_um: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cell {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub marker: Option<f64>,
    pub phenotype_probabilities: Vec<f32>,
    pub stratum: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Limits {
    pub maximum_cells: usize,
    pub maximum_pair_visits: usize,
    pub maximum_permutation_edge_evaluations: usize,
    pub maximum_geometry_vertices: usize,
    pub maximum_geometry_work: usize,
    pub memory_budget_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            maximum_cells: 100_000,
            maximum_pair_visits: 2_000_000,
            maximum_permutation_edge_evaluations: 50_000_000,
            maximum_geometry_vertices: 100_000,
            maximum_geometry_work: 50_000_000,
            memory_budget_bytes: 512 * 1024 * 1024,
        }
    }
}
