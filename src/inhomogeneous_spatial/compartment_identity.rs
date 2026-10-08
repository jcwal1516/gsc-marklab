use marklab_workflow::ContentDigest;

use crate::{mark_pair_plan::erl_workspace_bytes, BinaryCompartmentPartition2D, Pattern};

use super::{
    InhomogeneousPairCorrelationPoint, InhomogeneousSpatialError, InhomogeneousSpatialPoint,
    PiecewiseCompartmentIntensityPoint, PiecewiseCompartmentPairCorrelationConfig,
    PiecewiseCompartmentRole, PiecewiseCompartmentSpatialConfig,
};

pub(super) fn piecewise_configuration_digest(
    config: &PiecewiseCompartmentSpatialConfig,
) -> ContentDigest {
    let mut fields = vec![b"marklab-piecewise-compartment-spatial-config-v1".to_vec()];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        (config.simulations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_compartment_queries as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_pair_visits as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_null_draws as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn piecewise_intensity_digest(
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    negative_count: usize,
    positive_count: usize,
    negative_intensity: f64,
    positive_intensity: f64,
    points: &[PiecewiseCompartmentIntensityPoint],
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-leave-one-out-piecewise-binary-compartment-intensity-v1".to_vec(),
        partition.descriptor().logical_digest.as_bytes().to_vec(),
        (negative_count as u128).to_be_bytes().to_vec(),
        (positive_count as u128).to_be_bytes().to_vec(),
        negative_intensity.to_bits().to_be_bytes().to_vec(),
        positive_intensity.to_bits().to_be_bytes().to_vec(),
    ];
    for (row, point) in points.iter().enumerate() {
        fields.push(pattern.x_um[row].to_bits().to_be_bytes().to_vec());
        fields.push(pattern.y_um[row].to_bits().to_be_bytes().to_vec());
        fields.push(vec![match point.role {
            PiecewiseCompartmentRole::Negative => 0,
            PiecewiseCompartmentRole::Positive => 1,
        }]);
        fields.push(point.intensity_per_um2.to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

pub(super) fn retained_bytes(
    points: usize,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentSpatialConfig,
) -> Result<usize, InhomogeneousSpatialError> {
    let geometry =
        crate::geom::spatial_index::SpatialIndex2D::estimated_storage_bytes_for_len(points)
            .checked_add(
                points
                    .checked_mul(std::mem::size_of::<f64>())
                    .ok_or(InhomogeneousSpatialError::SizeOverflow)?,
            )
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let generated = points
        .checked_mul(3 * std::mem::size_of::<f64>())
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let descriptor = partition.descriptor();
    let maximum_id_bytes = descriptor
        .negative_compartment_id
        .len()
        .max(descriptor.positive_compartment_id.len());
    let point_storage = points
        .checked_mul(
            std::mem::size_of::<PiecewiseCompartmentIntensityPoint>()
                + std::mem::size_of::<PiecewiseCompartmentRole>()
                + std::mem::size_of::<f64>()
                + maximum_id_bytes,
        )
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let matrix = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let descriptors = config
        .simulations
        .checked_mul(std::mem::size_of::<Vec<f64>>())
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let radius_work = config
        .radii_um
        .len()
        .checked_add(1)
        .and_then(|value| {
            value.checked_mul(
                std::mem::size_of::<InhomogeneousSpatialPoint>()
                    + 8 * std::mem::size_of::<f64>()
                    + 5 * std::mem::size_of::<usize>()
                    + 2 * std::mem::size_of::<bool>(),
            )
        })
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let unique_work = points
        .checked_mul(std::mem::size_of::<((u64, u64), usize)>())
        .and_then(|value| value.checked_mul(4))
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let erl = erl_workspace_bytes(config.simulations, config.radii_um.len())
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    geometry
        .checked_add(generated)
        .and_then(|value| value.checked_add(point_storage))
        .and_then(|value| value.checked_add(matrix))
        .and_then(|value| value.checked_add(descriptors))
        .and_then(|value| value.checked_add(radius_work))
        .and_then(|value| value.checked_add(unique_work))
        .and_then(|value| value.checked_add(erl))
        .ok_or(InhomogeneousSpatialError::SizeOverflow)
}

pub(super) fn piecewise_g_configuration_digest(
    config: &PiecewiseCompartmentPairCorrelationConfig,
) -> ContentDigest {
    ContentDigest::from_framed([
        b"marklab-piecewise-compartment-pair-correlation-config-v1".as_slice(),
        piecewise_configuration_digest(&config.intensity).as_bytes(),
        &config.pair_bandwidth_um.to_bits().to_be_bytes(),
    ])
}

pub(super) fn piecewise_g_retained_bytes(
    points: usize,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentPairCorrelationConfig,
) -> Result<usize, InhomogeneousSpatialError> {
    let base = retained_bytes(points, partition, &config.intensity)?;
    let extra = config
        .intensity
        .radii_um
        .len()
        .checked_mul(
            2 * std::mem::size_of::<InhomogeneousPairCorrelationPoint>()
                + 6 * std::mem::size_of::<f64>()
                + 3 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<bool>(),
        )
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    base.checked_add(extra)
        .ok_or(InhomogeneousSpatialError::SizeOverflow)
}
