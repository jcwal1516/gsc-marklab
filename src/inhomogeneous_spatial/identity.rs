use marklab_workflow::ContentDigest;

use crate::{mark_pair_plan::erl_workspace_bytes, ObservationWindow2D, Pattern};

use super::{
    intensity::probe_storage_bytes,
    types::{
        InhomogeneousIntensityGridPoint, InhomogeneousIntensityPoint, InhomogeneousSpatialConfig,
        InhomogeneousSpatialError, InhomogeneousSpatialPoint,
    },
};

pub(super) fn retained_bytes(
    points: usize,
    config: &InhomogeneousSpatialConfig,
) -> Result<usize, InhomogeneousSpatialError> {
    let probes = config.integration_grid[0]
        .checked_mul(config.integration_grid[1])
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let geometry =
        crate::geom::spatial_index::SpatialIndex2D::estimated_storage_bytes_for_len(points)
            .checked_add(
                points
                    .checked_mul(std::mem::size_of::<f64>())
                    .ok_or(InhomogeneousSpatialError::SizeOverflow)?,
            )
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let probe_storage =
        probe_storage_bytes(probes).ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let point_storage = points
        .checked_mul(
            std::mem::size_of::<InhomogeneousIntensityPoint>()
                + 5 * std::mem::size_of::<f64>()
                + 4 * std::mem::size_of::<usize>(),
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
        .checked_mul(2)
        .and_then(|value| value.checked_add(probe_storage))
        .and_then(|value| value.checked_add(point_storage))
        .and_then(|value| value.checked_add(matrix))
        .and_then(|value| value.checked_add(descriptors))
        .and_then(|value| value.checked_add(radius_work))
        .and_then(|value| value.checked_add(unique_work))
        .and_then(|value| value.checked_add(erl))
        .ok_or(InhomogeneousSpatialError::SizeOverflow)
}

pub(crate) fn configuration_digest(config: &InhomogeneousSpatialConfig) -> ContentDigest {
    let mut fields = vec![b"marklab-inhomogeneous-spatial-config-v1".to_vec()];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        (config.integration_grid[0] as u128).to_be_bytes().to_vec(),
        (config.integration_grid[1] as u128).to_be_bytes().to_vec(),
        (config.simulations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        config
            .minimum_intensity_per_um2
            .to_bits()
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_probes as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_intensity_evaluations as u128)
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

pub(crate) fn intensity_result_digest(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
    points: &[InhomogeneousIntensityPoint],
    fixed_grid: &[InhomogeneousIntensityGridPoint],
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-leave-one-out-gaussian-intensity-v1".to_vec(),
        window.descriptor().logical_digest.as_bytes().to_vec(),
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        (config.integration_grid[0] as u128).to_be_bytes().to_vec(),
        (config.integration_grid[1] as u128).to_be_bytes().to_vec(),
        (fixed_grid.len() as u128).to_be_bytes().to_vec(),
    ];
    for (row, point) in points.iter().enumerate() {
        fields.push(pattern.x_um[row].to_bits().to_be_bytes().to_vec());
        fields.push(pattern.y_um[row].to_bits().to_be_bytes().to_vec());
        fields.push(point.intensity_per_um2.to_bits().to_be_bytes().to_vec());
        fields.push(point.boundary_mass.to_bits().to_be_bytes().to_vec());
    }
    fields.push(
        fixed_grid_digest(window, config, fixed_grid)
            .as_bytes()
            .to_vec(),
    );
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

pub(crate) fn fixed_grid_digest(
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
    fixed_grid: &[InhomogeneousIntensityGridPoint],
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-fixed-gaussian-intensity-grid-v1".to_vec(),
        window.descriptor().logical_digest.as_bytes().to_vec(),
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        (config.integration_grid[0] as u128).to_be_bytes().to_vec(),
        (config.integration_grid[1] as u128).to_be_bytes().to_vec(),
        (fixed_grid.len() as u128).to_be_bytes().to_vec(),
    ];
    for point in fixed_grid {
        fields.push((point.probe_index as u128).to_be_bytes().to_vec());
        fields.push(point.x_um.to_bits().to_be_bytes().to_vec());
        fields.push(point.y_um.to_bits().to_be_bytes().to_vec());
        fields.push(point.intensity_per_um2.to_bits().to_be_bytes().to_vec());
        fields.push(point.cell_mass.to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}
