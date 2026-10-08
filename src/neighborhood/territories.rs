use std::mem::size_of;

use crate::{
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multimodal::{
        cells::{CellSection, FusedCell},
        labels::PrimaryLabelEncoding,
    },
    output::NeighborhoodTerritory,
    perf::counters::enforce_storage_budget,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerritoryDomainConfig {
    pub eps_um: f64,
    pub min_cells: usize,
    pub min_radius_um: f64,
}

#[derive(Debug)]
pub(crate) struct TerritoryDetectionAnalysis {
    pub(crate) territories: Vec<NeighborhoodTerritory>,
    pub(crate) estimated_peak_storage_bytes: usize,
}

#[cfg(test)]
pub fn detect_mmr_abnormal_territories(
    cells: &[FusedCell],
    config: TerritoryDomainConfig,
) -> Result<Vec<NeighborhoodTerritory>> {
    validate_config(config)?;
    validate_cells(cells)?;
    let index = SpatialIndex2D::from_points(
        cells
            .iter()
            .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
    )?;
    let labels = PrimaryLabelEncoding::new(cells)?;
    Ok(
        detect_validated_mmr_abnormal_territories(cells, &labels, &index, config, usize::MAX)?
            .territories,
    )
}

pub(crate) fn detect_mmr_abnormal_territories_with_index(
    cells: &[FusedCell],
    labels: &PrimaryLabelEncoding,
    index: &SpatialIndex2D,
    config: TerritoryDomainConfig,
    storage_budget_bytes: usize,
) -> Result<TerritoryDetectionAnalysis> {
    validate_config(config)?;
    validate_cells(cells)?;
    if index.len() != cells.len() {
        return Err(MarklabError::Geometry(format!(
            "spatial index has {} points for {} territory cells",
            index.len(),
            cells.len()
        )));
    }
    if labels.len() != cells.len() {
        return Err(MarklabError::Geometry(format!(
            "primary label encoding has {} entries for {} territory cells",
            labels.len(),
            cells.len()
        )));
    }
    detect_validated_mmr_abnormal_territories(cells, labels, index, config, storage_budget_bytes)
}

fn detect_validated_mmr_abnormal_territories(
    cells: &[FusedCell],
    labels: &PrimaryLabelEncoding,
    index: &SpatialIndex2D,
    config: TerritoryDomainConfig,
    storage_budget_bytes: usize,
) -> Result<TerritoryDetectionAnalysis> {
    let Some(abnormal_label) = labels.id_for("mmr_abnormal") else {
        return Ok(TerritoryDetectionAnalysis {
            territories: Vec::new(),
            estimated_peak_storage_bytes: 0,
        });
    };
    let abnormal_count = cells
        .iter()
        .enumerate()
        .filter(|(index, cell)| {
            cell.source_section == CellSection::Ihc && labels.id_at(*index) == Some(abnormal_label)
        })
        .count();
    let base_storage_bytes = territory_base_storage_bytes(index.len(), abnormal_count);
    enforce_storage_budget(
        "multimodal territory neighborhood plan",
        base_storage_bytes,
        storage_budget_bytes,
    )?;
    let abnormal_indices = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            (cell.source_section == CellSection::Ihc && labels.id_at(index) == Some(abnormal_label))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    if abnormal_indices.is_empty() {
        return Ok(TerritoryDetectionAnalysis {
            territories: Vec::new(),
            estimated_peak_storage_bytes: base_storage_bytes,
        });
    }

    let (neighbors, neighborhood_storage_bytes) = abnormal_neighbor_lists_with_index(
        index,
        &abnormal_indices,
        config.eps_um,
        base_storage_bytes,
        storage_budget_bytes,
    )?;
    let mut visited = vec![false; abnormal_indices.len()];
    let mut assigned = vec![false; abnormal_indices.len()];
    let mut clusters = Vec::new();

    for start in 0..abnormal_indices.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;

        if neighbors[start].len() < config.min_cells {
            continue;
        }

        let mut cluster_positions = Vec::new();
        expand_cluster(
            start,
            &neighbors,
            config.min_cells,
            &mut visited,
            &mut assigned,
            &mut cluster_positions,
        );
        clusters.push(cluster_positions);
    }

    let territories = clusters
        .iter()
        .enumerate()
        .map(|(component_id, cluster_positions)| {
            let component = cluster_positions
                .iter()
                .map(|position| abnormal_indices[*position])
                .collect::<Vec<_>>();
            territory_from_component(component_id, &component, cells, config.min_radius_um)
        })
        .collect();
    Ok(TerritoryDetectionAnalysis {
        territories,
        estimated_peak_storage_bytes: neighborhood_storage_bytes,
    })
}

fn validate_config(config: TerritoryDomainConfig) -> Result<()> {
    if !config.eps_um.is_finite() || config.eps_um <= 0.0 {
        return Err(MarklabError::Config(
            "territory_eps_um must be finite and positive".into(),
        ));
    }
    if config.min_cells == 0 {
        return Err(MarklabError::Config(
            "territory_min_cells must be greater than zero".into(),
        ));
    }
    if !config.min_radius_um.is_finite() || config.min_radius_um < 0.0 {
        return Err(MarklabError::Config(
            "territory_min_radius_um must be finite and non-negative".into(),
        ));
    }
    Ok(())
}

fn validate_cells(cells: &[FusedCell]) -> Result<()> {
    for (index, cell) in cells.iter().enumerate() {
        if !cell.x_um_registered.is_finite() || !cell.y_um_registered.is_finite() {
            return Err(MarklabError::Validation(format!(
                "fused cell {index} registered coordinates must be finite"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn abnormal_neighbor_lists(
    cells: &[FusedCell],
    abnormal_indices: &[usize],
    eps_um: f64,
) -> Result<Vec<Vec<usize>>> {
    let index = SpatialIndex2D::from_points(
        cells
            .iter()
            .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
    )?;
    let base_storage_bytes = territory_base_storage_bytes(index.len(), abnormal_indices.len());
    Ok(abnormal_neighbor_lists_with_index(
        &index,
        abnormal_indices,
        eps_um,
        base_storage_bytes,
        usize::MAX,
    )?
    .0)
}

fn abnormal_neighbor_lists_with_index(
    index: &SpatialIndex2D,
    abnormal_indices: &[usize],
    eps_um: f64,
    base_storage_bytes: usize,
    storage_budget_bytes: usize,
) -> Result<(Vec<Vec<usize>>, usize)> {
    let mut abnormal_position = vec![None; index.len()];
    for (position, cell_index) in abnormal_indices.iter().copied().enumerate() {
        if cell_index >= index.len() {
            return Err(MarklabError::Geometry(format!(
                "abnormal cell index {cell_index} is out of bounds for {} indexed cells",
                index.len()
            )));
        }
        abnormal_position[cell_index] = Some(position);
    }

    let mut lists = Vec::with_capacity(abnormal_indices.len());
    let mut stored_entries = 0usize;
    for (position, cell_index) in abnormal_indices.iter().copied().enumerate() {
        let mut neighbors = Vec::new();
        let mut storage_error = None;
        index.visit_within_radius(cell_index, eps_um, |neighbor| {
            if let Some(position) = abnormal_position[neighbor.index] {
                if storage_error.is_none() {
                    let next_entries = stored_entries
                        .saturating_add(neighbors.len())
                        .saturating_add(1);
                    let required =
                        territory_neighbor_storage_bytes(base_storage_bytes, next_entries);
                    if let Err(error) = enforce_storage_budget(
                        "multimodal territory neighborhood plan",
                        required,
                        storage_budget_bytes,
                    ) {
                        storage_error = Some(error);
                    } else {
                        neighbors.push(position);
                    }
                }
            }
        })?;
        if let Some(error) = storage_error {
            return Err(error);
        }
        let next_entries = stored_entries
            .saturating_add(neighbors.len())
            .saturating_add(1);
        enforce_storage_budget(
            "multimodal territory neighborhood plan",
            territory_neighbor_storage_bytes(base_storage_bytes, next_entries),
            storage_budget_bytes,
        )?;
        neighbors.push(position);
        neighbors.sort_unstable();
        neighbors.dedup();
        stored_entries = stored_entries.saturating_add(neighbors.len());
        lists.push(neighbors);
    }
    Ok((
        lists,
        territory_neighbor_storage_bytes(base_storage_bytes, stored_entries),
    ))
}

fn territory_base_storage_bytes(index_len: usize, abnormal_count: usize) -> usize {
    index_len
        .saturating_mul(size_of::<Option<usize>>())
        .saturating_add(abnormal_count.saturating_mul(size_of::<usize>()))
        .saturating_add(
            abnormal_count
                .saturating_mul(size_of::<Vec<usize>>())
                .saturating_mul(2),
        )
        .saturating_add(abnormal_count.saturating_mul(2))
        .saturating_add(
            abnormal_count
                .saturating_mul(size_of::<usize>())
                .saturating_mul(3),
        )
        .saturating_add(abnormal_count.saturating_mul(size_of::<NeighborhoodTerritory>()))
}

fn territory_neighbor_storage_bytes(base_storage_bytes: usize, stored_entries: usize) -> usize {
    // Vec capacity can temporarily exceed length. Four words per retained
    // entry conservatively covers capacity growth and cluster queue copies.
    base_storage_bytes.saturating_add(
        stored_entries
            .saturating_mul(size_of::<usize>())
            .saturating_mul(4),
    )
}

fn expand_cluster(
    start: usize,
    neighbors: &[Vec<usize>],
    min_cells: usize,
    visited: &mut [bool],
    assigned: &mut [bool],
    cluster_positions: &mut Vec<usize>,
) {
    let mut queue = vec![start];
    assign_to_cluster(start, assigned, cluster_positions);

    while let Some(current) = queue.pop() {
        if neighbors[current].len() < min_cells {
            continue;
        }

        for neighbor in &neighbors[current] {
            if !visited[*neighbor] {
                visited[*neighbor] = true;
                if neighbors[*neighbor].len() >= min_cells {
                    queue.push(*neighbor);
                }
            }
            assign_to_cluster(*neighbor, assigned, cluster_positions);
        }
    }
}

fn assign_to_cluster(index: usize, assigned: &mut [bool], cluster_positions: &mut Vec<usize>) {
    if !assigned[index] {
        assigned[index] = true;
        cluster_positions.push(index);
    }
}

fn territory_from_component(
    component_id: usize,
    component: &[usize],
    cells: &[FusedCell],
    min_radius_um: f64,
) -> NeighborhoodTerritory {
    let supporting_cells = component.len();
    let center_x_um = component
        .iter()
        .map(|index| cells[*index].x_um_registered)
        .sum::<f64>()
        / supporting_cells as f64;
    let center_y_um = component
        .iter()
        .map(|index| cells[*index].y_um_registered)
        .sum::<f64>()
        / supporting_cells as f64;
    let max_component_distance_um = component
        .iter()
        .map(|index| {
            let dx = cells[*index].x_um_registered - center_x_um;
            let dy = cells[*index].y_um_registered - center_y_um;
            dx.hypot(dy)
        })
        .fold(0.0_f64, f64::max);
    let radius_um = (max_component_distance_um + min_radius_um).max(min_radius_um);

    NeighborhoodTerritory {
        center_x_um,
        center_y_um,
        radius_um,
        supporting_abnormal_cells: supporting_cells,
        cluster_id: component_id as u32,
    }
}
