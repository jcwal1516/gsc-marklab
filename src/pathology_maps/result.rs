use serde::Serialize;

use super::input::{Design, Limits, Marker, Phenotypes};

/// Experimental H&E/IHC annotation profiles and within-slide spatial maps.
///
/// Serialize to inspect the version-one application result. Patient identity is retained;
/// local randomization probabilities do not treat cells as independent patients.
#[derive(Clone, Debug, Serialize)]
pub struct PathologyMapsResult {
    pub(super) format: &'static str,
    pub(super) version: u32,
    pub(super) study_id: String,
    pub(super) marker: Marker,
    pub(super) phenotypes: Phenotypes,
    pub(super) design: Design,
    pub(super) limits: Limits,
    pub(super) claim_scope: &'static str,
    pub(super) slides: Vec<SlideResult>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SlideResult {
    pub slide_id: String,
    pub patient_id: String,
    pub coordinate_frame_id: String,
    pub bounds_um: [f64; 4],
    pub window: serde_json::Value,
    pub profiles: Vec<StructureProfile>,
    pub neighborhood_maps: Vec<NeighborhoodMap>,
    pub local_map: LocalMap,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct StructureProfile {
    pub annotation_id: String,
    pub provenance: String,
    pub geometry: serde_json::Value,
    pub boundary_uncertainty_um: f64,
    pub distance_convention: &'static str,
    pub area_method: &'static str,
    pub maximum_buffer_chord_error_um: f64,
    pub bands: Vec<ProfileBand>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ProfileBand {
    pub lower_um: f64,
    pub upper_um: f64,
    pub area_um2: f64,
    pub cell_count: usize,
    pub observed_markers: usize,
    pub missing_markers: usize,
    pub marker_mean: Option<f64>,
    pub cell_density_per_mm2: Option<f64>,
    pub phenotype_mass: Vec<f64>,
    pub phenotype_density_per_mm2: Option<Vec<f64>>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct NeighborhoodMap {
    pub radius_um: f64,
    pub rows: Vec<NeighborhoodRow>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct NeighborhoodRow {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub neighbor_count: usize,
    pub composition: Option<Vec<f64>>,
    pub effective_diversity: Option<f64>,
    pub state: &'static str,
    pub dominant_phenotype: Option<String>,
    pub distance_to_tissue_edge_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct LocalMap {
    pub radius_um: f64,
    pub null: &'static str,
    pub multiplicity: &'static str,
    pub eligible_cells: usize,
    pub permutations_completed: usize,
    pub rows: Vec<LocalRow>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct LocalRow {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub value: Option<f64>,
    pub permutation_stratum: String,
    pub status: &'static str,
    pub statistic: Option<f64>,
    pub raw_p_value: Option<f64>,
    pub within_slide_adjusted_p_value: Option<f64>,
    pub adjusted_p_value: Option<f64>,
    pub quadrant: Option<&'static str>,
    pub classification: &'static str,
}
