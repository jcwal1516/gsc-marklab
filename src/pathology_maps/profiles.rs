use crate::{common::stats::mean_all_finite, geom::window::PROFILE_BUFFER_ANGLE, Result};

use super::{
    admission::Geometry,
    charge,
    input::{Recipe, Slide},
    invalid,
    result::{ProfileBand, StructureProfile},
};

pub(super) fn analyze(
    slide: &Slide,
    geometry: &Geometry,
    recipe: &Recipe,
    remaining: &mut usize,
) -> Result<Vec<StructureProfile>> {
    let edges = &recipe.design.profile_edges_um;
    let classes = recipe.phenotypes.names.len();
    let mut results = Vec::with_capacity(slide.annotations.len());
    for (source, annotation) in slide.annotations.iter().zip(&geometry.annotations) {
        let annotation_vertices = annotation.descriptor().vertex_count;
        let tissue_vertices = geometry.window.descriptor().vertex_count;
        let estimated_vertices = annotation_vertices
            .checked_mul(128)
            .and_then(|n| n.checked_add(tissue_vertices))
            .ok_or_else(|| invalid("annotation geometry size overflow"))?;
        if estimated_vertices > recipe.limits.maximum_geometry_vertices {
            return Err(invalid("annotation buffer vertex resource limit exceeded"));
        }
        let work = estimated_vertices
            .checked_mul(annotation_vertices + tissue_vertices)
            .and_then(|n| n.checked_mul(edges.len()))
            .and_then(|n| n.checked_add(slide.cells.len().saturating_mul(annotation_vertices)))
            .ok_or_else(|| invalid("annotation work overflow"))?;
        charge(remaining, work, "annotation geometry")?;
        let cumulative = edges
            .iter()
            .map(|distance| {
                geometry
                    .window
                    .clipped_annotation_offset_area(
                        annotation,
                        *distance,
                        recipe.limits.maximum_geometry_vertices,
                    )
                    .map_err(|error| invalid(error.to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut bands = Vec::with_capacity(edges.len() - 1);
        for (bounds, areas) in edges.windows(2).zip(cumulative.windows(2)) {
            let area = areas[1] - areas[0];
            if area < -1e-10 * geometry.window.area_um2() {
                return Err(invalid("offset areas are not monotone"));
            }
            bands.push(ProfileBand {
                lower_um: bounds[0],
                upper_um: bounds[1],
                area_um2: area.max(0.0),
                cell_count: 0,
                observed_markers: 0,
                missing_markers: 0,
                marker_mean: None,
                cell_density_per_mm2: None,
                phenotype_mass: vec![0.0; classes],
                phenotype_density_per_mm2: None,
            });
        }
        let mut marks = vec![Vec::new(); bands.len()];
        for cell in &slide.cells {
            let distance = -annotation
                .signed_boundary_distance_um(cell.x_um, cell.y_um)
                .map_err(|error| invalid(error.to_string()))?;
            if !distance.is_finite() {
                return Err(invalid("nonfinite annotation distance"));
            }
            if distance < edges[0] || distance > edges[edges.len() - 1] {
                continue;
            }
            let band_index = edges
                .partition_point(|edge| *edge <= distance)
                .saturating_sub(1)
                .min(bands.len() - 1);
            let band = &mut bands[band_index];
            band.cell_count += 1;
            if let Some(value) = cell.marker {
                marks[band_index].push(value);
                band.observed_markers += 1;
            } else {
                band.missing_markers += 1;
            }
            for (mass, probability) in band
                .phenotype_mass
                .iter_mut()
                .zip(&cell.phenotype_probabilities)
            {
                *mass += f64::from(*probability);
            }
        }
        for (band, values) in bands.iter_mut().zip(&marks) {
            if !values.is_empty() {
                let scale = values
                    .iter()
                    .copied()
                    .map(f64::abs)
                    .fold(0.0, f64::max)
                    .max(f64::MIN_POSITIVE);
                band.marker_mean = mean_all_finite(values.iter().map(|value| *value / scale))
                    .map(|mean| mean * scale);
                if band.marker_mean.is_none_or(|value| !value.is_finite()) {
                    return Err(invalid("nonfinite band marker mean"));
                }
            }
            if band.area_um2 > 0.0 {
                let density = |count: f64| count / band.area_um2 * 1_000_000.0;
                let total = density(band.cell_count as f64);
                let phenotype = band
                    .phenotype_mass
                    .iter()
                    .copied()
                    .map(density)
                    .collect::<Vec<_>>();
                if !total.is_finite() || phenotype.iter().any(|value| !value.is_finite()) {
                    return Err(invalid("nonfinite band density"));
                }
                band.cell_density_per_mm2 = Some(total);
                band.phenotype_density_per_mm2 = Some(phenotype);
            } else if band.cell_count > 0 {
                return Err(invalid(
                    "cells occupy an annotation band with zero resolved area",
                ));
            }
        }
        results.push(StructureProfile {
            annotation_id: source.id.clone(),
            provenance: source.provenance.clone(),
            geometry: source.geometry.clone(),
            boundary_uncertainty_um: source.boundary_uncertainty_um,
            distance_convention:
                "negative_inside_positive_outside; half_open_bands_final_upper_inclusive",
            area_method: "tissue_clipped_round_polygon_offsets; angular_step_0.05_radians",
            maximum_buffer_chord_error_um: edges.iter().copied().map(f64::abs).fold(0.0, f64::max)
                * (1.0 - (PROFILE_BUFFER_ANGLE / 2.0).cos()),
            bands,
        });
    }
    Ok(results)
}
