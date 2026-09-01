use marklab_workflow::ContentDigest;

use crate::{mark_pair_plan::erl_workspace_bytes, ObservationWindow2D, Pattern};

use super::{
    intensity::{extrema, probe_storage_bytes, FittedIntensity},
    types::{
        InhomogeneousIntensityGridPoint, InhomogeneousIntensityPoint,
        InhomogeneousIntensitySummary, InhomogeneousSpatialConfig, InhomogeneousSpatialError,
        InhomogeneousSpatialPoint,
    },
};

pub(super) fn into_intensity_summary(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
    fitted: FittedIntensity,
) -> Result<InhomogeneousIntensitySummary, InhomogeneousSpatialError> {
    let (minimum, maximum) = extrema(&fitted.observed_intensities)?;
    let grid_digest = fixed_grid_digest(window, config, &fitted.fixed_grid);
    let artifact_digest = intensity_result_digest(
        pattern,
        window,
        config,
        &fitted.point_values,
        &fitted.fixed_grid,
    );
    Ok(InhomogeneousIntensitySummary {
        estimator: "gaussian_kernel".into(),
        kernel: "isotropic_gaussian_2d".into(),
        cross_fit: config.cross_fit_folds.map_or_else(
            || "leave_one_out_n_over_n_minus_one".into(),
            |folds| format!("balanced_cell_id_rank_{folds}_fold"),
        ),
        boundary_correction: "deterministic_cell_center_quadrature".into(),
        bandwidth_um: config.bandwidth_um,
        integration_grid: config.integration_grid,
        retained_probe_count: fitted.grid.probes.len(),
        probe_spacing_um: fitted.grid.spacing_um,
        maximum_probe_displacement_um: 0.5
            * fitted.grid.spacing_um[0].hypot(fitted.grid.spacing_um[1]),
        minimum_intensity_per_um2: config.minimum_intensity_per_um2,
        observed_minimum_intensity_per_um2: minimum,
        observed_maximum_intensity_per_um2: maximum,
        artifact_digest: artifact_digest.to_string(),
        point_values: fitted.point_values,
        fixed_grid_digest: grid_digest.to_string(),
        fixed_grid_total_mass: fitted.fixed_grid_total_mass,
        fixed_grid: fitted.fixed_grid,
    })
}

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
        .and_then(|value| {
            config.cross_fit_folds.map_or(Some(value), |folds| {
                value.checked_add(
                    points
                        .checked_add(folds)?
                        .checked_mul(std::mem::size_of::<usize>())?,
                )
            })
        })
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
    if let Some(folds) = config.cross_fit_folds {
        fields.push(b"balanced-cell-id-rank-cross-fit".to_vec());
        fields.push((folds as u128).to_be_bytes().to_vec());
    }
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
        if config.cross_fit_folds.is_some() {
            b"marklab-cross-fitted-gaussian-intensity-v1".to_vec()
        } else {
            b"marklab-leave-one-out-gaussian-intensity-v1".to_vec()
        },
        window.descriptor().logical_digest.as_bytes().to_vec(),
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        (config.integration_grid[0] as u128).to_be_bytes().to_vec(),
        (config.integration_grid[1] as u128).to_be_bytes().to_vec(),
        (fixed_grid.len() as u128).to_be_bytes().to_vec(),
    ];
    if let Some(folds) = config.cross_fit_folds {
        fields.push((folds as u128).to_be_bytes().to_vec());
    }
    for (row, point) in points.iter().enumerate() {
        if config.cross_fit_folds.is_some() {
            if let Some(ids) = pattern.cell_ids.as_deref() {
                fields.push(ids[row].as_bytes().to_vec());
            }
        }
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
        if config.cross_fit_folds.is_some() {
            b"marklab-cross-fitted-gaussian-intensity-grid-v1".to_vec()
        } else {
            b"marklab-fixed-gaussian-intensity-grid-v1".to_vec()
        },
        window.descriptor().logical_digest.as_bytes().to_vec(),
        config.bandwidth_um.to_bits().to_be_bytes().to_vec(),
        (config.integration_grid[0] as u128).to_be_bytes().to_vec(),
        (config.integration_grid[1] as u128).to_be_bytes().to_vec(),
        (fixed_grid.len() as u128).to_be_bytes().to_vec(),
    ];
    if let Some(folds) = config.cross_fit_folds {
        fields.push((folds as u128).to_be_bytes().to_vec());
    }
    for point in fixed_grid {
        fields.push((point.probe_index as u128).to_be_bytes().to_vec());
        fields.push(point.x_um.to_bits().to_be_bytes().to_vec());
        fields.push(point.y_um.to_bits().to_be_bytes().to_vec());
        fields.push(point.intensity_per_um2.to_bits().to_be_bytes().to_vec());
        fields.push(point.cell_mass.to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}
