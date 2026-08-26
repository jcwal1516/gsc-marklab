use crate::SimulationError;

use super::{
    MappedUptakeCell, MappedVesselSource, UptakeCell, VascularTransportSpec, VesselSource,
};

pub(super) fn validate_entities(
    vessels: &[VesselSource],
    cells: &[UptakeCell],
) -> Result<(), SimulationError> {
    for pair in vessels.windows(2) {
        if pair[0].vessel_id == pair[1].vessel_id {
            return Err(SimulationError::Invalid("duplicate vessel ID".into()));
        }
    }
    for vessel in vessels {
        if invalid_id(&vessel.vessel_id)
            || !vessel.source_concentration_per_time.is_finite()
            || vessel.source_concentration_per_time < 0.0
        {
            return Err(SimulationError::Invalid("invalid vessel source".into()));
        }
    }
    for pair in cells.windows(2) {
        if pair[0].cell_id == pair[1].cell_id {
            return Err(SimulationError::Invalid("duplicate uptake cell ID".into()));
        }
    }
    for cell in cells {
        if invalid_id(&cell.cell_id)
            || !cell.linear_uptake_per_time.is_finite()
            || cell.linear_uptake_per_time < 0.0
        {
            return Err(SimulationError::Invalid("invalid uptake cell".into()));
        }
    }
    Ok(())
}

fn invalid_id(value: &str) -> bool {
    value.is_empty() || value.trim() != value
}

pub(super) fn map_vessels(
    spec: &VascularTransportSpec,
) -> Result<(Vec<MappedVesselSource>, Vec<f64>), SimulationError> {
    let mut field = vec![0.0; spec.initial_concentration_row_major.len()];
    let mut mapped = Vec::with_capacity(spec.vessel_sources.len());
    for vessel in &spec.vessel_sources {
        let (index, ix, iy, distance) = map_point(vessel.x_um, vessel.y_um, spec)?;
        field[index] += vessel.source_concentration_per_time;
        if !field[index].is_finite() {
            return Err(SimulationError::Invalid(
                "mapped vessel source overflowed".into(),
            ));
        }
        mapped.push(MappedVesselSource {
            vessel_id: vessel.vessel_id.clone(),
            grid_index: index,
            ix,
            iy,
            mapping_distance_um: distance,
            source_concentration_per_time: vessel.source_concentration_per_time,
        });
    }
    Ok((mapped, field))
}

pub(super) fn map_uptake_cells(
    spec: &VascularTransportSpec,
) -> Result<(Vec<MappedUptakeCell>, Vec<f64>), SimulationError> {
    let mut field = vec![0.0; spec.initial_concentration_row_major.len()];
    let mut mapped = Vec::with_capacity(spec.uptake_cells.len());
    for cell in &spec.uptake_cells {
        let (index, ix, iy, distance) = map_point(cell.x_um, cell.y_um, spec)?;
        field[index] += cell.linear_uptake_per_time;
        if !field[index].is_finite() {
            return Err(SimulationError::Invalid(
                "mapped cell uptake overflowed".into(),
            ));
        }
        mapped.push(MappedUptakeCell {
            cell_id: cell.cell_id.clone(),
            grid_index: index,
            ix,
            iy,
            mapping_distance_um: distance,
            linear_uptake_per_time: cell.linear_uptake_per_time,
        });
    }
    Ok((mapped, field))
}

fn map_point(
    x_um: f64,
    y_um: f64,
    spec: &VascularTransportSpec,
) -> Result<(usize, u32, u32, f64), SimulationError> {
    let maximum_x = f64::from(spec.grid_x - 1) * spec.spacing_x_um;
    let maximum_y = f64::from(spec.grid_y - 1) * spec.spacing_y_um;
    if !x_um.is_finite()
        || !y_um.is_finite()
        || !(0.0..=maximum_x).contains(&x_um)
        || !(0.0..=maximum_y).contains(&y_um)
    {
        return Err(SimulationError::Invalid(
            "vessel or uptake coordinate lies outside the grid".into(),
        ));
    }
    let ix = (x_um / spec.spacing_x_um).round() as u32;
    let iy = (y_um / spec.spacing_y_um).round() as u32;
    let mapped_x = f64::from(ix) * spec.spacing_x_um;
    let mapped_y = f64::from(iy) * spec.spacing_y_um;
    Ok((
        iy as usize * spec.grid_x as usize + ix as usize,
        ix,
        iy,
        (x_um - mapped_x).hypot(y_um - mapped_y),
    ))
}
