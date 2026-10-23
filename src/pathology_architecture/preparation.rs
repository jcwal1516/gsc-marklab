use std::collections::BTreeSet;

use geo::{unary_union, Area, BooleanOps, MultiLineString, MultiPolygon, Polygon};
use rstar::{
    primitives::{GeomWithData, Rectangle},
    RTree, RTreeObject,
};

use crate::{
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    ObservationWindow2D, ObservationWindowLimits, Result, SpatialAxis,
};

use super::{
    geometry::{bounded, charge_overlay, parse_multilinestring, polygon_vertices, RingIndex},
    host_memory::{admitted_resources, ESTIMATE_BYTES_PER_INPUT_BYTE},
    input::{Recipe, Slide},
    invalid, UNCLASSIFIED,
};

const MAXIMUM_CLASSES: usize = 16;
const MAXIMUM_REGIONS: usize = 4096;
const CONTAINMENT_TOLERANCE: f64 = 1e-9;

pub(super) struct Prepared {
    pub recipe: Recipe,
    pub slides: Vec<SlideGeometry>,
    pub remaining_work: usize,
}

pub(super) struct SlideGeometry {
    pub window: MultiPolygon<f64>,
    pub window_area_um2: f64,
    pub epithelium: MultiPolygon<f64>,
    pub epithelial_area_um2: f64,
    /// Epithelium inside each declared class's regions, then outside every region.
    pub class_epithelium: Vec<MultiPolygon<f64>>,
    /// Architecture regions tagged by class index.
    pub regions: RingIndex,
    pub lumens: Option<MultiPolygon<f64>>,
    pub mucin: Option<MultiPolygon<f64>>,
    pub exclusions: Option<MultiPolygon<f64>>,
    pub declared_front: Option<MultiLineString<f64>>,
}

pub(super) fn prepare(bytes: &[u8]) -> Result<Prepared> {
    let admitted = admitted_resources();
    if bytes.len() as u64 > admitted.recipe_bytes {
        return Err(invalid(format!(
            "recipe has {} bytes; this host admits at most {} (1/{ESTIMATE_BYTES_PER_INPUT_BYTE} of its memory budget ceiling)",
            bytes.len(),
            admitted.recipe_bytes
        )));
    }
    let mut recipe: Recipe = serde_json::from_slice(bytes)?;
    validate_recipe(&recipe)?;
    let limits = recipe.limits;
    if limits.memory_budget_bytes as u64 > admitted.memory_budget_ceiling {
        return Err(invalid(format!(
            "memory_budget_bytes {} exceeds this host's ceiling {} (three quarters of physical memory, at least 8 GiB)",
            limits.memory_budget_bytes, admitted.memory_budget_ceiling
        )));
    }
    let mut remaining = limits.maximum_geometry_work;
    let total_cells = recipe
        .slides
        .iter()
        .try_fold(0usize, |sum, slide| sum.checked_add(slide.cells.len()))
        .ok_or_else(|| invalid("cell count overflow"))?;
    if total_cells > limits.maximum_cells {
        return Err(invalid("cell resource limit exceeded"));
    }
    if total_cells > 0 && recipe.cells.is_none() {
        return Err(invalid(
            "slides declare cells without top-level cells provenance",
        ));
    }
    let total_spots = recipe
        .slides
        .iter()
        .try_fold(0usize, |sum, slide| sum.checked_add(slide.spots.len()))
        .ok_or_else(|| invalid("spot count overflow"))?;
    if total_spots > limits.maximum_spots {
        return Err(invalid("spot resource limit exceeded"));
    }
    if total_spots > 0 && recipe.spots.is_none() {
        return Err(invalid(
            "slides declare spots without top-level spots provenance",
        ));
    }
    let estimate = bytes
        .len()
        .checked_mul(ESTIMATE_BYTES_PER_INPUT_BYTE as usize)
        .and_then(|value| value.checked_add(limits.maximum_geometry_vertices.checked_mul(256)?))
        .and_then(|value| value.checked_add(total_cells.checked_mul(512)?))
        .and_then(|value| value.checked_add(total_spots.checked_mul(1024)?))
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
    let mut slide_ids = BTreeSet::new();
    let mut frames = BTreeSet::new();
    let mut slides = Vec::with_capacity(recipe.slides.len());
    for slide in &mut recipe.slides {
        slide
            .architecture_regions
            .sort_by(|left, right| left.id.cmp(&right.id));
        slide.cells.sort_by(|left, right| left.id.cmp(&right.id));
        slide.spots.sort_by(|left, right| left.id.cmp(&right.id));
    }
    for slide in &recipe.slides {
        if !text(&slide.slide_id)
            || !text(&slide.patient_id)
            || !slide_ids.insert(slide.slide_id.clone())
            || !frames.insert(slide.coordinate_frame_id.clone())
        {
            return Err(invalid(
                "invalid or duplicate slide or coordinate-frame identity",
            ));
        }
        if slide.declared_front.is_some() && recipe.declared_front.is_none() {
            return Err(invalid(format!(
                "slide {} declares a front without top-level declared_front provenance",
                slide.slide_id
            )));
        }
        slides.push(prepare_slide(
            slide,
            &recipe,
            admitted.recipe_bytes as usize,
            &mut remaining,
        )?);
    }
    Ok(Prepared {
        recipe,
        slides,
        remaining_work: remaining,
    })
}

fn validate_recipe(recipe: &Recipe) -> Result<()> {
    let classes = &recipe.architecture.classes;
    let layers = std::iter::once((
        &recipe.epithelium.measurement_status,
        &recipe.epithelium.provenance,
    ))
    .chain(std::iter::once((
        &recipe.architecture.measurement_status,
        &recipe.architecture.provenance,
    )))
    .chain(
        recipe
            .declared_front
            .iter()
            .chain(&recipe.cells)
            .chain(&recipe.spots)
            .map(|layer| (&layer.measurement_status, &layer.provenance)),
    );
    if recipe.format != "marklab.pathology_architecture_recipe"
        || recipe.version != 1
        || !text(&recipe.study_id)
        || recipe.slides.is_empty()
        || recipe.slides.len() > 64
        || classes.is_empty()
        || classes.len() > MAXIMUM_CLASSES
        || classes
            .iter()
            .any(|name| !text(name) || name == UNCLASSIFIED)
        || classes.iter().collect::<BTreeSet<_>>().len() != classes.len()
    {
        return Err(invalid(
            "invalid recipe identity, slide count or architecture classes",
        ));
    }
    for (status, provenance) in layers {
        if crate::measurement_status_wire::parse(status).is_none() || !text(provenance) {
            return Err(invalid("invalid layer measurement status or provenance"));
        }
    }
    let d = &recipe.design;
    let positive = |value: f64| value.is_finite() && value > 0.0 && (value * value).is_finite();
    if !positive(d.closing_radius_um)
        || !(d.minimum_bulk_area_um2.is_finite() && d.minimum_bulk_area_um2 >= 0.0)
        || !positive(d.tissue_edge_tolerance_um)
        || !positive(d.contour_sample_spacing_um)
        || !(d.curvature_smoothing_um.is_finite() && d.curvature_smoothing_um >= 0.0)
        || !(2..=33).contains(&d.band_edges_um.len())
        || d.band_edges_um
            .iter()
            .any(|edge| !(edge * edge).is_finite())
        || d.band_edges_um.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "invalid closing radius, bulk area, edge tolerance, contour sampling or distance bands",
        ));
    }
    if let Some(budding) = &d.budding {
        if !(1..=64).contains(&budding.maximum_cells)
            || !(budding.hotspot_area_mm2.is_finite()
                && budding.hotspot_area_mm2 > 0.0
                && budding.hotspot_area_mm2 <= 100.0)
            || !(budding.mucin_exclusion_um.is_finite() && budding.mucin_exclusion_um >= 0.0)
            || recipe.cells.is_none()
        {
            return Err(invalid(
                "budding needs 1..=64 cells, a hotspot area in (0, 100] mm2, a nonnegative mucin exclusion and a declared cells layer",
            ));
        }
    }
    if let Some(branching) = &d.branching {
        if !positive(branching.pixel_size_um)
            || !(branching.minimum_branch_length_um.is_finite()
                && branching.minimum_branch_length_um >= 0.0)
        {
            return Err(invalid(
                "invalid branching pixel size or minimum branch length",
            ));
        }
    }
    let l = recipe.limits;
    if l.maximum_geometry_vertices == 0
        || l.maximum_geometry_vertices > 20_000_000
        || l.maximum_components == 0
        || l.maximum_components > 2_000_000
        || l.maximum_geometry_work == 0
        || l.maximum_geometry_work as u64 > 200_000_000_000
        || l.maximum_cells == 0
        || l.maximum_cells > 5_000_000
        || l.maximum_spots == 0
        || l.maximum_spots > 2_000_000
        || l.maximum_raster_pixels == 0
        || l.maximum_raster_pixels as u64 > 4_000_000_000
        || l.memory_budget_bytes == 0
    {
        return Err(invalid("resource limits exceed admitted bounds"));
    }
    Ok(())
}

fn prepare_slide(
    slide: &Slide,
    recipe: &Recipe,
    maximum_layer_bytes: usize,
    remaining: &mut usize,
) -> Result<SlideGeometry> {
    let limits = recipe.limits;
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
    let window_limits = ObservationWindowLimits::new(
        maximum_layer_bytes,
        limits.maximum_components,
        limits.maximum_components.saturating_mul(16),
        limits.maximum_geometry_vertices,
        limits.maximum_geometry_work,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let parse = |value: &serde_json::Value, layer: &str| -> Result<MultiPolygon<f64>> {
        ObservationWindow2D::from_geojson_str(&serde_json::to_string(value)?, window_limits)
            .and_then(|window| window.with_coordinate_frame(&registry, frame.clone()))
            .map(|window| window.geo_multipolygon().clone())
            .map_err(|error| invalid(format!("slide {} {layer}: {error}", slide.slide_id)))
    };
    let edges = &recipe.design.band_edges_um;
    let uncertainty = slide.boundary_uncertainty_um;
    if !uncertainty.is_finite()
        || uncertainty < 0.0
        || edges
            .windows(2)
            .any(|band| band[1] - band[0] < 2.0 * uncertainty)
        || slide.architecture_regions.len() > MAXIMUM_REGIONS
    {
        return Err(invalid(format!(
            "slide {} has invalid boundary uncertainty, bands narrower than twice it, or too many regions",
            slide.slide_id
        )));
    }
    let window = parse(&slide.window, "window")?;
    let window_area_um2 = window.unsigned_area();
    let tolerance = CONTAINMENT_TOLERANCE * window_area_um2;
    let epithelium = parse(&slide.epithelium, "epithelium")?;
    let epithelial_area_um2 = epithelium.unsigned_area();
    let inside = |layer: &MultiPolygon<f64>, name: &str, remaining: &mut usize| {
        contained(layer, &window, tolerance, remaining, &slide.slide_id, name)
    };
    inside(&epithelium, "epithelium", remaining)?;
    let classes = &recipe.architecture.classes;
    let mut region_ids = BTreeSet::new();
    let mut class_regions = vec![Vec::new(); classes.len()];
    for region in &slide.architecture_regions {
        let class = classes
            .iter()
            .position(|name| *name == region.class)
            .ok_or_else(|| invalid(format!("region {} has an undeclared class", region.id)))?;
        if !text(&region.id) || !region_ids.insert(&region.id) {
            return Err(invalid("invalid or duplicate architecture region identity"));
        }
        let geometry = parse(&region.geometry, "architecture region")?;
        inside(&geometry, "architecture region", remaining)?;
        class_regions[class].extend(geometry.0);
    }
    let class_regions = class_regions
        .into_iter()
        .map(MultiPolygon::new)
        .collect::<Vec<_>>();
    let all_regions = class_regions
        .iter()
        .flat_map(|regions| regions.0.iter().cloned())
        .collect::<Vec<_>>();
    let region_vertices = class_regions.iter().map(polygon_vertices).sum::<usize>();
    charge_overlay(remaining, region_vertices, "region union")?;
    let region_union = unary_union(&all_regions);
    if region_overlap_area(&all_regions, remaining)? > tolerance {
        return Err(invalid(format!(
            "slide {} architecture regions overlap",
            slide.slide_id
        )));
    }
    let mut class_epithelium = Vec::with_capacity(classes.len() + 1);
    for regions in &class_regions {
        charge_overlay(
            remaining,
            polygon_vertices(regions) + polygon_vertices(&epithelium),
            "class epithelium",
        )?;
        class_epithelium.push(bounded(
            epithelium.intersection(regions),
            limits.maximum_geometry_vertices,
            "class epithelium",
        )?);
    }
    charge_overlay(
        remaining,
        region_vertices + polygon_vertices(&epithelium),
        "unclassified epithelium",
    )?;
    class_epithelium.push(bounded(
        epithelium.difference(&region_union),
        limits.maximum_geometry_vertices,
        "unclassified epithelium",
    )?);
    let regions = RingIndex::from_lines(class_regions.iter().enumerate().flat_map(
        |(class, regions)| {
            regions.0.iter().flat_map(move |polygon| {
                std::iter::once(polygon.exterior())
                    .chain(polygon.interiors())
                    .map(move |ring| (class as u32, ring))
            })
        },
    ));
    let optional = |value: &Option<serde_json::Value>,
                    name: &str,
                    disjoint: bool,
                    remaining: &mut usize|
     -> Result<Option<MultiPolygon<f64>>> {
        let Some(value) = value else { return Ok(None) };
        let geometry = parse(value, name)?;
        inside(&geometry, name, remaining)?;
        if disjoint {
            charge_overlay(
                remaining,
                polygon_vertices(&geometry) + polygon_vertices(&epithelium),
                "epithelium overlap",
            )?;
            if geometry.intersection(&epithelium).unsigned_area() > tolerance {
                return Err(invalid(format!(
                    "slide {} {name} overlaps epithelium",
                    slide.slide_id
                )));
            }
        }
        Ok(Some(geometry))
    };
    let lumens = optional(&slide.lumens, "lumens", true, remaining)?;
    let mucin = optional(&slide.mucin, "mucin", true, remaining)?;
    let exclusions = optional(
        &slide.front_exclusions,
        "front exclusions",
        false,
        remaining,
    )?;
    let declared_front = slide
        .declared_front
        .as_ref()
        .map(|value| parse_multilinestring(value, limits.maximum_geometry_vertices))
        .transpose()?;
    let window_index = RingIndex::from_polygons(&window, |_| 0);
    validate_cells(slide, &window_index)?;
    let mut spot_ids = BTreeSet::new();
    if slide.spots.iter().any(|spot| {
        !text(&spot.id)
            || !spot_ids.insert(&spot.id)
            || !spot.x_um.is_finite()
            || !spot.y_um.is_finite()
            || !(spot.diameter_um.is_finite()
                && spot.diameter_um > 0.0
                && spot.diameter_um <= 10_000.0)
    }) {
        return Err(invalid(format!(
            "slide {} has an invalid or duplicate spot or spot diameter",
            slide.slide_id
        )));
    }
    if let Some(front) = &declared_front {
        let index = &window_index;
        if front
            .0
            .iter()
            .flat_map(|line| &line.0)
            .any(|point| index.locate([point.x, point.y]).is_none())
        {
            return Err(invalid(format!(
                "slide {} declared front leaves the tissue window",
                slide.slide_id
            )));
        }
    }
    Ok(SlideGeometry {
        window,
        window_area_um2,
        epithelium,
        epithelial_area_um2,
        class_epithelium,
        regions,
        lumens,
        mucin,
        exclusions,
        declared_front,
    })
}

/// Summed pairwise intersection area of regions whose bounding boxes meet. Overlap is
/// measured directly: the area of a union of many regions differs from their summed area
/// by overlay rounding that grows with the total boundary, which exceeded the tolerance
/// for disjoint regions.
fn region_overlap_area(polygons: &[Polygon<f64>], remaining: &mut usize) -> Result<f64> {
    let boxes = polygons
        .iter()
        .enumerate()
        .map(|(index, polygon)| {
            let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for point in &polygon.exterior().0 {
                low = [low[0].min(point.x), low[1].min(point.y)];
                high = [high[0].max(point.x), high[1].max(point.y)];
            }
            GeomWithData::new(Rectangle::from_corners(low, high), index)
        })
        .collect::<Vec<_>>();
    let vertices = polygons
        .iter()
        .map(|polygon| {
            std::iter::once(polygon.exterior())
                .chain(polygon.interiors())
                .map(|ring| ring.0.len())
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    let tree = RTree::bulk_load(boxes.clone());
    let mut overlap = 0.0;
    for item in &boxes {
        for other in tree.locate_in_envelope_intersecting(item.envelope()) {
            if other.data <= item.data {
                continue;
            }
            charge_overlay(
                remaining,
                vertices[item.data] + vertices[other.data],
                "region overlap",
            )?;
            overlap += polygons[item.data]
                .intersection(&polygons[other.data])
                .unsigned_area();
        }
    }
    Ok(overlap)
}

fn validate_cells(slide: &Slide, window: &RingIndex) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut locations = BTreeSet::new();
    let bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
    for cell in &slide.cells {
        let nucleus_valid = cell.nucleus.is_none_or(|n| {
            [
                n.area_um2,
                n.perimeter_um,
                n.convex_area_um2,
                n.major_axis_um,
                n.minor_axis_um,
            ]
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
                && n.minor_axis_um <= n.major_axis_um
                && n.area_um2 <= n.convex_area_um2 * (1.0 + 1e-9)
                && n.major_axis_angle_rad.is_finite()
                && n.major_axis_angle_rad.abs() <= std::f64::consts::PI
        });
        if !text(&cell.id)
            || !ids.insert(&cell.id)
            || !cell.x_um.is_finite()
            || !cell.y_um.is_finite()
            || window.locate([cell.x_um, cell.y_um]).is_none()
            || !locations.insert((bits(cell.x_um), bits(cell.y_um)))
            || !nucleus_valid
        {
            return Err(invalid(format!(
                "slide {} has an invalid or duplicate cell, an out-of-window cell or an invalid nucleus",
                slide.slide_id
            )));
        }
    }
    Ok(())
}

fn contained(
    layer: &MultiPolygon<f64>,
    window: &MultiPolygon<f64>,
    tolerance: f64,
    remaining: &mut usize,
    slide_id: &str,
    name: &str,
) -> Result<()> {
    charge_overlay(
        remaining,
        polygon_vertices(layer) + polygon_vertices(window),
        "layer containment",
    )?;
    if layer.difference(window).unsigned_area() > tolerance {
        return Err(invalid(format!(
            "slide {slide_id} {name} extends outside the tissue window"
        )));
    }
    Ok(())
}

pub(super) fn text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
