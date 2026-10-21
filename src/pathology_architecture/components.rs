use geo::{Area, InteriorPoint, LineString, Polygon};
use marklab_numerics::summation::neumaier_sum;

use crate::Result;

use super::{
    charge,
    front::Front,
    geometry::{line_length, Point, RingIndex},
    input::Design,
    invalid,
    preparation::SlideGeometry,
    result::ComponentRow,
    UNCLASSIFIED,
};

/// Fewest uniform contour samples for which turning is reported.
const MINIMUM_CONTOUR_SAMPLES: usize = 16;

/// Measure every epithelial component and place its interior point against the front.
pub(super) fn measure(
    slide: &SlideGeometry,
    front: &Front,
    design: &Design,
    classes: &[String],
    remaining: &mut usize,
) -> Result<Vec<ComponentRow>> {
    let polygons = &slide.epithelium.0;
    let lumen_counts = slide
        .lumens
        .as_ref()
        .map(|lumens| count_lumens(polygons, &lumens.0))
        .transpose()?;
    let mut rows = Vec::with_capacity(polygons.len());
    for (index, polygon) in polygons.iter().enumerate() {
        let area_um2 = polygon.unsigned_area();
        let rings = std::iter::once(polygon.exterior())
            .chain(polygon.interiors())
            .collect::<Vec<_>>();
        let perimeter_um = neumaier_sum(rings.iter().map(|ring| line_length(ring)));
        let mut turning = Vec::with_capacity(rings.len());
        let mut contour_length = Vec::with_capacity(rings.len());
        for ring in &rings {
            let samples = (line_length(ring) / design.contour_sample_spacing_um).round();
            charge(
                remaining,
                (samples as usize).saturating_mul(
                    kernel_half_width(
                        design.curvature_smoothing_um,
                        design.contour_sample_spacing_um,
                    )
                    .saturating_mul(2)
                    .saturating_add(2),
                ),
                "contour curvature",
            )?;
            if let Some((ring_turning, ring_length)) = ring_turning(
                ring,
                design.contour_sample_spacing_um,
                design.curvature_smoothing_um,
            ) {
                turning.push(ring_turning);
                contour_length.push(ring_length);
            }
        }
        let absolute_turning_rad = neumaier_sum(turning);
        let curvature_contour_length_um = neumaier_sum(contour_length);
        let point = polygon
            .interior_point()
            .ok_or_else(|| invalid("epithelial component has no interior point"))?;
        let point = [point.x(), point.y()];
        let class_index = slide
            .regions
            .locate(point)
            .map_or(classes.len(), |tag| tag as usize);
        let signed_front_distance_um = front.signed_distance_um(point);
        rows.push(ComponentRow {
            id: format!("epi-{index:06}"),
            class: classes
                .get(class_index)
                .map_or(UNCLASSIFIED, String::as_str)
                .to_owned(),
            class_index,
            x_um: point[0],
            y_um: point[1],
            area_um2,
            perimeter_um,
            hole_count: polygon.interiors().len(),
            lumen_count: lumen_counts.as_ref().map(|counts| counts[index]),
            isoperimetric_quotient: 4.0 * std::f64::consts::PI * area_um2
                / (perimeter_um * perimeter_um),
            absolute_turning_rad,
            curvature_contour_length_um,
            mean_absolute_curvature_per_um: (curvature_contour_length_um > 0.0)
                .then(|| absolute_turning_rad / curvature_contour_length_um),
            signed_front_distance_um,
            band: signed_front_distance_um
                .and_then(|distance| band_index(distance, &design.band_edges_um)),
            epithelial_cell_count: None,
            bud: None,
            skeleton_length_um: None,
            branch_point_count: None,
            end_point_count: None,
        });
    }
    Ok(rows)
}

/// Half-open band containing a signed distance; the final band includes its upper edge.
pub(super) fn band_index(distance: f64, edges: &[f64]) -> Option<usize> {
    let bands = edges.len() - 1;
    if distance < edges[0] || distance > edges[bands] {
        return None;
    }
    Some(
        edges
            .partition_point(|edge| *edge <= distance)
            .saturating_sub(1)
            .min(bands - 1),
    )
}

/// Assign each declared lumen to the smallest epithelial hole containing its interior point.
fn count_lumens(components: &[Polygon<f64>], lumens: &[Polygon<f64>]) -> Result<Vec<usize>> {
    let holes = components
        .iter()
        .enumerate()
        .flat_map(|(component, polygon)| {
            polygon
                .interiors()
                .iter()
                .map(move |ring| (component, ring))
        })
        .collect::<Vec<_>>();
    let areas = holes
        .iter()
        .map(|(_, ring)| Polygon::new((*ring).clone(), vec![]).unsigned_area())
        .collect::<Vec<_>>();
    let index = RingIndex::from_lines(
        holes
            .iter()
            .enumerate()
            .map(|(tag, (_, ring))| (tag as u32, *ring)),
    );
    let mut counts = vec![0; components.len()];
    for lumen in lumens {
        let point = lumen
            .interior_point()
            .ok_or_else(|| invalid("lumen component has no interior point"))?;
        if let Some(hole) = index
            .locate_all([point.x(), point.y()])
            .into_iter()
            .min_by(|left, right| areas[*left as usize].total_cmp(&areas[*right as usize]))
        {
            counts[holes[hole as usize].0] += 1;
        }
    }
    Ok(counts)
}

fn kernel_half_width(smoothing_um: f64, spacing_um: f64) -> usize {
    if smoothing_um == 0.0 {
        0
    } else {
        (4.0 * smoothing_um / spacing_um).ceil().max(1.0) as usize
    }
}

/// Absolute turning and length of one closed ring after uniform resampling and
/// circular Gaussian smoothing. A simple convex contour turns by exactly 2*pi.
///
/// Returns `None` when the ring has fewer than sixteen samples or cannot hold the
/// complete smoothing kernel without wrapping.
pub(super) fn ring_turning(
    ring: &LineString<f64>,
    spacing_um: f64,
    smoothing_um: f64,
) -> Option<(f64, f64)> {
    let length = line_length(ring);
    let count = (length / spacing_um).round();
    if !count.is_finite() || count < MINIMUM_CONTOUR_SAMPLES as f64 {
        return None;
    }
    let count = count as usize;
    let step = length / count as f64;
    let half = kernel_half_width(smoothing_um, step);
    if 2 * half + 1 > count {
        return None;
    }
    let origin = ring.0[0];
    let edges = ring
        .0
        .windows(2)
        .map(|pair| {
            let start = [pair[0].x - origin.x, pair[0].y - origin.y];
            let end = [pair[1].x - origin.x, pair[1].y - origin.y];
            (start, end, (end[0] - start[0]).hypot(end[1] - start[1]))
        })
        .collect::<Vec<_>>();
    let mut samples: Vec<Point> = Vec::with_capacity(count);
    let (mut edge, mut edge_start) = (0, 0.0);
    for sample in 0..count {
        let offset = sample as f64 * step;
        while edge + 1 < edges.len() && edge_start + edges[edge].2 < offset {
            edge_start += edges[edge].2;
            edge += 1;
        }
        let (start, end, edge_length) = edges[edge];
        let t = ((offset - edge_start) / edge_length).clamp(0.0, 1.0);
        samples.push([
            start[0] + t * (end[0] - start[0]),
            start[1] + t * (end[1] - start[1]),
        ]);
    }
    if half > 0 {
        let sigma = smoothing_um / step;
        let weights = (0..=2 * half)
            .map(|j| {
                let offset = j as f64 - half as f64;
                (-0.5 * (offset / sigma).powi(2)).exp()
            })
            .collect::<Vec<_>>();
        let total = neumaier_sum(weights.iter().copied());
        samples = (0..count)
            .map(|center| {
                let mut point = [0.0, 0.0];
                for (j, weight) in weights.iter().enumerate() {
                    let source = samples[(center + count + j - half) % count];
                    point[0] += weight * source[0];
                    point[1] += weight * source[1];
                }
                [point[0] / total, point[1] / total]
            })
            .collect();
    }
    let mut turning = Vec::with_capacity(count);
    let mut lengths = Vec::with_capacity(count);
    for center in 0..count {
        let previous = samples[(center + count - 1) % count];
        let current = samples[center];
        let next = samples[(center + 1) % count];
        let incoming = [current[0] - previous[0], current[1] - previous[1]];
        let outgoing = [next[0] - current[0], next[1] - current[1]];
        let cross = incoming[0] * outgoing[1] - incoming[1] * outgoing[0];
        let dot = incoming[0] * outgoing[0] + incoming[1] * outgoing[1];
        turning.push(cross.atan2(dot).abs());
        lengths.push(outgoing[0].hypot(outgoing[1]));
    }
    let turning = neumaier_sum(turning);
    let length = neumaier_sum(lengths);
    (turning.is_finite() && length.is_finite() && length > 0.0).then_some((turning, length))
}
