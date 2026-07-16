use crate::{
    errors::{MarklabError, Result},
    multimodal::cell_table::{primary_label, CellSection, FusedCell},
    output::TerritoryFeature,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerritoryDomainConfig {
    pub eps_um: f64,
    pub min_cells: usize,
    pub min_radius_um: f64,
}

pub fn detect_mmr_abnormal_territories(
    cells: &[FusedCell],
    config: TerritoryDomainConfig,
) -> Result<Vec<TerritoryFeature>> {
    validate_config(config)?;
    validate_cells(cells)?;

    let abnormal_indices = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| is_mmr_abnormal_ihc_cell(cell).then_some(index))
        .collect::<Vec<_>>();
    if abnormal_indices.is_empty() {
        return Ok(Vec::new());
    }

    let neighbors = abnormal_neighbor_lists(cells, &abnormal_indices, config.eps_um);
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
    cell.source_section == CellSection::Ihc
        && primary_label(cell).as_deref() == Some("mmr_abnormal")
}

fn abnormal_neighbor_lists(
    cells: &[FusedCell],
    abnormal_indices: &[usize],
    eps_um: f64,
) -> Vec<Vec<usize>> {
    let mut neighbors = vec![Vec::new(); abnormal_indices.len()];
    for left in 0..abnormal_indices.len() {
        for right in left..abnormal_indices.len() {
            let left_index = abnormal_indices[left];
            let right_index = abnormal_indices[right];
            if fused_cell_distance_um(&cells[left_index], &cells[right_index]) <= eps_um {
                neighbors[left].push(right);
                if left != right {
                    neighbors[right].push(left);
                }
            }
        }
    }
    neighbors
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
) -> TerritoryFeature {
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

    TerritoryFeature {
        center_x_um,
        center_y_um,
        radius_um,
        scale_um: radius_um / 2.0_f64.sqrt(),
        z_or_power: supporting_cells as f64,
        supporting_cells,
        component_id: Some(component_id as u32),
        qc_overlap_fraction: 0.0,
    }
}

fn fused_cell_distance_um(left: &FusedCell, right: &FusedCell) -> f64 {
    let dx = left.x_um_registered - right.x_um_registered;
    let dy = left.y_um_registered - right.y_um_registered;
    dx.hypot(dy)
}
