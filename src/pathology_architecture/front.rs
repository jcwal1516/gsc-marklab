use geo::{unary_union, Area, BooleanOps, MultiLineString, MultiPolygon, Polygon};
use marklab_numerics::summation::neumaier_sum;

use crate::{common::stats::percentile_nearest_rank_sorted, Result};

use super::{
    geometry::{
        bounded, buffered, charge_overlay, line_length, line_vertices, polygon_vertices,
        sample_lines, Point, RingIndex,
    },
    input::{Design, Limits},
    invalid,
    preparation::SlideGeometry,
    result::{DistanceSummary, FrontAgreement},
};

pub(super) const AVAILABLE: &str = "available";
const NO_BULK: &str = "unavailable_no_tumor_bulk_at_minimum_area";
const NO_FRONT: &str = "unavailable_no_front_after_tissue_edge_and_exclusion_clipping";

/// Closed, hole-filled tumor bulk and its advancing-front lines.
pub(super) struct Front {
    pub status: &'static str,
    pub bulk: MultiPolygon<f64>,
    bulk_index: RingIndex,
    pub lines: MultiLineString<f64>,
    segments: RingIndex,
    pub length_um: f64,
}

impl Front {
    pub(super) fn available(&self) -> bool {
        self.status == AVAILABLE
    }

    /// Distance to the nearest front segment, negative inside the tumor bulk.
    pub(super) fn signed_distance_um(&self, point: Point) -> Option<f64> {
        let (distance, _) = self.segments.nearest(point)?;
        if distance == 0.0 {
            return Some(0.0);
        }
        Some(if self.bulk_index.locate(point).is_some() {
            -distance
        } else {
            distance
        })
    }

    /// Whether a point lies in the closed tumor bulk.
    pub(super) fn in_bulk(&self, point: Point) -> bool {
        self.bulk_index.locate(point).is_some()
    }

    /// Direction of the nearest front segment, counterclockwise from +x.
    pub(super) fn nearest_direction_rad(&self, point: Point) -> Option<f64> {
        let (_, segment) = self.segments.nearest(point)?;
        Some((segment.end[1] - segment.start[1]).atan2(segment.end[0] - segment.start[0]))
    }

    /// Front segments for exact hotspot placement.
    pub(super) fn segments(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.lines
            .0
            .iter()
            .flat_map(|line| line.0.windows(2))
            .filter(|pair| pair[0] != pair[1])
            .map(|pair| ([pair[0].x, pair[0].y], [pair[1].x, pair[1].y]))
    }

    fn unavailable(status: &'static str, bulk: MultiPolygon<f64>) -> Self {
        let bulk_index = RingIndex::from_polygons(&bulk, |_| 0);
        Self {
            status,
            bulk,
            bulk_index,
            lines: MultiLineString::new(vec![]),
            segments: RingIndex::from_lines(std::iter::empty()),
            length_um: 0.0,
        }
    }
}

/// Close tumor tissue at the declared radius, fill holes, clip to tissue and keep
/// bulk components of at least the declared area. The front is the bulk boundary
/// farther than the edge tolerance from the tissue edge and outside declared exclusions.
pub(super) fn derive(
    slide: &SlideGeometry,
    design: &Design,
    limits: &Limits,
    remaining: &mut usize,
) -> Result<Front> {
    let maximum = limits.maximum_geometry_vertices;
    let extras = slide
        .lumens
        .iter()
        .chain(&slide.mucin)
        .flat_map(|layer| layer.0.iter().cloned())
        .collect::<Vec<_>>();
    let tumor = if extras.is_empty() {
        slide.epithelium.clone()
    } else {
        let extras = MultiPolygon::new(extras);
        charge_overlay(
            remaining,
            polygon_vertices(&slide.epithelium) + polygon_vertices(&extras),
            "tumor tissue union",
        )?;
        bounded(
            slide.epithelium.union(&extras),
            maximum,
            "tumor tissue union",
        )?
    };
    let radius = design.closing_radius_um;
    let dilated = buffered(
        &tumor,
        radius,
        polygon_vertices(&tumor),
        maximum,
        remaining,
        "tumor dilation",
    )?;
    let closed = buffered(
        &dilated,
        -radius,
        polygon_vertices(&dilated),
        maximum,
        remaining,
        "tumor erosion",
    )?;
    let filled = closed
        .0
        .iter()
        .map(|polygon| Polygon::new(polygon.exterior().clone(), vec![]))
        .collect::<Vec<_>>();
    charge_overlay(remaining, polygon_vertices(&closed), "bulk hole filling")?;
    let filled = unary_union(&filled);
    charge_overlay(
        remaining,
        polygon_vertices(&filled) + polygon_vertices(&slide.window),
        "bulk tissue clipping",
    )?;
    let clipped = bounded(filled.intersection(&slide.window), maximum, "tumor bulk")?;
    let bulk = MultiPolygon::new(
        clipped
            .0
            .into_iter()
            .filter(|polygon| {
                let area = polygon.unsigned_area();
                area > 0.0 && area >= design.minimum_bulk_area_um2
            })
            .collect(),
    );
    if bulk.0.is_empty() {
        return Ok(Front::unavailable(NO_BULK, bulk));
    }
    let rings = MultiLineString::new(
        bulk.0
            .iter()
            .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
            .cloned()
            .collect(),
    );
    let inner = buffered(
        &slide.window,
        -design.tissue_edge_tolerance_um,
        polygon_vertices(&slide.window),
        maximum,
        remaining,
        "tissue edge tolerance",
    )?;
    charge_overlay(
        remaining,
        line_vertices(&rings) + polygon_vertices(&inner),
        "front edge clipping",
    )?;
    let mut lines = inner.clip(&rings, false);
    if let Some(exclusions) = &slide.exclusions {
        charge_overlay(
            remaining,
            line_vertices(&lines) + polygon_vertices(exclusions),
            "front exclusion clipping",
        )?;
        lines = exclusions.clip(&lines, true);
    }
    lines
        .0
        .retain(|line| line.0.len() >= 2 && line_length(line) > 0.0);
    lines.0.sort_by(|left, right| {
        let key = |line: &geo::LineString<f64>| (line.0[0].x, line.0[0].y, line.0.len());
        let (a, b) = (key(left), key(right));
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    if lines.0.is_empty() {
        return Ok(Front::unavailable(NO_FRONT, bulk));
    }
    if line_vertices(&lines) > maximum {
        return Err(invalid("front vertex resource limit exceeded"));
    }
    let length_um = neumaier_sum(lines.0.iter().map(line_length));
    let segments = RingIndex::from_lines(
        lines
            .0
            .iter()
            .enumerate()
            .map(|(index, line)| (index as u32, line)),
    );
    let bulk_index = RingIndex::from_polygons(&bulk, |_| 0);
    Ok(Front {
        status: AVAILABLE,
        bulk,
        bulk_index,
        lines,
        segments,
        length_um,
    })
}

/// Symmetric sampled distances between the derived and a declared front.
pub(super) fn agreement(
    front: &Front,
    declared: &MultiLineString<f64>,
    spacing_um: f64,
    remaining: &mut usize,
) -> Result<Option<FrontAgreement>> {
    let declared_length_um = neumaier_sum(declared.0.iter().map(line_length));
    if !front.available() {
        return Ok(None);
    }
    let declared_index = RingIndex::from_lines(declared.0.iter().map(|line| (0, line)));
    let derived_samples = sample_lines(&front.lines, spacing_um);
    let declared_samples = sample_lines(declared, spacing_um);
    charge_overlay(
        remaining,
        derived_samples.len() + declared_samples.len(),
        "front agreement",
    )?;
    let summarize = |samples: &[Point], target: &RingIndex| -> Result<DistanceSummary> {
        let mut distances = samples
            .iter()
            .map(|point| target.nearest(*point).map(|(distance, _)| distance))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid("front agreement target has no segments"))?;
        distances.sort_by(f64::total_cmp);
        Ok(DistanceSummary {
            samples: distances.len(),
            mean_um: neumaier_sum(distances.iter().copied()) / distances.len() as f64,
            p95_um: percentile_nearest_rank_sorted(&distances, 0.95)
                .ok_or_else(|| invalid("front agreement has no finite samples"))?,
            maximum_um: *distances.last().expect("samples are nonempty"),
        })
    };
    Ok(Some(FrontAgreement {
        declared_length_um,
        sample_spacing_um: spacing_um,
        derived_to_declared: summarize(&derived_samples, &declared_index)?,
        declared_to_derived: summarize(&declared_samples, &front.segments)?,
    }))
}
