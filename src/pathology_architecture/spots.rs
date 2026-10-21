use geo::{Area, BooleanOps, Buffer, MultiPolygon, Polygon};
use rstar::{
    primitives::{GeomWithData, Rectangle},
    RTree, AABB,
};

use crate::Result;

use super::{
    components::band_index,
    front::Front,
    geometry::{charge_overlay, polygon_vertices, round_style, Point, RingIndex},
    input::Spot,
    preparation::SlideGeometry,
    result::SpotRow,
};

/// Polygons with bounding-box and boundary indexes for local disk overlaps.
struct Coverage<'a> {
    polygons: &'a [Polygon<f64>],
    boxes: RTree<GeomWithData<Rectangle<Point>, usize>>,
    rings: RingIndex,
}

impl<'a> Coverage<'a> {
    fn new(geometry: &'a MultiPolygon<f64>) -> Self {
        let boxes = geometry
            .0
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
            .collect();
        Self {
            polygons: &geometry.0,
            boxes: RTree::bulk_load(boxes),
            rings: RingIndex::from_polygons(geometry, |_| 0),
        }
    }

    /// Covered area of the polygonal disk. A disk farther than its radius from every
    /// boundary is wholly inside or outside; only boundary-crossing disks are overlaid.
    fn overlap(
        &self,
        center: Point,
        radius: f64,
        disk: &MultiPolygon<f64>,
        disk_area: f64,
        remaining: &mut usize,
    ) -> Result<f64> {
        let Some((distance, _)) = self.rings.nearest(center) else {
            return Ok(0.0);
        };
        if distance >= radius {
            return Ok(if self.rings.locate(center).is_some() {
                disk_area
            } else {
                0.0
            });
        }
        let envelope = AABB::from_corners(
            [center[0] - radius, center[1] - radius],
            [center[0] + radius, center[1] + radius],
        );
        let local = MultiPolygon::new(
            self.boxes
                .locate_in_envelope_intersecting(envelope)
                .map(|entry| self.polygons[entry.data].clone())
                .collect(),
        );
        charge_overlay(
            remaining,
            polygon_vertices(&local) + polygon_vertices(disk),
            "spot overlap",
        )?;
        Ok(local.intersection(disk).unsigned_area().min(disk_area))
    }
}

/// Tissue, tumor-bulk and per-class epithelial cover of each spot disk, with the
/// signed front distance and band of its centre.
pub(super) fn attach(
    spots: &[Spot],
    slide: &SlideGeometry,
    front: &Front,
    class_order: &[String],
    edges: &[f64],
    remaining: &mut usize,
) -> Result<Vec<SpotRow>> {
    let window = Coverage::new(&slide.window);
    let classes = slide
        .class_epithelium
        .iter()
        .map(Coverage::new)
        .collect::<Vec<_>>();
    let bulk = (!front.bulk.0.is_empty()).then(|| Coverage::new(&front.bulk));
    let mut rows = Vec::with_capacity(spots.len());
    for spot in spots {
        let center = [spot.x_um, spot.y_um];
        let radius = spot.diameter_um / 2.0;
        let disk = geo::Point::new(center[0], center[1]).buffer_with_style(round_style(radius));
        let disk_area = disk.unsigned_area();
        let fraction = |coverage: &Coverage<'_>, remaining: &mut usize| -> Result<f64> {
            Ok(coverage.overlap(center, radius, &disk, disk_area, remaining)? / disk_area)
        };
        let class_fractions = classes
            .iter()
            .map(|coverage| fraction(coverage, remaining))
            .collect::<Result<Vec<_>>>()?;
        let dominant = class_fractions
            .iter()
            .enumerate()
            .filter(|(_, share)| **share > 0.0)
            .fold(
                None,
                |best: Option<(usize, f64)>, (index, share)| match best {
                    Some((_, top)) if top >= *share => best,
                    _ => Some((index, *share)),
                },
            );
        let signed_front_distance_um = front.signed_distance_um(center);
        rows.push(SpotRow {
            id: spot.id.clone(),
            x_um: spot.x_um,
            y_um: spot.y_um,
            diameter_um: spot.diameter_um,
            signed_front_distance_um,
            band: signed_front_distance_um.and_then(|distance| band_index(distance, edges)),
            tissue_fraction: fraction(&window, remaining)?,
            tumor_bulk_fraction: bulk
                .as_ref()
                .map(|coverage| fraction(coverage, remaining))
                .transpose()?,
            epithelial_fraction: class_fractions.iter().sum(),
            dominant_class: dominant.map(|(index, _)| class_order[index].clone()),
            class_fractions,
        });
    }
    Ok(rows)
}
