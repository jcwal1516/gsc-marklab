use marklab_numerics::summation::neumaier_sum;

use super::{
    components::band_index,
    front::Front,
    geometry::RingIndex,
    input::{Cell, Nucleus},
    preparation::SlideGeometry,
    result::{CellRow, CellSummary, ComponentRow, NucleusRow, NucleusSummary},
};

/// Place every cell in its epithelial component and front band, count epithelial
/// nuclei per component and derive nuclear shape and front alignment.
pub(super) fn assign(
    cells: &[Cell],
    slide: &SlideGeometry,
    front: &Front,
    components: &mut [ComponentRow],
    edges: &[f64],
) -> (Vec<CellRow>, CellSummary) {
    let epithelium = RingIndex::from_polygons(&slide.epithelium, |index| index as u32);
    for component in components.iter_mut() {
        component.epithelial_cell_count = Some(0);
    }
    let mut outside = 0;
    let mut rows = Vec::with_capacity(cells.len());
    for cell in cells {
        let point = [cell.x_um, cell.y_um];
        let component = epithelium.locate(point).map(|tag| tag as usize);
        if cell.epithelial {
            match component {
                Some(index) => {
                    if let Some(count) = components[index].epithelial_cell_count.as_mut() {
                        *count += 1;
                    }
                }
                None => outside += 1,
            }
        }
        let signed_front_distance_um = front.signed_distance_um(point);
        rows.push(CellRow {
            id: cell.id.clone(),
            epithelial: cell.epithelial,
            x_um: cell.x_um,
            y_um: cell.y_um,
            component_id: component.map(|index| components[index].id.clone()),
            class: component.map(|index| components[index].class.clone()),
            class_index: component.map(|index| components[index].class_index),
            signed_front_distance_um,
            band: signed_front_distance_um.and_then(|distance| band_index(distance, edges)),
            nucleus: cell.nucleus.map(|nucleus| shape(nucleus, front, point)),
        });
    }
    let summary = CellSummary {
        cell_count: cells.len(),
        epithelial_cell_count: cells.iter().filter(|cell| cell.epithelial).count(),
        epithelial_cells_outside_epithelium: outside,
        nucleus_count: cells.iter().filter(|cell| cell.nucleus.is_some()).count(),
    };
    (rows, summary)
}

/// Circularity, solidity and aspect ratio, and the nematic alignment `cos(2 * angle)`
/// between the nuclear major axis and the nearest front segment: +1 parallel to the
/// front, -1 perpendicular, 0 on average for isotropic orientation.
fn shape(nucleus: Nucleus, front: &Front, point: [f64; 2]) -> NucleusRow {
    NucleusRow {
        area_um2: nucleus.area_um2,
        circularity: 4.0 * std::f64::consts::PI * nucleus.area_um2
            / (nucleus.perimeter_um * nucleus.perimeter_um),
        solidity: nucleus.area_um2 / nucleus.convex_area_um2,
        aspect_ratio: nucleus.major_axis_um / nucleus.minor_axis_um,
        front_alignment: front
            .nearest_direction_rad(point)
            .map(|direction| (2.0 * (nucleus.major_axis_angle_rad - direction)).cos()),
    }
}

/// Mean epithelial nuclear shape for cells inside one class's components in one band.
pub(super) fn summarize(cells: &[CellRow], band: usize, class: usize) -> Option<NucleusSummary> {
    let nuclei = cells
        .iter()
        .filter(|cell| cell.epithelial && cell.band == Some(band) && cell.class_index == Some(class))
        .filter_map(|cell| cell.nucleus)
        .collect::<Vec<_>>();
    if nuclei.is_empty() {
        return None;
    }
    let count = nuclei.len() as f64;
    let mean = |value: fn(&NucleusRow) -> f64| neumaier_sum(nuclei.iter().map(value)) / count;
    let alignments = nuclei
        .iter()
        .map(|nucleus| nucleus.front_alignment)
        .collect::<Option<Vec<_>>>();
    Some(NucleusSummary {
        count: nuclei.len(),
        mean_area_um2: mean(|nucleus| nucleus.area_um2),
        mean_circularity: mean(|nucleus| nucleus.circularity),
        mean_solidity: mean(|nucleus| nucleus.solidity),
        mean_aspect_ratio: mean(|nucleus| nucleus.aspect_ratio),
        mean_front_alignment: alignments.map(|values| neumaier_sum(values) / count),
    })
}
