use serde::Serialize;

use super::input::{Architecture, Design, Layer, Limits};

/// Research-tier epithelial architecture relative to a derived invasive front.
///
/// Serialize to inspect the version-one application result. Region and front geometry
/// are caller-declared or derived from caller-declared polygons; nothing is segmented here.
#[derive(Clone, Debug, Serialize)]
pub struct PathologyArchitectureResult {
    pub(super) format: &'static str,
    pub(super) version: u32,
    pub(super) study_id: String,
    pub(super) architecture: Architecture,
    pub(super) epithelium: Layer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) declared_front: Option<Layer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cells: Option<Layer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) spots: Option<Layer>,
    pub(super) design: Design,
    pub(super) limits: Limits,
    pub(super) claim_scope: &'static str,
    pub(super) class_order: Vec<String>,
    pub(super) slides: Vec<SlideResult>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SlideResult {
    pub slide_id: String,
    pub patient_id: String,
    pub coordinate_frame_id: String,
    pub tissue_area_um2: f64,
    pub epithelial_area_um2: f64,
    pub epithelial_component_count: usize,
    pub front: FrontResult,
    pub cell_summary: Option<CellSummary>,
    pub budding: Option<BuddingResult>,
    pub bands: Vec<BandResult>,
    pub components: Vec<ComponentRow>,
    pub cells: Vec<CellRow>,
    pub spots: Vec<SpotRow>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SpotRow {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub diameter_um: f64,
    pub signed_front_distance_um: Option<f64>,
    pub band: Option<usize>,
    pub tissue_fraction: f64,
    pub tumor_bulk_fraction: Option<f64>,
    pub epithelial_fraction: f64,
    /// Fraction of the spot area covered by each class's epithelium, in class order.
    pub class_fractions: Vec<f64>,
    pub dominant_class: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct CellSummary {
    pub cell_count: usize,
    pub epithelial_cell_count: usize,
    pub epithelial_cells_outside_epithelium: usize,
    pub nucleus_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct BuddingResult {
    pub status: &'static str,
    pub definition: &'static str,
    pub bud_count: usize,
    pub buds_inside_bulk: usize,
    pub buds_outside_bulk: usize,
    pub mucin_excluded_candidates: usize,
    pub hotspot: Option<Hotspot>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Hotspot {
    pub center_um: Option<[f64; 2]>,
    pub radius_um: f64,
    pub area_mm2: f64,
    pub bud_count: usize,
    pub bud_count_per_0_785_mm2: f64,
    pub grade: &'static str,
    pub field_tissue_fraction: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct CellRow {
    pub id: String,
    pub epithelial: bool,
    pub x_um: f64,
    pub y_um: f64,
    pub component_id: Option<String>,
    pub class: Option<String>,
    #[serde(skip)]
    pub class_index: Option<usize>,
    pub signed_front_distance_um: Option<f64>,
    pub band: Option<usize>,
    pub nucleus: Option<NucleusRow>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct NucleusRow {
    pub area_um2: f64,
    pub circularity: f64,
    pub solidity: f64,
    pub aspect_ratio: f64,
    pub front_alignment: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct NucleusSummary {
    pub count: usize,
    pub mean_area_um2: f64,
    pub mean_circularity: f64,
    pub mean_solidity: f64,
    pub mean_aspect_ratio: f64,
    pub mean_front_alignment: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct FrontResult {
    pub status: &'static str,
    pub length_um: f64,
    pub bulk_area_um2: f64,
    pub bulk_component_count: usize,
    pub distance_convention: &'static str,
    pub area_method: &'static str,
    pub maximum_buffer_chord_error_um: f64,
    pub declared_agreement: Option<FrontAgreement>,
    pub lines: serde_json::Value,
    pub bulk: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct FrontAgreement {
    pub declared_length_um: f64,
    pub sample_spacing_um: f64,
    pub derived_to_declared: DistanceSummary,
    pub declared_to_derived: DistanceSummary,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct DistanceSummary {
    pub samples: usize,
    pub mean_um: f64,
    pub p95_um: f64,
    pub maximum_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct BandResult {
    pub lower_um: f64,
    pub upper_um: f64,
    pub tissue_area_um2: f64,
    pub epithelial_area_um2: f64,
    pub epithelial_area_fraction: Option<f64>,
    pub classes: Vec<ClassBand>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ClassBand {
    pub class: String,
    pub epithelial_area_um2: f64,
    pub epithelial_area_share: Option<f64>,
    pub component_count: usize,
    pub component_area_um2: f64,
    pub median_component_area_um2: Option<f64>,
    pub normalized_component_area_entropy: Option<f64>,
    pub mean_absolute_curvature_per_um: Option<f64>,
    pub area_weighted_isoperimetric_quotient: Option<f64>,
    pub lumen_containing_component_fraction: Option<f64>,
    pub bud_count: Option<usize>,
    pub branch_points_per_mm2: Option<f64>,
    pub skeleton_length_um_per_mm2: Option<f64>,
    pub epithelial_nuclei: Option<NucleusSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ComponentRow {
    pub id: String,
    pub class: String,
    #[serde(skip)]
    pub class_index: usize,
    pub x_um: f64,
    pub y_um: f64,
    pub area_um2: f64,
    pub perimeter_um: f64,
    pub hole_count: usize,
    pub lumen_count: Option<usize>,
    pub isoperimetric_quotient: f64,
    pub absolute_turning_rad: f64,
    pub curvature_contour_length_um: f64,
    pub mean_absolute_curvature_per_um: Option<f64>,
    pub signed_front_distance_um: Option<f64>,
    pub band: Option<usize>,
    pub epithelial_cell_count: Option<usize>,
    pub bud: Option<bool>,
    pub skeleton_length_um: Option<f64>,
    pub branch_point_count: Option<usize>,
    pub end_point_count: Option<usize>,
}
