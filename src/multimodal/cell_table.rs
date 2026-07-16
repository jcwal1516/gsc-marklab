use serde::{Deserialize, Serialize};
#[cfg(feature = "cli")]
use std::path::Path;

#[cfg(feature = "cli")]
use crate::errors::{MmrspaceError, Result};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CellSection {
    He,
    Ihc,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HeCell {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub cell_type: Option<String>,
    pub cell_type_probability: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IhcCell {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub mmr_mark: Option<u8>,
    pub mmr_probability: Option<f64>,
}

#[cfg(feature = "cli")]
#[derive(Clone, Debug, Deserialize)]
struct CellVitHeCell {
    cell_id: String,
    x_centroid_um: f64,
    y_centroid_um: f64,
    predicted_class: String,
    class_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FusedCell {
    pub source_section: CellSection,
    pub source_cell_id: String,
    pub x_um_registered: f64,
    pub y_um_registered: f64,
    pub mmr_mark: Option<u8>,
    pub mmr_probability: Option<f64>,
    pub cell_type: Option<String>,
    pub cell_type_probability: Option<f64>,
    pub same_section: bool,
    pub registration_error_um: Option<f64>,
    pub timepoint: String,
    pub case_id: String,
    pub protein: String,
}

#[cfg(feature = "cli")]
pub fn load_he_cell_table_csv(path: impl AsRef<Path>) -> Result<Vec<HeCell>> {
    let path = path.as_ref();
    let mut reader = csv::Reader::from_path(path)
        .map_err(|err| MmrspaceError::Schema(format!("failed to read H&E cell CSV: {err}")))?;
    let mut cells = Vec::new();
    for (index, row) in reader.deserialize().enumerate() {
        let row_number = index + 2;
        let cell: HeCell =
            row.map_err(|err| MmrspaceError::Schema(format!("invalid H&E cell row: {err}")))?;
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

#[cfg(feature = "cli")]
pub fn load_cellvit_he_cell_table_csv(
    path: impl AsRef<Path>,
    min_probability: f64,
) -> Result<Vec<HeCell>> {
    let path = path.as_ref();
    if !min_probability.is_finite() || !(0.0..=1.0).contains(&min_probability) {
        return Err(MmrspaceError::Config(
            "CellViT minimum probability must be in [0, 1]".into(),
        ));
    }

    let mut reader = csv::Reader::from_path(path).map_err(|err| {
        MmrspaceError::Schema(format!("failed to read CellViT H&E cell CSV: {err}"))
    })?;
    let mut cells = Vec::new();
    for (index, row) in reader.deserialize().enumerate() {
        let row_number = index + 2;
        let row: CellVitHeCell =
            row.map_err(|err| MmrspaceError::Schema(format!("invalid CellViT H&E row: {err}")))?;
        validate_cell_id(path, row_number, &row.cell_id)?;
        validate_xy(
            path,
            row_number,
            Some(&row.cell_id),
            row.x_centroid_um,
            row.y_centroid_um,
        )?;
        validate_probability(
            path,
            row_number,
            Some(&row.cell_id),
            Some(row.class_probability),
            "class_probability",
        )?;
        if row.class_probability < min_probability {
            return Err(validation_error(
                path,
                row_number,
                Some(&row.cell_id),
                "class_probability is below configured CellViT minimum probability",
            ));
        }
        let cell_type = normalize_cellvit_class(&row.predicted_class).ok_or_else(|| {
            validation_error(
                path,
                row_number,
                Some(&row.cell_id),
                "unknown CellViT predicted_class",
            )
        })?;
        cells.push(HeCell {
            cell_id: row.cell_id,
            x_um: row.x_centroid_um,
            y_um: row.y_centroid_um,
            cell_type: Some(cell_type),
            cell_type_probability: Some(row.class_probability),
        });
    }
    Ok(cells)
}

#[cfg(feature = "cli")]
fn normalize_cellvit_class(label: &str) -> Option<String> {
    let normalized = label.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "lymphocyte" | "t_lymphocyte" | "b_lymphocyte" | "immune_lymphocyte" => {
            Some("lymphocyte".into())
        }
        "stroma" | "stromal" | "stromal_cell" | "fibroblast" => Some("stroma".into()),
        "tumor" | "tumour" | "tumor_cell" | "tumor_epithelial" | "epithelial" => {
            Some("tumor".into())
        }
        "macrophage" | "histiocyte" => Some("macrophage".into()),
        "plasma_cell" | "plasma" => Some("plasma_cell".into()),
        "neutrophil" => Some("neutrophil".into()),
        "endothelial" | "endothelial_cell" => Some("endothelial".into()),
        _ => None,
    }
}

pub fn primary_label(cell: &FusedCell) -> Option<String> {
    match cell.source_section {
        CellSection::Ihc => ihc_mmr_label(cell),
        CellSection::He => cell
            .cell_type
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_owned),
    }
}

fn ihc_mmr_label(cell: &FusedCell) -> Option<String> {
    match cell.mmr_mark {
        Some(1) => Some("mmr_abnormal".into()),
        Some(0) => Some("mmr_retained".into()),
        _ => cell.mmr_probability.map(|probability| {
            if probability >= 0.5 {
                "mmr_abnormal".into()
            } else {
                "mmr_retained".into()
            }
        }),
    }
}

#[cfg(feature = "cli")]
pub fn load_ihc_cell_table_csv(path: impl AsRef<Path>) -> Result<Vec<IhcCell>> {
    let path = path.as_ref();
    let mut reader = csv::Reader::from_path(path)
        .map_err(|err| MmrspaceError::Schema(format!("failed to read IHC cell CSV: {err}")))?;
    let mut cells = Vec::new();
    for (index, row) in reader.deserialize().enumerate() {
        let row_number = index + 2;
        let cell: IhcCell =
            row.map_err(|err| MmrspaceError::Schema(format!("invalid IHC cell row: {err}")))?;
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

#[cfg(feature = "cli")]
fn validate_cell_id(path: &Path, row_number: usize, cell_id: &str) -> Result<()> {
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

#[cfg(feature = "cli")]
fn validate_xy(
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

#[cfg(feature = "cli")]
fn validate_probability(
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

#[cfg(feature = "cli")]
fn validation_error(
    path: &Path,
    row_number: usize,
    cell_id: Option<&str>,
    message: &str,
) -> MmrspaceError {
    let cell_id_context = match cell_id {
        Some(cell_id) if cell_id.trim().is_empty() => ", cell_id <blank>".to_string(),
        Some(cell_id) => format!(", cell_id {}", cell_id.trim()),
        None => String::new(),
    };
    MmrspaceError::Schema(format!(
        "{} row {}{}: {}",
        path.display(),
        row_number,
        cell_id_context,
        message
    ))
}
