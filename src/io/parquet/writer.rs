use std::{fs::File, path::Path, sync::Arc};

use arrow::{array::RecordBatch, datatypes::Schema};
use parquet::arrow::arrow_writer::ArrowWriter;

use crate::errors::{MarklabError, Result};

pub(super) fn write_record_batch(
    path: &Path,
    schema: Arc<Schema>,
    batch: &RecordBatch,
) -> Result<()> {
    let file = File::create(path).map_err(|source| MarklabError::io(path, source))?;
    let mut writer = ArrowWriter::try_new(file, schema, None)
        .map_err(|err| MarklabError::Schema(err.to_string()))?;
    writer
        .write(batch)
        .map_err(|err| MarklabError::Schema(err.to_string()))?;
    writer
        .close()
        .map_err(|err| MarklabError::Schema(err.to_string()))?;
    Ok(())
}
