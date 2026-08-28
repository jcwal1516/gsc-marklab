use rstar::{PointDistance, RTree, RTreeObject, AABB};
use std::cmp::Ordering;

use super::ObservationWindowError;

pub(super) type Point = [f64; 2];
pub(super) type Ring = Vec<Point>;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Polygon {
    pub(super) exterior: Ring,
    pub(super) holes: Vec<Ring>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BoundarySegment {
    pub(super) start: Point,
    pub(super) end: Point,
    ring_id: usize,
    index_in_ring: usize,
    segment_count: usize,
    global_index: usize,
}

impl BoundarySegment {
    pub(super) fn distance_2_to(&self, point: Point) -> f64 {
        segment_distance_2(self.start, self.end, point)
    }

    pub(super) fn canonical_key(&self) -> [u64; 4] {
        let start = [self.start[0].to_bits(), self.start[1].to_bits()];
        let end = [self.end[0].to_bits(), self.end[1].to_bits()];
        if start <= end {
            [start[0], start[1], end[0], end[1]]
        } else {
            [end[0], end[1], start[0], start[1]]
        }
    }

    fn adjacent(&self, other: &Self) -> bool {
        self.ring_id == other.ring_id
            && (self.index_in_ring.abs_diff(other.index_in_ring) == 1
                || (self.index_in_ring == 0 && other.index_in_ring + 1 == self.segment_count)
                || (other.index_in_ring == 0 && self.index_in_ring + 1 == self.segment_count))
    }
}

impl RTreeObject for BoundarySegment {
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

impl PointDistance for BoundarySegment {
    fn distance_2(&self, point: &Point) -> f64 {
        self.distance_2_to(*point)
    }
}

pub(super) struct TopologySummary {
    pub(super) area_um2: f64,
    pub(super) perimeter_um: f64,
    pub(super) bounds_um: [f64; 4],
    pub(super) hole_count: usize,
    pub(super) segments: Vec<BoundarySegment>,
}

pub(super) fn validate_and_canonicalize(
    polygons: &mut [Polygon],
    maximum_candidates: usize,
) -> Result<TopologySummary, ObservationWindowError> {
    let mut total_area = 0.0;
    let mut perimeter = 0.0;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut hole_count = 0_usize;

    for polygon in polygons.iter_mut() {
        canonicalize_ring(&mut polygon.exterior, true)?;
        let exterior_area = signed_area(&polygon.exterior).abs();
        let mut hole_area = 0.0;
        perimeter += ring_perimeter(&polygon.exterior)?;
        update_bounds(&mut bounds, &polygon.exterior);
        for hole in &mut polygon.holes {
            canonicalize_ring(hole, false)?;
            hole_area += signed_area(hole).abs();
            perimeter += ring_perimeter(hole)?;
            hole_count += 1;
        }
        polygon
            .holes
            .sort_by(|left, right| compare_ring(left, right));
        let component_area = exterior_area - hole_area;
        if !component_area.is_finite() || component_area <= 0.0 {
            return invalid("polygon holes must leave positive finite area");
        }
        total_area += component_area;
    }
    polygons.sort_by(compare_polygon);
    if !total_area.is_finite() || total_area <= 0.0 || !perimeter.is_finite() || perimeter <= 0.0 {
        return invalid("window area and perimeter must be finite and positive");
    }

    let segments = build_segments(polygons)?;
    validate_segment_intersections(&segments, maximum_candidates)?;
    validate_containment(polygons)?;
    Ok(TopologySummary {
        area_um2: total_area,
        perimeter_um: perimeter,
        bounds_um: bounds,
        hole_count,
        segments,
    })
}

pub(super) fn window_contains(polygons: &[Polygon], point: Point) -> bool {
    if polygons
        .iter()
        .flat_map(|polygon| std::iter::once(&polygon.exterior).chain(polygon.holes.iter()))
        .flat_map(|ring| ring.windows(2))
        .any(|segment| point_on_segment(point, segment[0], segment[1]))
    {
        return true;
    }
    polygons
        .iter()
        .any(|polygon| polygon_contains(polygon, point))
}

fn polygon_contains(polygon: &Polygon, point: Point) -> bool {
    point_in_ring(&polygon.exterior, point)
        && !polygon.holes.iter().any(|hole| point_in_ring(hole, point))
}

fn canonicalize_ring(ring: &mut Ring, exterior: bool) -> Result<(), ObservationWindowError> {
    for segment in ring.windows(2) {
        if segment[0] == segment[1] {
            return invalid("linear ring contains a zero-length edge");
        }
    }
    let area = signed_area(ring);
    if !area.is_finite() || area == 0.0 {
        return invalid("linear ring must have nonzero finite signed area");
    }
    let should_reverse = (exterior && area < 0.0) || (!exterior && area > 0.0);
    if should_reverse {
        ring.reverse();
    }
    let open_len = ring.len() - 1;
    let first = (0..open_len)
        .min_by(|left, right| compare_point(ring[*left], ring[*right]))
        .expect("validated ring has an open vertex");
    ring[..open_len].rotate_left(first);
    ring[open_len] = ring[0];
    Ok(())
}

fn compare_polygon(left: &Polygon, right: &Polygon) -> Ordering {
    compare_ring(&left.exterior, &right.exterior)
        .then_with(|| left.holes.len().cmp(&right.holes.len()))
        .then_with(|| {
            left.holes
                .iter()
                .zip(&right.holes)
                .map(|(left, right)| compare_ring(left, right))
                .find(|ordering| *ordering != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        })
}

fn compare_ring(left: &[Point], right: &[Point]) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| {
        left.iter()
            .zip(right)
            .map(|(left, right)| compare_point(*left, *right))
            .find(|ordering| *ordering != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    })
}

fn compare_point(left: Point, right: Point) -> Ordering {
    left[0]
        .total_cmp(&right[0])
        .then_with(|| left[1].total_cmp(&right[1]))
}

fn signed_area(ring: &[Point]) -> f64 {
    ring.windows(2)
        .map(|segment| segment[0][0] * segment[1][1] - segment[1][0] * segment[0][1])
        .sum::<f64>()
        * 0.5
}

pub(super) fn polygon_area(polygon: &Polygon) -> f64 {
    signed_area(&polygon.exterior).abs()
        - polygon
            .holes
            .iter()
            .map(|hole| signed_area(hole).abs())
            .sum::<f64>()
}

fn ring_perimeter(ring: &[Point]) -> Result<f64, ObservationWindowError> {
    let perimeter = ring
        .windows(2)
        .map(|segment| (segment[1][0] - segment[0][0]).hypot(segment[1][1] - segment[0][1]))
        .sum::<f64>();
    if perimeter.is_finite() && perimeter > 0.0 {
        Ok(perimeter)
    } else {
        invalid("linear ring perimeter must be finite and positive")
    }
}

fn update_bounds(bounds: &mut [f64; 4], ring: &[Point]) {
    for [x, y] in ring.iter().copied().take(ring.len().saturating_sub(1)) {
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    }
}

fn build_segments(polygons: &[Polygon]) -> Result<Vec<BoundarySegment>, ObservationWindowError> {
    let capacity = polygons
        .iter()
        .flat_map(|polygon| std::iter::once(&polygon.exterior).chain(polygon.holes.iter()))
        .try_fold(0_usize, |total, ring| {
            total
                .checked_add(ring.len().saturating_sub(1))
                .ok_or(ObservationWindowError::SizeOverflow)
        })?;
    let mut segments = Vec::new();
    segments
        .try_reserve_exact(capacity)
        .map_err(|_| ObservationWindowError::SizeOverflow)?;
    for (ring_id, ring) in polygons
        .iter()
        .flat_map(|polygon| std::iter::once(&polygon.exterior).chain(polygon.holes.iter()))
        .enumerate()
    {
        let segment_count = ring.len() - 1;
        for (index_in_ring, segment) in ring.windows(2).enumerate() {
            segments.push(BoundarySegment {
                start: segment[0],
                end: segment[1],
                ring_id,
                index_in_ring,
                segment_count,
                global_index: segments.len(),
            });
        }
    }
    Ok(segments)
}

fn validate_segment_intersections(
    segments: &[BoundarySegment],
    maximum_candidates: usize,
) -> Result<(), ObservationWindowError> {
    let tree = RTree::bulk_load(segments.to_vec());
    let mut candidates = 0_usize;
    for segment in segments {
        for other in tree.locate_in_envelope_intersecting(segment.envelope()) {
            if other.global_index <= segment.global_index || segment.adjacent(other) {
                continue;
            }
            candidates = candidates
                .checked_add(1)
                .ok_or(ObservationWindowError::SizeOverflow)?;
            if candidates > maximum_candidates {
                return Err(ObservationWindowError::TopologyCandidateLimitExceeded {
                    maximum: maximum_candidates,
                });
            }
            if segments_intersect(segment.start, segment.end, other.start, other.end) {
                return invalid("rings self-intersect, cross, overlap, or touch");
            }
        }
    }
    Ok(())
}

fn validate_containment(polygons: &[Polygon]) -> Result<(), ObservationWindowError> {
    for polygon in polygons {
        for (hole_index, hole) in polygon.holes.iter().enumerate() {
            let sample = hole[0];
            if !point_in_ring(&polygon.exterior, sample) {
                return invalid("hole lies outside its exterior ring");
            }
            for (other_index, other) in polygon.holes.iter().enumerate() {
                if hole_index != other_index && point_in_ring(other, sample) {
                    return invalid("holes overlap or contain one another");
                }
            }
        }
    }
    for left in 0..polygons.len() {
        for right in (left + 1)..polygons.len() {
            if polygon_contains(&polygons[left], polygons[right].exterior[0])
                || polygon_contains(&polygons[right], polygons[left].exterior[0])
            {
                return invalid("polygon components overlap or contain one another");
            }
        }
    }
    Ok(())
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    if opposite_signs(ab_c, ab_d) && opposite_signs(cd_a, cd_b) {
        return true;
    }
    (ab_c == 0.0 && point_on_segment(c, a, b))
        || (ab_d == 0.0 && point_on_segment(d, a, b))
        || (cd_a == 0.0 && point_on_segment(a, c, d))
        || (cd_b == 0.0 && point_on_segment(b, c, d))
}

fn opposite_signs(left: f64, right: f64) -> bool {
    (left < 0.0 && right > 0.0) || (left > 0.0 && right < 0.0)
}

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn point_on_segment(point: Point, start: Point, end: Point) -> bool {
    cross(start, end, point) == 0.0
        && point[0] >= start[0].min(end[0])
        && point[0] <= start[0].max(end[0])
        && point[1] >= start[1].min(end[1])
        && point[1] <= start[1].max(end[1])
}

fn point_in_ring(ring: &[Point], point: Point) -> bool {
    let mut inside = false;
    for segment in ring.windows(2) {
        let [start, end] = [segment[0], segment[1]];
        let crosses = (start[1] > point[1]) != (end[1] > point[1]);
        if crosses {
            let intersection_x =
                (end[0] - start[0]) * (point[1] - start[1]) / (end[1] - start[1]) + start[0];
            if point[0] < intersection_x {
                inside = !inside;
            }
        }
    }
    inside
}

fn segment_distance_2(start: Point, end: Point, point: Point) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length_2 = dx * dx + dy * dy;
    let t = (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length_2).clamp(0.0, 1.0);
    let nearest_x = start[0] + t * dx;
    let nearest_y = start[1] + t * dy;
    (point[0] - nearest_x).powi(2) + (point[1] - nearest_y).powi(2)
}

fn invalid<T>(reason: &str) -> Result<T, ObservationWindowError> {
    Err(ObservationWindowError::InvalidTopology {
        reason: reason.into(),
    })
}
