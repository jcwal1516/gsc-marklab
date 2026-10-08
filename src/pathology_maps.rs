//! One concrete H&E/IHC workflow; numerical and geometry owners remain shared.

mod admission;
mod input;
mod local;
mod neighborhoods;
mod profiles;
mod publication;
mod render;
mod result;

pub use publication::publish_pathology_maps;
pub use result::PathologyMapsResult;

use crate::{MarklabError, Result};

/// Analyze strict version-one pathology-map JSON with bounded native computations.
///
/// Produces annotation-relative profiles, descriptive composition maps, and a single-marker
/// local random-labeling family per slide. The combined location/slide family uses existing
/// within-slide Max-absolute adjustment and Bonferroni across slides. No filesystem I/O.
pub fn analyze_pathology_maps(recipe_json: &[u8]) -> Result<PathologyMapsResult> {
    let prepared = admission::prepare(recipe_json)?;
    let recipe = prepared.recipe;
    let mut slides = Vec::with_capacity(recipe.slides.len());
    let mut remaining_pairs = recipe.limits.maximum_pair_visits;
    let mut remaining_permutation_work = recipe.limits.maximum_permutation_edge_evaluations;
    let mut remaining_geometry_work = recipe.limits.maximum_geometry_work;
    for (slide, geometry) in recipe.slides.iter().zip(&prepared.geometry) {
        let profiles = profiles::analyze(slide, geometry, &recipe, &mut remaining_geometry_work)?;
        let neighborhood_maps =
            neighborhoods::analyze(slide, geometry, &recipe, &mut remaining_pairs)?;
        let local_map = local::analyze(
            slide,
            geometry,
            &recipe,
            &mut remaining_pairs,
            &mut remaining_permutation_work,
        )?;
        slides.push(result::SlideResult {
            slide_id: slide.slide_id.clone(),
            patient_id: slide.patient_id.clone(),
            coordinate_frame_id: slide.coordinate_frame_id.clone(),
            bounds_um: geometry.window.descriptor().bounds_um,
            window: slide.window.clone(),
            profiles,
            neighborhood_maps,
            local_map,
        });
    }
    Ok(PathologyMapsResult {
        format: "marklab.pathology_maps_result", version: 1,
        study_id: recipe.study_id, marker: recipe.marker, phenotypes: recipe.phenotypes,
        design: recipe.design, limits: recipe.limits, slides,
        claim_scope: "experimental_annotation_profiles_and_within_slide_maps; no_population_or_discovered_niche_claim",
    })
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(format!("pathology maps: {}", message.into()))
}

fn charge(remaining: &mut usize, work: usize, description: &str) -> Result<()> {
    *remaining = remaining
        .checked_sub(work)
        .ok_or_else(|| invalid(format!("{description} resource limit exceeded")))?;
    Ok(())
}
