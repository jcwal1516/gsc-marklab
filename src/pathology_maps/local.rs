use crate::{
    local_multivariate_moran_permutation, LocalMultivariateMoranLimits,
    LocalMultivariateMoranPoint, Result,
};

use super::{
    admission::Geometry,
    charge,
    input::{Recipe, Slide},
    invalid,
    result::{LocalMap, LocalRow},
};

pub(super) fn analyze(
    slide: &Slide,
    geometry: &Geometry,
    recipe: &Recipe,
    remaining_pairs: &mut usize,
    remaining_permutations: &mut usize,
) -> Result<LocalMap> {
    let rows = slide
        .cells
        .iter()
        .zip(&geometry.ids)
        .map(|(cell, id)| LocalRow {
            cell_id: id.as_str().into(),
            x_um: cell.x_um,
            y_um: cell.y_um,
            value: cell.marker,
            permutation_stratum: cell.stratum.clone(),
            status: if cell.marker.is_some() {
                "isolated_observed_point"
            } else {
                "missing_marker"
            },
            statistic: None,
            raw_p_value: None,
            within_slide_adjusted_p_value: None,
            adjusted_p_value: None,
            quadrant: None,
            classification: "unavailable",
        })
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    for (index, cell) in slide.cells.iter().enumerate() {
        if cell.marker.is_none() {
            continue;
        }
        let neighbors = geometry
            .index
            .within_radius(index, recipe.design.local_radius_um)?;
        charge(remaining_pairs, neighbors.len(), "local observed graph")?;
        if neighbors
            .iter()
            .any(|neighbor| slide.cells[neighbor.index].marker.is_some())
        {
            selected.push(index);
        }
    }
    let mut map = LocalMap {
        radius_um: recipe.design.local_radius_um,
        null: "fixed_graph_whole_values_randomly_labeled_within_declared_strata; population_variance_standardization",
        multiplicity: "within_slide_max_abs_all_eligible_locations_then_bonferroni_across_all_input_slides",
        eligible_cells: selected.len(), permutations_completed: 0, rows,
    };
    if selected.len() < 3 {
        for index in selected {
            map.rows[index].status = "insufficient_observed_points";
        }
        return Ok(map);
    }
    let values = selected
        .iter()
        .map(|index| slide.cells[*index].marker.expect("observed selection"))
        .collect::<Vec<_>>();
    if values.windows(2).all(|pair| pair[0] == pair[1]) {
        for index in selected {
            map.rows[index].status = "zero_variance";
        }
        return Ok(map);
    }
    // Subtract before scaling to preserve exactly represented small differences at large offsets.
    // Opposite extreme finite values can overflow subtraction, so scale those before subtracting.
    let anchor = values[0];
    let mut residuals = values
        .iter()
        .map(|value| value - anchor)
        .collect::<Vec<_>>();
    if residuals.iter().any(|value| !value.is_finite()) {
        let scale = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
        residuals = values
            .iter()
            .map(|value| value / scale - anchor / scale)
            .collect();
    }
    let scale = residuals.iter().copied().map(f64::abs).fold(0.0, f64::max);
    for value in &mut residuals {
        *value /= scale;
    }
    let points = selected
        .iter()
        .zip(&residuals)
        .map(|(index, value)| {
            let cell = &slide.cells[*index];
            LocalMultivariateMoranPoint {
                point_id: geometry.ids[*index].as_str().into(),
                permutation_stratum: cell.stratum.clone(),
                x_um: cell.x_um,
                y_um: cell.y_um,
                values: vec![*value],
            }
        })
        .collect::<Vec<_>>();
    let result = local_multivariate_moran_permutation(
        &points,
        std::slice::from_ref(&recipe.marker.name),
        &geometry.window,
        recipe.design.local_radius_um,
        recipe.design.permutations,
        recipe.design.seed,
        LocalMultivariateMoranLimits {
            maximum_points: recipe.limits.maximum_cells,
            maximum_dimension: 1,
            maximum_directed_edges: recipe.limits.maximum_pair_visits,
            maximum_permutation_edge_evaluations: *remaining_permutations,
            memory_budget_bytes: recipe.limits.memory_budget_bytes,
        },
    )
    .map_err(|error| invalid(error.to_string()))?;
    charge(
        remaining_permutations,
        result.permutation_edge_evaluations,
        "local permutation",
    )?;
    map.permutations_completed = result.permutations_completed;
    for ((index, value), location) in selected.iter().zip(&residuals).zip(result.locations) {
        let z = (value - result.feature_means[0]) / result.feature_standard_deviations[0];
        let lag = if z == 0.0 {
            0.0
        } else {
            location.statistic / z
        };
        let quadrant = match (z > 0.0, lag > 0.0) {
            _ if z == 0.0 || lag == 0.0 => None,
            (true, true) => Some("high_high"),
            (false, false) => Some("low_low"),
            (true, false) => Some("high_low"),
            (false, true) => Some("low_high"),
        };
        let adjusted = (location.adjusted_p_value * recipe.slides.len() as f64).min(1.0);
        let row = &mut map.rows[*index];
        row.status = "available";
        row.statistic = Some(location.statistic);
        row.raw_p_value = Some(location.raw_p_value);
        row.within_slide_adjusted_p_value = Some(location.adjusted_p_value);
        row.adjusted_p_value = Some(adjusted);
        row.quadrant = quadrant;
        row.classification = if adjusted > recipe.design.alpha {
            "not_significant"
        } else {
            match quadrant {
                Some("high_high") => "high_high_hotspot",
                Some("low_low") => "low_low_coldspot",
                Some("high_low") => "high_low_outlier",
                Some("low_high") => "low_high_outlier",
                _ => "neutral",
            }
        };
    }
    Ok(map)
}
