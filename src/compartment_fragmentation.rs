use serde::{Deserialize, Serialize};

use crate::common::{finite::canonical_zero, summation::kahan_add};

use crate::BinaryCompartmentPartition2D;

/// Exact vector-polygon fragmentation summary for one compartment role.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentFragmentationSummary {
    /// Exact oriented compartment identity.
    pub compartment_id: String,
    /// Number of disconnected polygon components.
    pub component_count: usize,
    /// Number of holes across those components.
    pub hole_count: usize,
    /// Canonically sorted component areas in square micrometres.
    pub component_areas_um2: Vec<f64>,
    /// Total compartment area in square micrometres.
    pub total_area_um2: f64,
    /// Complete exterior-plus-hole perimeter in micrometres.
    pub total_perimeter_um: f64,
    /// Largest component's fraction of total compartment area.
    pub largest_component_area_fraction: f64,
    /// Shannon entropy of component-area proportions in natural-log units.
    pub component_area_entropy_nats: f64,
    /// Component-area entropy divided by `ln(component_count)`, or zero for one component.
    pub normalized_component_area_entropy: f64,
    /// Complete perimeter divided by area, in inverse micrometres.
    pub perimeter_area_ratio_per_um: f64,
}

/// Role-preserving binary compartment fragmentation result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentFragmentationResult {
    /// Exact partition identity.
    pub partition_digest: String,
    /// Shared physical coordinate frame.
    pub coordinate_frame_id: String,
    /// Declared negative-side fragmentation.
    pub negative: CompartmentFragmentationSummary,
    /// Declared positive-side fragmentation.
    pub positive: CompartmentFragmentationSummary,
    /// Explicit status of the distinct cell-adjacency mixing estimand.
    pub cell_mixing_status: String,
}

/// Derive exact component-area fragmentation from one validated binary partition.
pub fn compartment_fragmentation(
    partition: &BinaryCompartmentPartition2D,
) -> CompartmentFragmentationResult {
    let descriptor = partition.descriptor();
    CompartmentFragmentationResult {
        partition_digest: descriptor.logical_digest.to_string(),
        coordinate_frame_id: descriptor.coordinate_frame_id.as_str().into(),
        negative: summarize(
            &descriptor.negative_compartment_id,
            descriptor.negative_component_count,
            descriptor.negative_hole_count,
            partition.negative_component_areas_um2(),
            descriptor.negative_area_um2,
            descriptor.negative_boundary_length_um,
        ),
        positive: summarize(
            &descriptor.positive_compartment_id,
            descriptor.positive_component_count,
            descriptor.positive_hole_count,
            partition.positive_component_areas_um2(),
            descriptor.positive_area_um2,
            descriptor.positive_boundary_length_um,
        ),
        cell_mixing_status:
            "unavailable_requires_declared_physical_adjacency_scale_and_typed_cell_rows".into(),
    }
}

fn summarize(
    compartment_id: &str,
    component_count: usize,
    hole_count: usize,
    component_areas: &[f64],
    total_area: f64,
    perimeter: f64,
) -> CompartmentFragmentationSummary {
    let mut entropy = 0.0;
    let mut correction = 0.0;
    for area in component_areas {
        let probability = area / total_area;
        kahan_add(
            &mut entropy,
            &mut correction,
            -probability * probability.ln(),
        );
    }
    let entropy = canonical_zero(entropy + correction);
    let normalized = if component_count > 1 {
        entropy / (component_count as f64).ln()
    } else {
        0.0
    };
    CompartmentFragmentationSummary {
        compartment_id: compartment_id.into(),
        component_count,
        hole_count,
        component_areas_um2: component_areas.to_vec(),
        total_area_um2: total_area,
        total_perimeter_um: perimeter,
        largest_component_area_fraction: component_areas[component_areas.len() - 1] / total_area,
        component_area_entropy_nats: entropy,
        normalized_component_area_entropy: canonical_zero(normalized),
        perimeter_area_ratio_per_um: perimeter / total_area,
    }
}
