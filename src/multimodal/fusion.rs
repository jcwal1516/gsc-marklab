use crate::errors::{MarklabError, Result};
use crate::multimodal::cell_table::{CellSection, FusedCell, HeCell, IhcCell};
use crate::registration::transform::Transform2D;

#[derive(Clone, Debug)]
pub struct FusionMeta {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub registration_error_um: Option<f64>,
}

pub fn fuse_registered_cells(
    he_cells: &[HeCell],
    ihc_cells: &[IhcCell],
    he_to_ihc: &Transform2D,
    meta: &FusionMeta,
) -> Result<Vec<FusedCell>> {
    validate_transform(he_to_ihc)?;
    validate_meta(meta)?;

    let mut fused = Vec::with_capacity(he_cells.len() + ihc_cells.len());
    for cell in he_cells {
        validate_coordinates("H&E", &cell.cell_id, cell.x_um, cell.y_um)?;
        let (x_um_registered, y_um_registered) = he_to_ihc.apply(cell.x_um, cell.y_um);
        validate_coordinates(
            "registered H&E",
            &cell.cell_id,
            x_um_registered,
            y_um_registered,
        )?;
        fused.push(FusedCell {
            source_section: CellSection::He,
            source_cell_id: cell.cell_id.clone(),
            x_um_registered,
            y_um_registered,
            mmr_mark: None,
            mmr_probability: None,
            cell_type: cell.cell_type.clone(),
            cell_type_probability: cell.cell_type_probability,
            same_section: false,
            registration_error_um: meta.registration_error_um,
            timepoint: meta.timepoint.clone(),
            case_id: meta.case_id.clone(),
            protein: meta.protein.clone(),
        });
    }

    for cell in ihc_cells {
        validate_coordinates("IHC", &cell.cell_id, cell.x_um, cell.y_um)?;
        fused.push(FusedCell {
            source_section: CellSection::Ihc,
            source_cell_id: cell.cell_id.clone(),
            x_um_registered: cell.x_um,
            y_um_registered: cell.y_um,
            mmr_mark: cell.mmr_mark,
            mmr_probability: cell.mmr_probability,
            cell_type: None,
            cell_type_probability: None,
            same_section: true,
            registration_error_um: meta.registration_error_um,
            timepoint: meta.timepoint.clone(),
            case_id: meta.case_id.clone(),
            protein: meta.protein.clone(),
        });
    }

    Ok(fused)
}

fn validate_transform(transform: &Transform2D) -> Result<()> {
    let coefficients = [
        transform.m00,
        transform.m01,
        transform.m02,
        transform.m10,
        transform.m11,
        transform.m12,
    ];
    if coefficients.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(MarklabError::Validation(
            "registration transform coefficients must be finite".into(),
        ))
    }
}

fn validate_meta(meta: &FusionMeta) -> Result<()> {
    validate_required_meta_field("case_id", &meta.case_id)?;
    validate_required_meta_field("timepoint", &meta.timepoint)?;
    validate_required_meta_field("protein", &meta.protein)?;

    if let Some(error_um) = meta.registration_error_um {
        if !error_um.is_finite() || error_um < 0.0 {
            return Err(MarklabError::Validation(
                "registration_error_um must be finite and non-negative".into(),
            ));
        }
    }
    Ok(())
}

fn validate_required_meta_field(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(MarklabError::Schema(format!(
            "FusionMeta.{name} must not be blank"
        )))
    } else {
        Ok(())
    }
}

fn validate_coordinates(section: &str, cell_id: &str, x_um: f64, y_um: f64) -> Result<()> {
    if x_um.is_finite() && y_um.is_finite() {
        Ok(())
    } else {
        Err(MarklabError::Validation(format!(
            "{section} cell {cell_id} coordinates must be finite"
        )))
    }
}
