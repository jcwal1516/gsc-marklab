use geo::{Area, BooleanOps};
use marklab_numerics::summation::neumaier_sum;

use crate::Result;

use super::{
    front::Front,
    geometry::{
        bounded, buffered, charge_overlay, line_vertices, overlay_area_error, polygon_vertices,
    },
    input::Recipe,
    invalid, nuclei,
    preparation::SlideGeometry,
    result::{BandResult, CellRow, ClassBand, ComponentRow},
};

/// Exact band areas from cumulative tissue-clipped front offsets on each side, with
/// whole components assigned by their interior point's signed distance.
pub(super) struct Measured<'a> {
    pub components: &'a [ComponentRow],
    pub cells: &'a [CellRow],
    pub buds_available: bool,
}

pub(super) fn analyze(
    slide: &SlideGeometry,
    front: &Front,
    measured: &Measured<'_>,
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
    // Per cumulative area, the bound on what the overlays' coordinate rounding can change it by.
    let mut rounding = Vec::with_capacity(edges.len());
    for edge in edges {
        if *edge == 0.0 {
            cumulative.push(vec![0.0; class_order.len() + 1]);
            rounding.push(vec![0.0; class_order.len() + 1]);
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
        let region_error = overlay_area_error(side, &offset, &region);
        let mut areas = Vec::with_capacity(class_order.len() + 1);
        let mut errors = Vec::with_capacity(class_order.len() + 1);
        areas.push(region.unsigned_area());
        errors.push(region_error);
        for class in &slide.class_epithelium {
            charge_overlay(
                remaining,
                polygon_vertices(class) + polygon_vertices(&region),
                "class band clipping",
            )?;
            let clipped = class.intersection(&region);
            areas.push(clipped.unsigned_area());
            // The class area also moves with the region's own rounding.
            errors.push(region_error + overlay_area_error(class, &region, &clipped));
        }
        cumulative.push(areas);
        rounding.push(errors);
    }
    let tolerance = 1e-10 * slide.window_area_um2;
    let mut bands = Vec::with_capacity(edges.len() - 1);
    for (index, pair) in edges.windows(2).enumerate() {
        let (lower, upper) = (&cumulative[index], &cumulative[index + 1]);
        let errors = rounding[index].iter().zip(&rounding[index + 1]);
        let areas = lower
            .iter()
            .zip(upper)
            .zip(errors)
            .map(|((inner, outer), (inner_error, outer_error))| {
                let area = if pair[1] <= 0.0 {
                    inner - outer
                } else if pair[0] >= 0.0 {
                    outer - inner
                } else {
                    inner + outer
                };
                // An empty band can come out slightly negative from rounding; beyond the
                // rounding bound the offsets are inconsistent.
                if !area.is_finite() || area < -(tolerance + inner_error + outer_error) {
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
                let members = measured
                    .components
                    .iter()
                    .filter(|row| row.band == Some(index) && row.class_index == class)
                    .collect::<Vec<_>>();
                let mut summary = summarize(
                    name,
                    areas[class + 1],
                    epithelial_area_um2,
                    &members,
                    lumens_declared,
                );
                summary.bud_count = measured
                    .buds_available
                    .then(|| members.iter().filter(|row| row.bud == Some(true)).count());
                summary.epithelial_nuclei = nuclei::summarize(measured.cells, index, class);
                summary
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
        bud_count: None,
        branch_points_per_mm2: per_mm2(members, component_area_um2, |row| {
            row.branch_point_count.map(|count| count as f64)
        }),
        skeleton_length_um_per_mm2: per_mm2(members, component_area_um2, |row| {
            row.skeleton_length_um
        }),
        epithelial_nuclei: None,
    }
}

/// Summed component quantity per square millimetre of component area, when every
/// member reports it.
fn per_mm2(
    members: &[&ComponentRow],
    area_um2: f64,
    value: impl Fn(&ComponentRow) -> Option<f64>,
) -> Option<f64> {
    if members.is_empty() || area_um2 <= 0.0 {
        return None;
    }
    let values = members
        .iter()
        .map(|row| value(row))
        .collect::<Option<Vec<_>>>()?;
    Some(neumaier_sum(values) / area_um2 * 1e6)
}
