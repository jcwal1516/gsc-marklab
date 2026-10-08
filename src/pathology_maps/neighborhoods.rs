use crate::{soft_neighborhood_composition::composition_rows, Result};

use super::{
    admission::Geometry,
    charge,
    input::{Recipe, Slide},
    invalid,
    result::{NeighborhoodMap, NeighborhoodRow},
};

pub(super) fn analyze(
    slide: &Slide,
    geometry: &Geometry,
    recipe: &Recipe,
    remaining: &mut usize,
) -> Result<Vec<NeighborhoodMap>> {
    let mut maps = Vec::with_capacity(recipe.design.neighborhood_radii_um.len());
    for radius in &recipe.design.neighborhood_radii_um {
        let composition = composition_rows(
            &geometry.index,
            &geometry.ids,
            &geometry.probabilities,
            recipe.phenotypes.names.len(),
            *radius,
            *remaining,
        )
        .map_err(|error| invalid(error.to_string()))?;
        charge(
            remaining,
            composition.directed_pair_visits,
            "neighborhood pairs",
        )?;
        let mut rows = Vec::with_capacity(slide.cells.len());
        for (cell, row) in slide.cells.iter().zip(composition.rows) {
            let distance = geometry
                .window
                .boundary_distance_um(cell.x_um, cell.y_um)
                .map_err(|error| invalid(error.to_string()))?;
            if !distance.is_finite() {
                return Err(invalid("nonfinite distance to observation-window edge"));
            }
            let effective_diversity = row.mean_neighbor_probabilities.as_ref().map(|values| {
                (-values
                    .iter()
                    .copied()
                    .filter(|p| *p > 0.0)
                    .map(|p| p * p.ln())
                    .sum::<f64>())
                .exp()
            });
            let dominant = row.mean_neighbor_probabilities.as_ref().and_then(|values| {
                let mut candidates = values
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| **value >= recipe.design.dominance_threshold);
                let first = candidates.next().map(|(index, _)| index);
                if candidates.next().is_none() {
                    first
                } else {
                    None
                }
            });
            let state = if row.neighbor_count < recipe.design.minimum_neighbors {
                "insufficient_support"
            } else if distance < *radius {
                "boundary_truncated"
            } else if dominant.is_some() {
                "dominant"
            } else {
                "mixed"
            };
            rows.push(NeighborhoodRow {
                cell_id: row.cell_id,
                x_um: cell.x_um,
                y_um: cell.y_um,
                neighbor_count: row.neighbor_count,
                composition: row.mean_neighbor_probabilities,
                effective_diversity,
                state,
                dominant_phenotype: dominant
                    .filter(|_| state == "dominant")
                    .map(|index| recipe.phenotypes.names[index].clone()),
                distance_to_tissue_edge_um: distance,
            });
        }
        maps.push(NeighborhoodMap {
            radius_um: *radius,
            rows,
        });
    }
    Ok(maps)
}
