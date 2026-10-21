use geo::{Area, BooleanOps};
use marklab_numerics::summation::neumaier_sum;

use crate::Result;

use super::{
    front::Front,
    geometry::{bounded, buffered, charge_overlay, line_vertices, polygon_vertices},
    input::Recipe,
    invalid,
    preparation::SlideGeometry,
    result::{BandResult, ClassBand, ComponentRow},
};

/// Exact band areas from cumulative tissue-clipped front offsets on each side, with
/// whole components assigned by their interior point's signed distance.
pub(super) fn analyze(
    slide: &SlideGeometry,
    front: &Front,
    components: &[ComponentRow],
    recipe: &Recipe,
    class_order: &[String],
    remaining: &mut usize,
) -> Result<Vec<BandResult>> {
    if !front.available() {
        return Ok(Vec::new());
    }
    let edges = &recipe.design.band_edges_um;
    let lumens_declared = slide.lumens.is_some();
    let maximum = recipe.limits.maximum_geometry_vertices;
    charge_overlay(
        remaining,
        polygon_vertices(&slide.window) + polygon_vertices(&front.bulk),
        "host tissue",
    )?;
    let outside = bounded(slide.window.difference(&front.bulk), maximum, "host tissue")?;
    let mut cumulative = Vec::with_capacity(edges.len());
    for edge in edges {
        if *edge == 0.0 {
            cumulative.push(vec![0.0; class_order.len() + 1]);
            continue;
        }
        let side = if *edge < 0.0 { &front.bulk } else { &outside };
        let offset = buffered(
            &front.lines,
            edge.abs(),
            line_vertices(&front.lines),
            maximum,
            remaining,
            "front band offset",
        )?;
        charge_overlay(
            remaining,
            polygon_vertices(side) + polygon_vertices(&offset),
            "front band clipping",
        )?;
        let region = bounded(side.intersection(&offset), maximum, "front band")?;
        let mut areas = Vec::with_capacity(class_order.len() + 1);
        areas.push(region.unsigned_area());
        for class in &slide.class_epithelium {
            charge_overlay(
                remaining,
                polygon_vertices(class) + polygon_vertices(&region),
                "class band clipping",
            )?;
            areas.push(class.intersection(&region).unsigned_area());
        }
        cumulative.push(areas);
    }
    let tolerance = 1e-10 * slide.window_area_um2;
    let mut bands = Vec::with_capacity(edges.len() - 1);
    for (index, pair) in edges.windows(2).enumerate() {
        let (lower, upper) = (&cumulative[index], &cumulative[index + 1]);
        let areas = lower
            .iter()
            .zip(upper)
            .map(|(inner, outer)| {
                let area = if pair[1] <= 0.0 {
                    inner - outer
                } else if pair[0] >= 0.0 {
                    outer - inner
                } else {
                    inner + outer
                };
                if !area.is_finite() || area < -tolerance {
                    return Err(invalid("front offset areas are not monotone"));
                }
                Ok(area.max(0.0))
            })
            .collect::<Result<Vec<_>>>()?;
        let epithelial_area_um2 = neumaier_sum(areas[1..].iter().copied());
        let classes = class_order
            .iter()
            .enumerate()
            .map(|(class, name)| {
                let members = components
                    .iter()
                    .filter(|row| row.band == Some(index) && row.class_index == class)
                    .collect::<Vec<_>>();
                summarize(name, areas[class + 1], epithelial_area_um2, &members, lumens_declared)
            })
            .collect();
        bands.push(BandResult {
            lower_um: pair[0],
            upper_um: pair[1],
            tissue_area_um2: areas[0],
            epithelial_area_um2,
            epithelial_area_fraction: (areas[0] > 0.0).then(|| epithelial_area_um2 / areas[0]),
            classes,
        });
    }
    Ok(bands)
}

fn summarize(
    class: &str,
    epithelial_area_um2: f64,
    band_epithelial_area_um2: f64,
    members: &[&ComponentRow],
    lumens_declared: bool,
) -> ClassBand {
    let mut areas = members.iter().map(|row| row.area_um2).collect::<Vec<_>>();
    areas.sort_by(f64::total_cmp);
    let component_area_um2 = neumaier_sum(areas.iter().copied());
    let entropy = (component_area_um2 > 0.0).then(|| {
        if areas.len() < 2 {
            return 0.0;
        }
        let entropy = -neumaier_sum(areas.iter().map(|area| {
            let share = area / component_area_um2;
            share * share.ln()
        }));
        entropy / (areas.len() as f64).ln()
    });
    let turning = neumaier_sum(members.iter().map(|row| row.absolute_turning_rad));
    let length = neumaier_sum(members.iter().map(|row| row.curvature_contour_length_um));
    ClassBand {
        class: class.to_owned(),
        epithelial_area_um2,
        epithelial_area_share: (band_epithelial_area_um2 > 0.0)
            .then(|| epithelial_area_um2 / band_epithelial_area_um2),
        component_count: members.len(),
        component_area_um2,
        median_component_area_um2: crate::common::stats::median_sorted_average_even(&areas),
        normalized_component_area_entropy: entropy,
        mean_absolute_curvature_per_um: (length > 0.0).then(|| turning / length),
        area_weighted_isoperimetric_quotient: (component_area_um2 > 0.0).then(|| {
            neumaier_sum(
                members
                    .iter()
                    .map(|row| row.area_um2 * row.isoperimetric_quotient),
            ) / component_area_um2
        }),
        lumen_containing_component_fraction: (lumens_declared && !members.is_empty()).then(|| {
            members
                .iter()
                .filter(|row| row.lumen_count.is_some_and(|count| count > 0))
                .count() as f64
                / members.len() as f64
        }),
    }
}
