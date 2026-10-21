use std::collections::BTreeSet;

use crate::{
    geom::spatial_index::SpatialIndex2D, CellId, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, ObservationWindow2D,
    ObservationWindowLimits, Result, SpatialAxis,
};

use super::{
    input::{Recipe, Slide},
    invalid,
};

pub(super) const MAXIMUM_INPUT_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct Prepared {
    pub recipe: Recipe,
    pub geometry: Vec<Geometry>,
}

pub(super) struct Geometry {
    pub window: ObservationWindow2D,
    pub annotations: Vec<ObservationWindow2D>,
    pub index: SpatialIndex2D,
    pub ids: Vec<CellId>,
    pub probabilities: Vec<f32>,
}

pub(super) fn prepare(bytes: &[u8]) -> Result<Prepared> {
    if bytes.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("recipe exceeds 64 MiB"));
    }
    let mut recipe: Recipe = serde_json::from_slice(bytes)?;
    validate_design(&recipe)?;
    let total_cells = recipe
        .slides
        .iter()
        .try_fold(0usize, |sum, slide| sum.checked_add(slide.cells.len()))
        .ok_or_else(|| invalid("cell count overflow"))?;
    let limits = recipe.limits;
    if total_cells == 0 || total_cells > limits.maximum_cells {
        return Err(invalid("cell resource limit exceeded or no cells"));
    }
    let row_bytes = recipe
        .design
        .neighborhood_radii_um
        .len()
        .checked_mul(recipe.phenotypes.names.len() * 8 + 512)
        .and_then(|value| value.checked_add(2048))
        .ok_or_else(|| invalid("memory estimate overflow"))?;
    let estimate = total_cells
        .checked_mul(row_bytes)
        .and_then(|value| value.checked_add(bytes.len().saturating_mul(8)))
        .and_then(|value| value.checked_add(limits.maximum_pair_visits.saturating_mul(32)))
        .and_then(|value| value.checked_add(limits.maximum_geometry_vertices.saturating_mul(128)))
        .ok_or_else(|| invalid("memory estimate overflow"))?;
    if estimate > limits.memory_budget_bytes {
        return Err(invalid(format!(
            "memory estimate {estimate} exceeds budget {}",
            limits.memory_budget_bytes
        )));
    }
    recipe
        .slides
        .sort_by(|left, right| left.slide_id.cmp(&right.slide_id));
    let mut slides = BTreeSet::new();
    let mut frames = BTreeSet::new();
    let mut geometry = Vec::with_capacity(recipe.slides.len());
    for slide in &mut recipe.slides {
        if !text(&slide.slide_id)
            || !text(&slide.patient_id)
            || !slides.insert(slide.slide_id.clone())
            || !frames.insert(slide.coordinate_frame_id.clone())
            || slide.cells.is_empty()
            || slide.annotations.len() > 16
        {
            return Err(invalid("invalid/duplicate slide or frame identity, empty cells, or invalid annotation count"));
        }
        slide.cells.sort_by(|left, right| left.id.cmp(&right.id));
        slide
            .annotations
            .sort_by(|left, right| left.id.cmp(&right.id));
        geometry.push(prepare_slide(
            slide,
            recipe.phenotypes.names.len(),
            &recipe.design.profile_edges_um,
            limits,
        )?);
    }
    Ok(Prepared { recipe, geometry })
}

fn validate_design(recipe: &Recipe) -> Result<()> {
    let d = &recipe.design;
    let l = recipe.limits;
    let names = &recipe.phenotypes.names;
    if recipe.format != "marklab.pathology_maps_recipe"
        || recipe.version != 1
        || !text(&recipe.study_id)
        || recipe.slides.is_empty()
        || recipe.slides.len() > 64
        || !text(&recipe.marker.name)
        || !text(&recipe.marker.unit)
        || !text(&recipe.marker.provenance)
        || !text(&recipe.phenotypes.provenance)
        || crate::measurement_status_wire::parse(&recipe.marker.measurement_status).is_none()
        || crate::measurement_status_wire::parse(&recipe.phenotypes.measurement_status).is_none()
        || names.len() < 2
        || names.len() > 64
        || names.iter().any(|name| !text(name))
        || names.iter().collect::<BTreeSet<_>>().len() != names.len()
    {
        return Err(invalid(
            "invalid recipe, assay metadata, phenotype names or measurement status",
        ));
    }
    if !increasing(&d.profile_edges_um, 2, 65)
        || !increasing(&d.neighborhood_radii_um, 1, 16)
        || d.neighborhood_radii_um[0] <= 0.0
        || !d.local_radius_um.is_finite()
        || d.local_radius_um <= 0.0
        || !(d.local_radius_um * d.local_radius_um).is_finite()
        || d.neighborhood_radii_um.iter().any(|r| !(r * r).is_finite())
        || d.minimum_neighbors == 0
        || !(d.dominance_threshold > 0.5 && d.dominance_threshold <= 1.0)
        || d.permutations == 0
        || d.permutations > 100_000
        || !(d.alpha > 0.0 && d.alpha < 1.0)
    {
        return Err(invalid(
            "invalid distance bands, radii, support, dominance threshold or inference design",
        ));
    }
    if l.maximum_cells == 0
        || l.maximum_cells > 1_000_000
        || l.maximum_pair_visits == 0
        || l.maximum_pair_visits > 250_000_000
        || l.maximum_permutation_edge_evaluations == 0
        || l.maximum_permutation_edge_evaluations as u64 > 50_000_000_000
        || l.maximum_geometry_vertices == 0
        || l.maximum_geometry_vertices > 1_000_000
        || l.maximum_geometry_work == 0
        || l.maximum_geometry_work > 100_000_000
        || l.memory_budget_bytes == 0
        || l.memory_budget_bytes as u64 > 8_u64 * 1024 * 1024 * 1024
    {
        return Err(invalid("resource limits exceed admitted bounds"));
    }
    Ok(())
}

fn prepare_slide(
    slide: &Slide,
    classes: usize,
    edges: &[f64],
    limits: super::input::Limits,
) -> Result<Geometry> {
    let frame = CoordinateFrameId::new(&slide.coordinate_frame_id)
        .map_err(|error| invalid(error.to_string()))?;
    let declared = CoordinateFrame::new(
        frame.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let registry = CoordinateRegistry::new(vec![declared], vec![], vec![], vec![])
        .map_err(|error| invalid(error.to_string()))?;
    let parse = |value: &serde_json::Value| -> Result<ObservationWindow2D> {
        ObservationWindow2D::from_geojson_str(
            &serde_json::to_string(value)?,
            ObservationWindowLimits::new(
                MAXIMUM_INPUT_BYTES,
                1024,
                4096,
                limits.maximum_geometry_vertices.min(10_000),
                limits.maximum_geometry_work,
            )
            .map_err(|error| invalid(error.to_string()))?,
        )
        .and_then(|window| window.with_coordinate_frame(&registry, frame.clone()))
        .map_err(|error| invalid(error.to_string()))
    };
    let window = parse(&slide.window)?;
    let mut annotation_ids = BTreeSet::new();
    let mut annotations = Vec::with_capacity(slide.annotations.len());
    for annotation in &slide.annotations {
        if !text(&annotation.id)
            || !text(&annotation.provenance)
            || !annotation_ids.insert(&annotation.id)
            || !annotation.boundary_uncertainty_um.is_finite()
            || annotation.boundary_uncertainty_um < 0.0
            || edges
                .windows(2)
                .any(|band| band[1] - band[0] < 2.0 * annotation.boundary_uncertainty_um)
        {
            return Err(invalid("invalid annotation, duplicate identity or bands narrower than boundary uncertainty"));
        }
        annotations.push(parse(&annotation.geometry)?);
    }
    let mut ids = Vec::with_capacity(slide.cells.len());
    let mut source_ids = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    let mut probabilities = Vec::with_capacity(slide.cells.len() * classes);
    for cell in &slide.cells {
        let bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
        let values = &cell.phenotype_probabilities;
        if !text(&cell.id)
            || !text(&cell.stratum)
            || !source_ids.insert(&cell.id)
            || !window.contains(cell.x_um, cell.y_um)
            || !coordinates.insert((bits(cell.x_um), bits(cell.y_um)))
            || cell.marker.is_some_and(|value| !value.is_finite())
            || values.len() != classes
            || values
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || (values.iter().map(|value| f64::from(*value)).sum::<f64>() - 1.0).abs() > 1e-6
        {
            return Err(invalid(format!("slide {} has an invalid cell, duplicate identity/location, out-of-window point or simplex", slide.slide_id)));
        }
        ids.push(
            CellId::new(format!("{}::{}", slide.slide_id, cell.id))
                .map_err(|error| invalid(error.to_string()))?,
        );
        probabilities.extend_from_slice(values);
    }
    let index = SpatialIndex2D::from_points(slide.cells.iter().map(|cell| [cell.x_um, cell.y_um]))?;
    Ok(Geometry {
        window,
        annotations,
        index,
        ids,
        probabilities,
    })
}

fn increasing(values: &[f64], minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&values.len())
        && values.iter().all(|value| value.is_finite())
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
