use std::path::Path;

use crate::errors::{MarklabError, Result};

use super::{
    cell_validation::{validate_cell_id, validate_probability, validate_xy, validation_error},
    cells::{HeCell, IhcCell},
};

pub(crate) fn load_he_cell_table_csv(path: impl AsRef<Path>) -> Result<Vec<HeCell>> {
    let path = path.as_ref();
    let mut reader = csv::Reader::from_path(path)
        .map_err(|err| MarklabError::Schema(format!("failed to read H&E cell CSV: {err}")))?;
    let mut cells = Vec::new();
    for (index, row) in reader.deserialize().enumerate() {
        let row_number = index + 2;
        let cell: HeCell =
            row.map_err(|err| MarklabError::Schema(format!("invalid H&E cell row: {err}")))?;
        validate_cell_id(path, row_number, &cell.cell_id)?;
        validate_xy(path, row_number, Some(&cell.cell_id), cell.x_um, cell.y_um)?;
        if cell
            .cell_type
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            return Err(validation_error(
                path,
                row_number,
                Some(&cell.cell_id),
                "H&E cell_type is required",
            ));
        }
        if cell.cell_type_probability.is_none() {
            return Err(validation_error(
                path,
                row_number,
                Some(&cell.cell_id),
                "cell_type_probability is required",
            ));
        }
        validate_probability(
            path,
            row_number,
            Some(&cell.cell_id),
            cell.cell_type_probability,
            "cell_type_probability",
        )?;
        cells.push(cell);
    }
    Ok(cells)
}

pub(crate) fn load_ihc_cell_table_csv(path: impl AsRef<Path>) -> Result<Vec<IhcCell>> {
    let path = path.as_ref();
    let mut reader = csv::Reader::from_path(path)
        .map_err(|err| MarklabError::Schema(format!("failed to read IHC cell CSV: {err}")))?;
    let mut cells = Vec::new();
    for (index, row) in reader.deserialize().enumerate() {
        let row_number = index + 2;
        let cell: IhcCell =
            row.map_err(|err| MarklabError::Schema(format!("invalid IHC cell row: {err}")))?;
        validate_cell_id(path, row_number, &cell.cell_id)?;
        validate_xy(path, row_number, Some(&cell.cell_id), cell.x_um, cell.y_um)?;
        if let Some(mark) = cell.mmr_mark {
            if mark > 1 {
                return Err(validation_error(
                    path,
                    row_number,
                    Some(&cell.cell_id),
                    "mmr_mark must be 0 or 1",
                ));
            }
        }
        validate_probability(
            path,
            row_number,
            Some(&cell.cell_id),
            cell.mmr_probability,
            "mmr_probability",
        )?;
        if cell.mmr_mark.is_none() && cell.mmr_probability.is_none() {
            return Err(validation_error(
                path,
                row_number,
                Some(&cell.cell_id),
                "IHC row requires mmr_mark or mmr_probability",
            ));
        }
        cells.push(cell);
    }
    Ok(cells)
}
