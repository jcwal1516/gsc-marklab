use marklab_workflow::ContentDigest;

use crate::{
    analyze_compartment_interface_profile, classical::SpatialGeometryPlan2D,
    BinaryCompartmentPartition2D, ClassicalSpatialLimits, CompartmentInterfaceLimits,
    DeclaredScalarPatternInput,
};

use super::types::*;

pub fn analyze_compartment_cell_mixing(
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    config: &CompartmentCellMixingConfig,
) -> Result<CompartmentCellMixingResult, CompartmentCellMixingError> {
    let profile_limits = CompartmentInterfaceLimits::new(
        config.limits.maximum_points,
        config.limits.maximum_distance_queries,
        config.limits.maximum_retained_bytes,
    )
    .map_err(interface)?;
    let profile = analyze_compartment_interface_profile(input, partition, &profile_limits)
        .map_err(interface)?;
    let geometry_limits = ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        1,
        config.limits.maximum_pair_visits,
        1,
        config.limits.maximum_retained_bytes,
    )
    .map_err(geometry)?;
    let pattern = input.pattern();
    let plan = SpatialGeometryPlan2D::new(
        &pattern.x_um,
        &pattern.y_um,
        partition.observation_window(),
        geometry_limits,
    )
    .map_err(geometry)?;
    let result_bytes = pattern
        .len()
        .checked_mul(2 * std::mem::size_of::<bool>() + 3 * std::mem::size_of::<usize>())
        .and_then(|value| {
            value.checked_add(2 * std::mem::size_of::<CompartmentCellMixingSummary>())
        })
        .ok_or(CompartmentCellMixingError::SizeOverflow)?;
    let estimated_storage_bytes = profile
        .estimated_storage_bytes
        .checked_add(plan.estimated_storage_bytes())
        .and_then(|value| value.checked_add(result_bytes))
        .ok_or(CompartmentCellMixingError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(CompartmentCellMixingError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let descriptor = partition.descriptor();
    let labels = profile
        .rows
        .iter()
        .map(|row| row.compartment_id == descriptor.positive_compartment_id)
        .collect::<Vec<_>>();
    let negative_count = labels.iter().filter(|value| !**value).count();
    let positive_count = labels.len() - negative_count;
    let mut visits = 0_usize;
    let mut edges = 0_usize;
    let mut cross = 0_usize;
    let mut same_incidences = [0_usize; 2];
    let mut cross_incidences = [0_usize; 2];
    for source in 0..labels.len() {
        let mut failure = None;
        plan.index()
            .visit_within_radius(source, config.radius_um, |neighbor| {
                if failure.is_some() {
                    return;
                }
                if let Err(error) = add_count(&mut visits, 1) {
                    failure = Some(error);
                    return;
                }
                if visits > config.limits.maximum_pair_visits {
                    failure = Some(CompartmentCellMixingError::PairVisitLimitExceeded {
                        observed: visits,
                        maximum: config.limits.maximum_pair_visits,
                    });
                    return;
                }
                if neighbor.index <= source {
                    return;
                }
                if let Err(error) = add_count(&mut edges, 1) {
                    failure = Some(error);
                    return;
                }
                let source_role = usize::from(labels[source]);
                let neighbor_role = usize::from(labels[neighbor.index]);
                if source_role == neighbor_role {
                    if let Err(error) = add_count(&mut same_incidences[source_role], 2) {
                        failure = Some(error);
                    }
                } else {
                    if let Err(error) = add_count(&mut cross, 1)
                        .and_then(|()| add_count(&mut cross_incidences[source_role], 1))
                        .and_then(|()| add_count(&mut cross_incidences[neighbor_role], 1))
                    {
                        failure = Some(error);
                    }
                }
            })
            .map_err(geometry)?;
        if let Some(error) = failure {
            return Err(error);
        }
    }
    if edges == 0 {
        return Err(CompartmentCellMixingError::NoAdjacencyEdges);
    }
    let cross_fraction = cross as f64 / edges as f64;
    let expectation = 2.0 * negative_count as f64 * positive_count as f64
        / (labels.len() as f64 * (labels.len() - 1) as f64);
    let entropy = binary_entropy(cross_fraction);
    let configuration_digest = configuration_digest(input, partition, config)?;
    let graph_digest = ContentDigest::from_framed([
        b"marklab-compartment-radius-graph-v1".as_slice(),
        plan.logical_digest().as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        configuration_digest.as_bytes(),
    ]);
    Ok(CompartmentCellMixingResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        mark_id: profile.mark_id,
        measurement_status: profile.measurement_status,
        coordinate_frame_id: profile.coordinate_frame_id,
        partition_digest: profile.partition_digest,
        radius_um: config.radius_um,
        graph_digest: graph_digest.to_string(),
        configuration_digest: configuration_digest.to_string(),
        point_count: labels.len(),
        undirected_edge_count: edges,
        directed_pair_visits: visits,
        cross_compartment_edge_count: cross,
        cross_compartment_edge_fraction: cross_fraction,
        random_label_cross_edge_expectation: expectation,
        cross_edge_fraction_minus_expectation: cross_fraction - expectation,
        edge_type_entropy_nats: entropy,
        normalized_edge_type_entropy: entropy / 2.0_f64.ln(),
        negative: summary(
            &descriptor.negative_compartment_id,
            negative_count,
            same_incidences[0],
            cross_incidences[0],
        ),
        positive: summary(
            &descriptor.positive_compartment_id,
            positive_count,
            same_incidences[1],
            cross_incidences[1],
        ),
        estimated_storage_bytes,
        limits: config.limits,
    })
}

fn summary(id: &str, cells: usize, same: usize, cross: usize) -> CompartmentCellMixingSummary {
    let total = same + cross;
    let fraction = if total == 0 {
        0.0
    } else {
        cross as f64 / total as f64
    };
    let entropy = binary_entropy(fraction);
    CompartmentCellMixingSummary {
        compartment_id: id.into(),
        cell_count: cells,
        same_compartment_neighbor_incidences: same,
        cross_compartment_neighbor_incidences: cross,
        neighbor_label_entropy_nats: entropy,
        normalized_neighbor_label_entropy: entropy / 2.0_f64.ln(),
    }
}

fn binary_entropy(probability: f64) -> f64 {
    let mut value = 0.0;
    if probability > 0.0 {
        value -= probability * probability.ln();
    }
    if probability < 1.0 {
        let complement = 1.0 - probability;
        value -= complement * complement.ln();
    }
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn add_count(target: &mut usize, amount: usize) -> Result<(), CompartmentCellMixingError> {
    *target = target
        .checked_add(amount)
        .ok_or(CompartmentCellMixingError::SizeOverflow)?;
    Ok(())
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    config: &CompartmentCellMixingConfig,
) -> Result<ContentDigest, CompartmentCellMixingError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        CompartmentCellMixingError::InterfaceProfile {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-compartment-cell-mixing-v1".as_slice(),
        declared.digest().as_bytes(),
        partition.descriptor().logical_digest.as_bytes(),
        &config.radius_um.to_bits().to_be_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_distance_queries as u128).to_be_bytes(),
        &(config.limits.maximum_pair_visits as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

fn interface(error: impl std::fmt::Display) -> CompartmentCellMixingError {
    CompartmentCellMixingError::InterfaceProfile {
        reason: error.to_string(),
    }
}

fn geometry(error: impl std::fmt::Display) -> CompartmentCellMixingError {
    CompartmentCellMixingError::Geometry {
        reason: error.to_string(),
    }
}
