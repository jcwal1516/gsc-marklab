use serde::{Deserialize, Serialize};

use crate::BinaryCompartmentPartition2D;

/// One compartment's exact shared-interface contact fraction.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentContactFraction {
    /// Exact oriented compartment identity.
    pub compartment_id: String,
    /// Shared internal interface length in micrometres.
    pub shared_interface_length_um: f64,
    /// Boundary length coincident with the analyzed tissue edge.
    pub outer_tissue_boundary_length_um: f64,
    /// Complete compartment polygon boundary denominator.
    pub denominator_boundary_length_um: f64,
    /// Shared interface divided by complete compartment boundary.
    pub contact_fraction: f64,
}

/// Exact role-preserving binary compartment contact result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentContactResult {
    /// Exact partition identity.
    pub partition_digest: String,
    /// Shared physical coordinate frame.
    pub coordinate_frame_id: String,
    /// Fixed denominator semantics.
    pub denominator: String,
    /// Declared negative-side compartment contact.
    pub negative: CompartmentContactFraction,
    /// Declared positive-side compartment contact.
    pub positive: CompartmentContactFraction,
}

/// Derive exact role-specific contact fractions from one validated partition.
pub fn compartment_contact_fractions(
    partition: &BinaryCompartmentPartition2D,
) -> CompartmentContactResult {
    let descriptor = partition.descriptor();
    CompartmentContactResult {
        partition_digest: descriptor.logical_digest.to_string(),
        coordinate_frame_id: descriptor.coordinate_frame_id.as_str().into(),
        denominator: "complete_compartment_boundary_including_tissue_edge_and_shared_interface"
            .into(),
        negative: contact(
            &descriptor.negative_compartment_id,
            descriptor.interface_length_um,
            descriptor.negative_outer_boundary_length_um,
            descriptor.negative_boundary_length_um,
        ),
        positive: contact(
            &descriptor.positive_compartment_id,
            descriptor.interface_length_um,
            descriptor.positive_outer_boundary_length_um,
            descriptor.positive_boundary_length_um,
        ),
    }
}

fn contact(
    compartment_id: &str,
    interface: f64,
    outer: f64,
    denominator: f64,
) -> CompartmentContactFraction {
    CompartmentContactFraction {
        compartment_id: compartment_id.into(),
        shared_interface_length_um: interface,
        outer_tissue_boundary_length_um: outer,
        denominator_boundary_length_um: denominator,
        contact_fraction: interface / denominator,
    }
}
