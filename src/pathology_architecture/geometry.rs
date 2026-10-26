use geo::{
    algorithm::buffer::{BufferStyle, LineCap, LineJoin},
    BoundingRect, Buffer, Coord, LineString, MultiLineString, MultiPolygon,
};
use rstar::{PointDistance, RTree, RTreeObject, AABB};

use crate::{common::finite::canonical_zero, geom::window::PROFILE_BUFFER_ANGLE, Result};

use super::{charge, invalid};

pub(super) type Point = [f64; 2];

/// One tagged boundary or front segment.
#[derive(Clone, Copy, Debug)]
pub(super) struct Segment {
    pub start: Point,
    pub end: Point,
    pub tag: u32,
}

impl RTreeObject for Segment {
    type Envelope = AABB<Point>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [
                self.start[0].min(self.end[0]),
                self.start[1].min(self.end[1]),
            ],
            [
                self.start[0].max(self.end[0]),
                self.start[1].max(self.end[1]),
            ],
        )
    }
}

impl PointDistance for Segment {
    fn distance_2(&self, point: &Point) -> f64 {
        segment_distance_2(self.start, self.end, *point)
    }
}

pub(super) fn segment_distance_2(start: Point, end: Point, point: Point) -> f64 {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length_2 = direction[0] * direction[0] + direction[1] * direction[1];
    let t = if length_2 > 0.0 {
        (((point[0] - start[0]) * direction[0] + (point[1] - start[1]) * direction[1]) / length_2)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = point[0] - (start[0] + t * direction[0]);
    let dy = point[1] - (start[1] + t * direction[1]);
    dx * dx + dy * dy
}

/// Indexed rings answering closed point-in-polygon and nearest-boundary queries.
///
/// Rings sharing a tag form one polygon set; even-odd parity per tag resolves holes.
/// Points on any boundary belong to the tag of that boundary.
pub(super) struct RingIndex {
    tree: RTree<Segment>,
    maximum_x: f64,
}

impl RingIndex {
    pub(super) fn from_lines<'a>(
        lines: impl IntoIterator<Item = (u32, &'a LineString<f64>)>,
    ) -> Self {
        let segments = lines
            .into_iter()
            .flat_map(|(tag, line)| {
                line.0
                    .windows(2)
                    .filter(|pair| pair[0] != pair[1])
                    .map(move |pair| Segment {
                        start: [pair[0].x, pair[0].y],
                        end: [pair[1].x, pair[1].y],
                        tag,
                    })
            })
            .collect::<Vec<_>>();
        let maximum_x = segments
            .iter()
            .map(|segment| segment.start[0].max(segment.end[0]))
            .fold(f64::NEG_INFINITY, f64::max);
        Self {
            tree: RTree::bulk_load(segments),
            maximum_x,
        }
    }

    pub(super) fn from_polygons(polygons: &MultiPolygon<f64>, tag: impl Fn(usize) -> u32) -> Self {
        Self::from_lines(polygons.0.iter().enumerate().flat_map(|(index, polygon)| {
            let tag = tag(index);
            std::iter::once(polygon.exterior())
                .chain(polygon.interiors())
                .map(move |ring| (tag, ring))
        }))
    }

    pub(super) fn nearest(&self, point: Point) -> Option<(f64, Segment)> {
        self.tree
            .nearest_neighbor(point)
            .map(|segment| (segment.distance_2(&point).sqrt(), *segment))
    }

    /// Every tag whose closed polygon set contains the point, in ascending order.
    pub(super) fn locate_all(&self, point: Point) -> Vec<u32> {
        let point = [canonical_zero(point[0]), canonical_zero(point[1])];
        let mut parity: Vec<(u32, bool)> = Vec::new();
        if self.tree.size() > 0 && point[0] <= self.maximum_x {
            let ray = AABB::from_corners(point, [self.maximum_x, point[1]]);
            for segment in self.tree.locate_in_envelope_intersecting(ray) {
                let [a, b] = [segment.start, segment.end];
                if (a[1] > point[1]) == (b[1] > point[1]) {
                    continue;
                }
                let crossing = a[0] + (point[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                if crossing > point[0] {
                    match parity.iter_mut().find(|(tag, _)| *tag == segment.tag) {
                        Some((_, odd)) => *odd = !*odd,
                        None => parity.push((segment.tag, true)),
                    }
                }
            }
        }
        let mut tags = parity
            .into_iter()
            .filter_map(|(tag, odd)| odd.then_some(tag))
            .collect::<Vec<_>>();
        if let Some((0.0, segment)) = self.nearest(point) {
            tags.push(segment.tag);
        }
        tags.sort_unstable();
        tags.dedup();
        tags
    }

    pub(super) fn locate(&self, point: Point) -> Option<u32> {
        self.locate_all(point).first().copied()
    }
}

pub(super) fn round_style(distance_um: f64) -> BufferStyle<f64> {
    BufferStyle::new(distance_um)
        .line_join(LineJoin::Round(PROFILE_BUFFER_ANGLE))
        .line_cap(LineCap::Round(PROFILE_BUFFER_ANGLE))
}

pub(super) fn polygon_vertices(geometry: &MultiPolygon<f64>) -> usize {
    geometry
        .0
        .iter()
        .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
        .map(|ring| ring.0.len())
        .sum()
}

pub(super) fn line_vertices(lines: &MultiLineString<f64>) -> usize {
    lines.0.iter().map(|line| line.0.len()).sum()
}

/// Charge an `n log n` overlay estimate for the summed input vertices.
pub(super) fn charge_overlay(
    remaining: &mut usize,
    vertices: usize,
    description: &str,
) -> Result<()> {
    let log = (usize::BITS - vertices.saturating_add(2).leading_zeros()) as usize;
    charge(remaining, vertices.saturating_mul(log).max(1), description)
}

/// Reject derived geometry that exceeds the declared vertex ceiling.
pub(super) fn bounded(
    geometry: MultiPolygon<f64>,
    maximum_vertices: usize,
    description: &str,
) -> Result<MultiPolygon<f64>> {
    if polygon_vertices(&geometry) > maximum_vertices {
        return Err(invalid(format!(
            "{description} vertex resource limit exceeded"
        )));
    }
    Ok(geometry)
}

pub(super) fn buffered(
    geometry: &impl Buffer<Scalar = f64>,
    distance_um: f64,
    input_vertices: usize,
    maximum_vertices: usize,
    remaining: &mut usize,
    description: &str,
) -> Result<MultiPolygon<f64>> {
    charge_overlay(remaining, input_vertices, description)?;
    bounded(
        geometry.buffer_with_style(round_style(distance_um)),
        maximum_vertices,
        description,
    )
}

/// Bound on the area change one boolean operation makes by rounding. geo overlays through
/// i_overlay, whose float adapter snaps every coordinate to a grid of step `2^(log2(h) - 29)`
/// (exponent truncated), `h` the larger half side of the operands' joint bounding box. Moving each
/// point of the result's boundary by at most a step changes its area by at most the boundary
/// length times the step; the exponent is rounded up and the bound doubled as a margin.
pub(super) fn overlay_area_error(
    subject: &MultiPolygon<f64>,
    clip: &MultiPolygon<f64>,
    result: &MultiPolygon<f64>,
) -> f64 {
    let (Some(first), Some(second)) = (subject.bounding_rect(), clip.bounding_rect()) else {
        return 0.0;
    };
    let width = first.max().x.max(second.max().x) - first.min().x.min(second.min().x);
    let height = first.max().y.max(second.max().y) - first.min().y.min(second.min().y);
    let half = 0.5 * width.max(height);
    if !(half > 0.0 && half.is_finite()) {
        return 0.0;
    }
    let step = (half.log2().ceil() - 29.0).exp2();
    let boundary: f64 = result
        .0
        .iter()
        .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
        .map(line_length)
        .sum();
    2.0 * boundary * step
}

pub(super) fn line_length(line: &LineString<f64>) -> f64 {
    line.0
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .sum()
}

/// Points at every multiple of `spacing_um` along each line, plus each line's end.
pub(super) fn sample_lines(lines: &MultiLineString<f64>, spacing_um: f64) -> Vec<Point> {
    let mut samples = Vec::new();
    for line in &lines.0 {
        let mut offset = 0.0;
        for pair in line.0.windows(2) {
            let length = (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y);
            while offset < length {
                let t = offset / length;
                samples.push([
                    pair[0].x + t * (pair[1].x - pair[0].x),
                    pair[0].y + t * (pair[1].y - pair[0].y),
                ]);
                offset += spacing_um;
            }
            offset -= length;
        }
        if let Some(last) = line.0.last().filter(|_| !line.is_closed()) {
            samples.push([last.x, last.y]);
        }
    }
    samples
}

fn position(coordinate: &Coord<f64>) -> serde_json::Value {
    serde_json::json!([canonical_zero(coordinate.x), canonical_zero(coordinate.y)])
}

fn ring_json(ring: &LineString<f64>) -> serde_json::Value {
    serde_json::Value::Array(ring.0.iter().map(position).collect())
}

pub(super) fn multipolygon_json(geometry: &MultiPolygon<f64>) -> serde_json::Value {
    serde_json::json!({
        "type": "MultiPolygon",
        "coordinates": geometry.0.iter().map(|polygon| {
            std::iter::once(polygon.exterior()).chain(polygon.interiors()).map(ring_json).collect::<Vec<_>>()
        }).collect::<Vec<_>>(),
    })
}

pub(super) fn multilinestring_json(lines: &MultiLineString<f64>) -> serde_json::Value {
    serde_json::json!({
        "type": "MultiLineString",
        "coordinates": lines.0.iter().map(ring_json).collect::<Vec<_>>(),
    })
}

/// Strict GeoJSON MultiLineString with finite positions and no zero-length segments.
pub(super) fn parse_multilinestring(
    value: &serde_json::Value,
    maximum_vertices: usize,
) -> Result<MultiLineString<f64>> {
    let object = value
        .as_object()
        .filter(|object| {
            object.len() == 2
                && object.get("type").and_then(|t| t.as_str()) == Some("MultiLineString")
        })
        .ok_or_else(|| invalid("declared front must be a GeoJSON MultiLineString"))?;
    let lines = object["coordinates"]
        .as_array()
        .filter(|lines| !lines.is_empty())
        .ok_or_else(|| invalid("declared front must contain at least one line"))?;
    let mut parsed = Vec::with_capacity(lines.len());
    let mut vertices = 0_usize;
    for line in lines {
        let positions = line
            .as_array()
            .filter(|positions| positions.len() >= 2)
            .ok_or_else(|| invalid("declared front lines need at least two positions"))?;
        vertices = vertices.saturating_add(positions.len());
        if vertices > maximum_vertices {
            return Err(invalid("declared front vertex resource limit exceeded"));
        }
        let coordinates = positions
            .iter()
            .map(|position| match position.as_array().map(Vec::as_slice) {
                Some([x, y]) => match (x.as_f64(), y.as_f64()) {
                    (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok(Coord { x, y }),
                    _ => Err(invalid(
                        "declared front positions must be finite [x, y] pairs",
                    )),
                },
                _ => Err(invalid(
                    "declared front positions must be finite [x, y] pairs",
                )),
            })
            .collect::<Result<Vec<_>>>()?;
        if coordinates.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("declared front contains a zero-length segment"));
        }
        parsed.push(LineString::new(coordinates));
    }
    Ok(MultiLineString::new(parsed))
}
