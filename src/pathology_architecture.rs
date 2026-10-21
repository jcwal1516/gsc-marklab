//! Epithelial architecture relative to a derived invasive front; geometry owners remain shared.

mod bands;
mod components;
mod front;
mod geometry;
mod input;
mod preparation;
mod publication;
mod result;

pub use publication::publish_pathology_architecture;
pub use result::PathologyArchitectureResult;

use geo::Area;

use crate::{
    common::finite::validate_serializable_finite, geom::window::buffer_chord_error_bound_um,
    MarklabError, Result,
};

const UNCLASSIFIED: &str = "unclassified";

/// Analyze strict version-one architecture JSON with bounded native geometry.
///
/// Closes declared tumor tissue into a bulk, derives its advancing front, and reports
/// tissue-clipped distance-band areas by architecture class with per-component
/// fragmentation, lumen and contour-curvature summaries. Descriptive only; no filesystem I/O.
pub fn analyze_pathology_architecture(recipe_json: &[u8]) -> Result<PathologyArchitectureResult> {
    let prepared = preparation::prepare(recipe_json)?;
    let mut remaining = prepared.remaining_work;
    let recipe = prepared.recipe;
    let class_order = recipe
        .architecture
        .classes
        .iter()
        .cloned()
        .chain(std::iter::once(UNCLASSIFIED.to_owned()))
        .collect::<Vec<_>>();
    let design = &recipe.design;
    let chord_error = buffer_chord_error_bound_um(
        design
            .band_edges_um
            .iter()
            .copied()
            .chain([design.closing_radius_um, design.tissue_edge_tolerance_um])
            .map(f64::abs)
            .fold(0.0, f64::max),
    );
    let mut slides = Vec::with_capacity(recipe.slides.len());
    for (slide, geometry) in recipe.slides.iter().zip(&prepared.slides) {
        let front = front::derive(geometry, design, &recipe.limits, &mut remaining)?;
        let declared_agreement = geometry
            .declared_front
            .as_ref()
            .map(|declared| {
                front::agreement(
                    &front,
                    declared,
                    design.contour_sample_spacing_um,
                    &mut remaining,
                )
            })
            .transpose()?
            .flatten();
        let components = components::measure(
            geometry,
            &front,
            design,
            &recipe.architecture.classes,
            &mut remaining,
        )?;
        let bands = bands::analyze(
            geometry,
            &front,
            &components,
            &recipe,
            &class_order,
            &mut remaining,
        )?;
        slides.push(result::SlideResult {
            slide_id: slide.slide_id.clone(),
            patient_id: slide.patient_id.clone(),
            coordinate_frame_id: slide.coordinate_frame_id.clone(),
            tissue_area_um2: geometry.window_area_um2,
            epithelial_area_um2: geometry.epithelial_area_um2,
            epithelial_component_count: components.len(),
            front: result::FrontResult {
                status: front.status,
                length_um: front.length_um,
                bulk_area_um2: front.bulk.unsigned_area(),
                bulk_component_count: front.bulk.0.len(),
                distance_convention:
                    "negative_inside_tumor_bulk_positive_outside; half_open_bands_final_upper_inclusive",
                area_method: "tissue_clipped_round_polygon_offsets; angular_step_0.05_radians",
                maximum_buffer_chord_error_um: chord_error,
                declared_agreement,
                lines: geometry::multilinestring_json(&front.lines),
                bulk: geometry::multipolygon_json(&front.bulk),
            },
            bands,
            components,
        });
    }
    let result = PathologyArchitectureResult {
        format: "marklab.pathology_architecture_result",
        version: 1,
        study_id: recipe.study_id,
        architecture: recipe.architecture,
        epithelium: recipe.epithelium,
        declared_front: recipe.declared_front,
        design: recipe.design,
        limits: recipe.limits,
        claim_scope: "research_descriptive_architecture_geometry; regions_declared_upstream; no_population_or_mechanism_claim",
        class_order,
        slides,
    };
    validate_serializable_finite(&result).map_err(|error| invalid(error.to_string()))?;
    Ok(result)
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(format!("pathology architecture: {}", message.into()))
}

fn charge(remaining: &mut usize, work: usize, description: &str) -> Result<()> {
    *remaining = remaining
        .checked_sub(work)
        .ok_or_else(|| invalid(format!("{description} resource limit exceeded")))?;
    Ok(())
}
