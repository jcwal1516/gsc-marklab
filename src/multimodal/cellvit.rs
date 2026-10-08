use std::path::Path;

use serde::Deserialize;

use crate::errors::{MarklabError, Result};

use super::{
    cell_validation::{validate_cell_id, validate_probability, validate_xy, validation_error},
    cells::HeCell,
};

#[derive(Clone, Debug, Deserialize)]
struct CellVitHeCell {
    cell_id: String,
    x_centroid_um: f64,
    y_centroid_um: f64,
    predicted_class: String,
    class_probability: f64,
}

pub(crate) fn load_cellvit_he_cell_table_csv(
    path: impl AsRef<Path>,
    min_probability: f64,
) -> Result<Vec<HeCell>> {
    let path = path.as_ref();
    if !min_probability.is_finite() || !(0.0..=1.0).contains(&min_probability) {
        return Err(MarklabError::Config(
            "CellViT minimum probability must be in [0, 1]".into(),
        ));
    }

    let mut reader = csv::Reader::from_path(path).map_err(|err| {
        MarklabError::Schema(format!("failed to read CellViT H&E cell CSV: {err}"))
    })?;
    let mut cells = Vec::new();
    for (index, row) in reader.deserialize().enumerate() {
        let row_number = index + 2;
        let row: CellVitHeCell =
            row.map_err(|err| MarklabError::Schema(format!("invalid CellViT H&E row: {err}")))?;
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
        let cell_type = normalize_class(&row.predicted_class).ok_or_else(|| {
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
            cell_type: Some(cell_type.to_owned()),
            cell_type_probability: Some(row.class_probability),
        });
    }
    Ok(cells)
}

fn normalize_class(label: &str) -> Option<&'static str> {
    let normalized = label.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "lymphocyte" | "t_lymphocyte" | "b_lymphocyte" | "immune_lymphocyte" => Some("lymphocyte"),
        "stroma" | "stromal" | "stromal_cell" | "fibroblast" => Some("stroma"),
        "tumor" | "tumour" | "tumor_cell" | "tumor_epithelial" | "epithelial" => Some("tumor"),
        "macrophage" | "histiocyte" => Some("macrophage"),
        "plasma_cell" | "plasma" => Some("plasma_cell"),
        "neutrophil" => Some("neutrophil"),
        "endothelial" | "endothelial_cell" => Some("endothelial"),
        _ => None,
    }
}
