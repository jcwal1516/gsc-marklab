use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VesselSource {
    pub vessel_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub source_concentration_per_time: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptakeCell {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub linear_uptake_per_time: f64,
}

#[derive(Clone, Debug)]
pub struct VascularTransportSpec {
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub initial_concentration_row_major: Vec<f64>,
    pub diffusion_um2_per_time_row_major: Vec<f64>,
    pub velocity_x_um_per_time_row_major: Vec<f64>,
    pub velocity_y_um_per_time_row_major: Vec<f64>,
    pub flow_approximation: String,
    pub vessel_sources: Vec<VesselSource>,
    pub uptake_cells: Vec<UptakeCell>,
    pub final_time: f64,
    pub time_step: f64,
    pub hypoxia_threshold: f64,
    pub record_every_steps: u32,
    pub maximum_cell_steps: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MappedVesselSource {
    pub vessel_id: String,
    pub grid_index: usize,
    pub ix: u32,
    pub iy: u32,
    pub mapping_distance_um: f64,
    pub source_concentration_per_time: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MappedUptakeCell {
    pub cell_id: String,
    pub grid_index: usize,
    pub ix: u32,
    pub iy: u32,
    pub mapping_distance_um: f64,
    pub linear_uptake_per_time: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VascularTransportStateCell {
    pub ix: u32,
    pub iy: u32,
    pub x_um: f64,
    pub y_um: f64,
    pub concentration: f64,
    pub gradient_magnitude_per_um: f64,
    pub vessel_source_concentration_per_time: f64,
    pub linear_uptake_per_time: f64,
    pub hypoxic: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct VascularTransportSnapshot {
    pub time: f64,
    pub total_concentration_mass: f64,
    pub minimum_concentration: f64,
    pub maximum_concentration: f64,
    pub hypoxic_cells: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct VascularHypoxicRegion {
    pub region_id: u32,
    pub cell_count: u32,
    pub area_um2: f64,
    pub minimum_concentration: f64,
    pub grid_indices: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VascularTransportSolverDiagnostics {
    pub method: &'static str,
    pub boundary_condition: &'static str,
    pub flow_approximation: &'static str,
    pub completed_steps: u32,
    pub cell_steps: u64,
    pub diffusive_cfl_number: f64,
    pub advective_cfl_number: f64,
    pub combined_cfl_number: f64,
    pub cfl_limit: f64,
    pub cumulative_vessel_source_mass: f64,
    pub cumulative_cell_uptake_mass: f64,
    pub maximum_transport_mass_residual: f64,
    pub mass_balance_residual: f64,
    pub positivity_violations: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VascularTransportResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator_id: &'static str,
    pub simulator_version: u32,
    pub dimensionality: u32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub spacing_x_um: f64,
    pub spacing_y_um: f64,
    pub final_time: f64,
    pub requested_time_step: f64,
    pub hypoxia_threshold: f64,
    pub mapped_vessel_sources: Vec<MappedVesselSource>,
    pub mapped_uptake_cells: Vec<MappedUptakeCell>,
    pub initial_state: Vec<VascularTransportStateCell>,
    pub final_state: Vec<VascularTransportStateCell>,
    pub hypoxic_regions: Vec<VascularHypoxicRegion>,
    pub trajectory: Vec<VascularTransportSnapshot>,
    pub solver: VascularTransportSolverDiagnostics,
    pub observation_model: &'static str,
    pub claim_status: &'static str,
}
