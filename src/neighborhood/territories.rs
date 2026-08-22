use crate::{
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multimodal::{
        cells::{CellSection, FusedCell},
        labels::primary_label,
    },
    output::NeighborhoodTerritory,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerritoryDomainConfig {
    pub eps_um: f64,
    pub min_cells: usize,
    pub min_radius_um: f64,
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
    detect_validated_mmr_abnormal_territories(cells, &index, config)
}

pub(crate) fn detect_mmr_abnormal_territories_with_index(
    cells: &[FusedCell],
    index: &SpatialIndex2D,
    config: TerritoryDomainConfig,
) -> Result<Vec<NeighborhoodTerritory>> {
    validate_config(config)?;
    validate_cells(cells)?;
    if index.len() != cells.len() {
        return Err(MarklabError::Geometry(format!(
            "spatial index has {} points for {} territory cells",
            index.len(),
            cells.len()
        )));
    }
    detect_validated_mmr_abnormal_territories(cells, index, config)
}

fn detect_validated_mmr_abnormal_territories(
    cells: &[FusedCell],
    index: &SpatialIndex2D,
    config: TerritoryDomainConfig,
) -> Result<Vec<NeighborhoodTerritory>> {
    let abnormal_indices = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| is_mmr_abnormal_ihc_cell(cell).then_some(index))
        .collect::<Vec<_>>();
    if abnormal_indices.is_empty() {
        return Ok(Vec::new());
    }

    let neighbors = abnormal_neighbor_lists_with_index(index, &abnormal_indices, config.eps_um)?;
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
    Ok(territories)
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

fn is_mmr_abnormal_ihc_cell(cell: &FusedCell) -> bool {
    cell.source_section == CellSection::Ihc && primary_label(cell) == Some("mmr_abnormal")
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
    abnormal_neighbor_lists_with_index(&index, abnormal_indices, eps_um)
}

fn abnormal_neighbor_lists_with_index(
    index: &SpatialIndex2D,
    abnormal_indices: &[usize],
    eps_um: f64,
) -> Result<Vec<Vec<usize>>> {
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

    abnormal_indices
        .iter()
        .copied()
        .enumerate()
        .map(|(position, cell_index)| {
            let mut neighbors = Vec::new();
            index.visit_within_radius(cell_index, eps_um, |neighbor| {
                if let Some(position) = abnormal_position[neighbor.index] {
                    neighbors.push(position);
                }
            })?;
            neighbors.push(position);
            neighbors.sort_unstable();
            neighbors.dedup();
            Ok(neighbors)
        })
        .collect()
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
