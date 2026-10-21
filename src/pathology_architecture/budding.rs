use geo::{Area, BooleanOps, Buffer};
use rstar::{RTree, AABB};

use crate::Result;

use super::{
    charge,
    front::Front,
    geometry::{charge_overlay, polygon_vertices, round_style, segment_distance_2, Point, RingIndex},
    input::BuddingDesign,
    preparation::SlideGeometry,
    result::{BuddingResult, ComponentRow, Hotspot},
};

const DEFINITION: &str = "epithelial_component_with_1_to_maximum_cells_epithelial_nuclei; \
    mucin_associated_components_excluded; hotspot_field_centered_on_the_front";
const ITBCC_FIELD_MM2: f64 = 0.785;

/// Mark tumor buds and find the front-centred field containing the most buds.
pub(super) fn analyze(
    design: &BuddingDesign,
    slide: &SlideGeometry,
    cells_present: bool,
    front: &Front,
    components: &mut [ComponentRow],
    remaining: &mut usize,
) -> Result<BuddingResult> {
    if !cells_present {
        return Ok(BuddingResult {
            status: "unavailable_no_cells",
            definition: DEFINITION,
            bud_count: 0,
            buds_inside_bulk: 0,
            buds_outside_bulk: 0,
            mucin_excluded_candidates: 0,
            hotspot: None,
        });
    }
    let mucin = slide
        .mucin
        .as_ref()
        .map(|mucin| RingIndex::from_polygons(mucin, |_| 0));
    let mut excluded = 0;
    for (row, polygon) in components.iter_mut().zip(&slide.epithelium.0) {
        let count = row.epithelial_cell_count.unwrap_or(0);
        let candidate = (1..=design.maximum_cells).contains(&count);
        let mut mucin_associated = false;
        if let (true, Some(index)) = (candidate, &mucin) {
            charge(remaining, polygon.exterior().0.len(), "bud mucin exclusion")?;
            mucin_associated = index.locate([row.x_um, row.y_um]).is_some()
                || polygon.exterior().0.iter().any(|vertex| {
                    index
                        .nearest([vertex.x, vertex.y])
                        .is_some_and(|(distance, _)| distance <= design.mucin_exclusion_um)
                });
        }
        if mucin_associated {
            excluded += 1;
        }
        row.bud = Some(candidate && !mucin_associated);
    }
    let buds = components
        .iter()
        .filter(|row| row.bud == Some(true))
        .map(|row| [row.x_um, row.y_um])
        .collect::<Vec<_>>();
    let inside = buds.iter().filter(|point| front.in_bulk(**point)).count();
    let hotspot = front
        .available()
        .then(|| hotspot(design, slide, front, &buds, remaining))
        .transpose()?;
    Ok(BuddingResult {
        status: if front.available() {
            "available"
        } else {
            "available_without_front_hotspot"
        },
        definition: DEFINITION,
        bud_count: buds.len(),
        buds_inside_bulk: inside,
        buds_outside_bulk: buds.len() - inside,
        mucin_excluded_candidates: excluded,
        hotspot,
    })
}

/// Exact maximum over field centres on the front: each bud contributes the closed
/// parameter interval of a segment within the field radius, and a sweep finds the
/// largest overlap. Ties keep the first segment and smallest parameter.
fn hotspot(
    design: &BuddingDesign,
    slide: &SlideGeometry,
    front: &Front,
    buds: &[Point],
    remaining: &mut usize,
) -> Result<Hotspot> {
    let radius = (design.hotspot_area_mm2 * 1e6 / std::f64::consts::PI).sqrt();
    let tree = RTree::bulk_load(buds.to_vec());
    let mut best: (usize, Option<Point>) = (0, None);
    for (start, end) in front.segments() {
        let envelope = AABB::from_corners(
            [start[0].min(end[0]) - radius, start[1].min(end[1]) - radius],
            [start[0].max(end[0]) + radius, start[1].max(end[1]) + radius],
        );
        let mut events = Vec::new();
        for bud in tree.locate_in_envelope(envelope) {
            if segment_distance_2(start, end, *bud) > radius * radius {
                continue;
            }
            let direction = [end[0] - start[0], end[1] - start[1]];
            let offset = [start[0] - bud[0], start[1] - bud[1]];
            let a = direction[0] * direction[0] + direction[1] * direction[1];
            let b = 2.0 * (offset[0] * direction[0] + offset[1] * direction[1]);
            let c = offset[0] * offset[0] + offset[1] * offset[1] - radius * radius;
            let root = (b * b - 4.0 * a * c).max(0.0).sqrt();
            let lower = ((-b - root) / (2.0 * a)).max(0.0);
            let upper = ((-b + root) / (2.0 * a)).min(1.0);
            if lower <= upper {
                events.push((lower, 1_i64));
                events.push((upper, -1_i64));
            }
        }
        charge(remaining, events.len().max(1), "bud hotspot sweep")?;
        events.sort_by(|left, right| left.0.total_cmp(&right.0).then(right.1.cmp(&left.1)));
        let mut running = 0_i64;
        for (t, delta) in events {
            running += delta;
            if running > best.0 as i64 {
                best = (
                    running as usize,
                    Some([
                        start[0] + t * (end[0] - start[0]),
                        start[1] + t * (end[1] - start[1]),
                    ]),
                );
            }
        }
    }
    let field_tissue_fraction = match best.1 {
        Some(center) => {
            let field = geo::Point::new(center[0], center[1]).buffer_with_style(round_style(radius));
            charge_overlay(
                remaining,
                polygon_vertices(&field) + polygon_vertices(&slide.window),
                "hotspot field clipping",
            )?;
            Some(field.intersection(&slide.window).unsigned_area() / field.unsigned_area())
        }
        None => None,
    };
    let normalized = best.0 as f64 * ITBCC_FIELD_MM2 / design.hotspot_area_mm2;
    Ok(Hotspot {
        center_um: best.1,
        radius_um: radius,
        area_mm2: design.hotspot_area_mm2,
        bud_count: best.0,
        bud_count_per_0_785_mm2: normalized,
        grade: if normalized < 5.0 {
            "Bd1"
        } else if normalized < 10.0 {
            "Bd2"
        } else {
            "Bd3"
        },
        field_tissue_fraction,
    })
}
