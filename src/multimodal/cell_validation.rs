use std::path::Path;

use crate::errors::{MarklabError, Result};

pub(super) fn validate_cell_id(path: &Path, row_number: usize, cell_id: &str) -> Result<()> {
    if cell_id.trim().is_empty() {
        return Err(validation_error(
            path,
            row_number,
            Some(cell_id),
            "cell_id is required",
        ));
    }
    Ok(())
}

pub(super) fn validate_xy(
    path: &Path,
    row_number: usize,
    cell_id: Option<&str>,
    x: f64,
    y: f64,
) -> Result<()> {
    if x.is_finite() && y.is_finite() {
        Ok(())
    } else {
        Err(validation_error(
            path,
            row_number,
            cell_id,
            "cell coordinates must be finite",
        ))
    }
}

pub(super) fn validate_probability(
    path: &Path,
    row_number: usize,
    cell_id: Option<&str>,
    value: Option<f64>,
    name: &str,
) -> Result<()> {
    if let Some(value) = value {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(validation_error(
                path,
                row_number,
                cell_id,
                &format!("{name} must be in [0, 1]"),
            ));
        }
    }
    Ok(())
}

pub(super) fn validation_error(
    path: &Path,
    row_number: usize,
    cell_id: Option<&str>,
    message: &str,
) -> MarklabError {
    let cell_id_context = match cell_id {
        Some(cell_id) if cell_id.trim().is_empty() => ", cell_id <blank>".to_string(),
        Some(cell_id) => format!(", cell_id {}", cell_id.trim()),
        None => String::new(),
    };
    MarklabError::Schema(format!(
        "{} row {}{}: {}",
        path.display(),
        row_number,
        cell_id_context,
        message
    ))
}
