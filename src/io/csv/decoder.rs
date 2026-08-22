use std::path::Path;

use crate::errors::{MarklabError, Result};

use super::schema::CellRow;

pub(super) struct DecodedRows {
    pub rows: Vec<CellRow>,
    pub has_internal_control: bool,
    pub has_artifact_columns: bool,
    pub has_nonviable_columns: bool,
}

pub(super) fn read_rows(path: &Path) -> Result<DecodedRows> {
    let mut reader =
        ::csv::Reader::from_path(path).map_err(|error| MarklabError::io(path, error.into()))?;
    let headers = reader
        .headers()
        .map_err(|error| MarklabError::Schema(format!("{} headers: {error}", path.display())))?
        .clone();
    let has_column = |name| headers.iter().any(|header| header == name);
    let rows = reader
        .deserialize::<CellRow>()
        .enumerate()
        .map(|(index, row)| {
            row.map_err(|error| {
                MarklabError::Schema(format!("{} row {}: {error}", path.display(), index + 2))
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(DecodedRows {
        rows,
        has_internal_control: has_column("internal_control_local"),
        has_artifact_columns: ["artifact", "edge_artifact", "fold_artifact"]
            .iter()
            .any(|name| has_column(name)),
        has_nonviable_columns: ["necrosis", "nonviable_therapy_effect"]
            .iter()
            .any(|name| has_column(name)),
    })
}
