use std::path::Path;

use crate::errors::{MmrspaceError, Result};

use super::schema::CellRow;

pub(super) fn read_rows(path: &Path) -> Result<Vec<CellRow>> {
    let mut reader =
        ::csv::Reader::from_path(path).map_err(|error| MmrspaceError::io(path, error.into()))?;
    reader
        .deserialize::<CellRow>()
        .enumerate()
        .map(|(index, row)| {
            row.map_err(|error| {
                MmrspaceError::Schema(format!("{} row {}: {error}", path.display(), index + 2))
            })
        })
        .collect()
}
