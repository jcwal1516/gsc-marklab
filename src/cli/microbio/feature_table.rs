//! Feature tables: one Parquet row per isolate or specimen, an `id` column and numeric feature
//! columns, with the modality, unit and provenance in the file's `marklab.feature_table`
//! metadata. Every modality reaches `fit-ast` and `predict-ast` through this one format.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{MarklabError, Result};

pub(super) const METADATA_KEY: &str = "marklab.feature_table";
const MAXIMUM_ID_BYTES: usize = 256;

/// What the features describe and how they were made.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FeatureTableMeta {
    pub version: u32,
    /// Such as `maldi_tof`.
    pub modality: String,
    pub unit: Unit,
    /// Whatever determines the features, such as preprocessing steps and bins. A model only
    /// accepts tables whose provenance equals its training table's.
    pub provenance: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Unit {
    Isolate,
    Specimen,
}

pub(super) struct FeatureTable {
    pub meta: FeatureTableMeta,
    pub columns: Vec<String>,
    pub ids: Vec<String>,
    /// Row-major, `ids.len() × columns.len()`.
    pub values: Vec<f64>,
}

pub(super) fn read_feature_table(path: &Path) -> Result<FeatureTable> {
    #[cfg(feature = "parquet")]
    return parquet_table::read(path);
    #[cfg(not(feature = "parquet"))]
    {
        let _ = path;
        bail!("feature tables require the parquet feature")
    }
}

pub(super) fn write_feature_table(path: &Path, table: &FeatureTable) -> Result<()> {
    #[cfg(feature = "parquet")]
    return parquet_table::write(path, table);
    #[cfg(not(feature = "parquet"))]
    {
        let _ = (path, table);
        bail!("feature tables require the parquet feature")
    }
}

pub(super) fn check_id(id: &str, row: usize) -> Result<()> {
    if id.is_empty() || id.len() > MAXIMUM_ID_BYTES {
        bail!("feature table row {row}: an id must be 1 to {MAXIMUM_ID_BYTES} bytes");
    }
    Ok(())
}

#[cfg(feature = "parquet")]
mod parquet_table {
    use std::{
        collections::{HashMap, HashSet},
        fs::{self, File, OpenOptions},
        path::Path,
        sync::Arc,
    };

    use arrow::{
        array::{Array, ArrayRef, Float64Array, RecordBatch, StringArray},
        compute::cast,
        datatypes::{DataType, Field, Schema},
    };
    use marklab_prediction::MAXIMUM_LOGISTIC_VALUES;
    use parquet::arrow::{arrow_reader::ParquetRecordBatchReaderBuilder, ArrowWriter};

    use super::{check_id, FeatureTable, FeatureTableMeta, METADATA_KEY};
    use crate::{MarklabError, Result};

    const MAXIMUM_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
    const BATCH_ROWS: usize = 1024;

    pub(super) fn read(path: &Path) -> Result<FeatureTable> {
        let io_error = |source| MarklabError::io(path, source);
        let invalid = |message: String| {
            MarklabError::Validation(format!("feature table {}: {message}", path.display()))
        };
        // Check before opening: opening a FIFO would block.
        if !fs::metadata(path).map_err(io_error)?.is_file() {
            return Err(invalid("not a regular file".to_owned()));
        }
        let file = File::open(path).map_err(io_error)?;
        if file.metadata().map_err(io_error)?.len() > MAXIMUM_FILE_BYTES {
            return Err(invalid("larger than 4 GiB".to_owned()));
        }
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|error| invalid(error.to_string()))?;
        let schema = builder.schema().clone();
        let meta: FeatureTableMeta = schema
            .metadata()
            .get(METADATA_KEY)
            .ok_or_else(|| invalid(format!("no `{METADATA_KEY}` metadata")))
            .and_then(|text| {
                serde_json::from_str(text)
                    .map_err(|error| invalid(format!("`{METADATA_KEY}` metadata: {error}")))
            })?;
        if meta.version != 1 || meta.modality.trim().is_empty() {
            return Err(invalid(
                "metadata must have version 1 and a non-empty modality".to_owned(),
            ));
        }
        let fields = schema.fields();
        let id_field = fields.first().filter(|field| field.name() == "id");
        let id_is_text = id_field.is_some_and(|field| {
            matches!(
                field.data_type(),
                DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
            )
        });
        if !id_is_text {
            return Err(invalid(
                "the first column must be a text column `id`".to_owned(),
            ));
        }
        let columns: Vec<String> = fields[1..].iter().map(|f| f.name().clone()).collect();
        if columns.is_empty() {
            return Err(invalid("there are no feature columns".to_owned()));
        }
        let mut seen = HashSet::new();
        for field in &fields[1..] {
            if !matches!(field.data_type(), DataType::Float64 | DataType::Float32) {
                return Err(invalid(format!(
                    "feature column `{}` must be Float64 or Float32",
                    field.name()
                )));
            }
            if !seen.insert(field.name().as_str()) {
                return Err(invalid(format!("column `{}` appears twice", field.name())));
            }
        }
        let rows = usize::try_from(builder.metadata().file_metadata().num_rows())
            .map_err(|_| invalid("negative row count".to_owned()))?;
        if rows == 0 || rows.saturating_mul(columns.len()) > MAXIMUM_LOGISTIC_VALUES {
            return Err(invalid(format!(
                "{rows} rows × {} features must be between 1 and {MAXIMUM_LOGISTIC_VALUES} values",
                columns.len()
            )));
        }
        let reader = builder
            .with_batch_size(BATCH_ROWS)
            .build()
            .map_err(|error| invalid(error.to_string()))?;
        let width = columns.len();
        let mut values = vec![0.0; rows * width];
        let mut ids = Vec::with_capacity(rows);
        for batch in reader {
            let batch = batch.map_err(|error| invalid(error.to_string()))?;
            let start = ids.len();
            if start + batch.num_rows() > rows {
                return Err(invalid(
                    "more rows than the file metadata declares".to_owned(),
                ));
            }
            let id_column = cast(batch.column(0), &DataType::Utf8)
                .map_err(|error| invalid(error.to_string()))?;
            let id_column = id_column
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("cast to Utf8 yields a StringArray");
            if id_column.null_count() > 0 {
                return Err(invalid("`id` has missing values".to_owned()));
            }
            for (offset, id) in id_column.iter().enumerate() {
                let id = id.expect("no nulls");
                check_id(id, start + offset + 1)?;
                ids.push(id.to_owned());
            }
            for (column, name) in columns.iter().enumerate() {
                let array = cast(batch.column(column + 1), &DataType::Float64)
                    .map_err(|error| invalid(error.to_string()))?;
                let array = array
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .expect("cast to Float64 yields a Float64Array");
                if array.null_count() > 0 {
                    return Err(invalid(format!("column `{name}` has missing values")));
                }
                for (offset, &value) in array.values().iter().enumerate() {
                    if !value.is_finite() {
                        return Err(invalid(format!(
                            "column `{name}` row {} is not finite",
                            start + offset + 1
                        )));
                    }
                    values[(start + offset) * width + column] = value;
                }
            }
        }
        if ids.len() != rows {
            return Err(invalid(
                "fewer rows than the file metadata declares".to_owned(),
            ));
        }
        let mut unique = HashSet::with_capacity(rows);
        if let Some(duplicate) = ids.iter().find(|id| !unique.insert(id.as_str())) {
            return Err(invalid(format!("id `{duplicate}` appears more than once")));
        }
        Ok(FeatureTable {
            meta,
            columns,
            ids,
            values,
        })
    }

    pub(super) fn write(path: &Path, table: &FeatureTable) -> Result<()> {
        let width = table.columns.len();
        let mut fields = vec![Field::new("id", DataType::Utf8, false)];
        fields.extend(
            table
                .columns
                .iter()
                .map(|name| Field::new(name, DataType::Float64, false)),
        );
        let metadata =
            HashMap::from([(METADATA_KEY.to_owned(), serde_json::to_string(&table.meta)?)]);
        let schema = Arc::new(Schema::new(fields).with_metadata(metadata));
        let mut arrays: Vec<ArrayRef> = vec![Arc::new(StringArray::from(table.ids.clone()))];
        for column in 0..width {
            let values: Vec<f64> = table
                .values
                .chunks_exact(width)
                .map(|row| row[column])
                .collect();
            arrays.push(Arc::new(Float64Array::from(values)));
        }
        let schema_error =
            |error: arrow::error::ArrowError| MarklabError::Schema(error.to_string());
        let batch = RecordBatch::try_new(schema.clone(), arrays).map_err(schema_error)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| MarklabError::io(path, source))?;
        let parquet_error =
            |error: parquet::errors::ParquetError| MarklabError::Schema(error.to_string());
        let mut writer = ArrowWriter::try_new(file, schema, None).map_err(parquet_error)?;
        writer.write(&batch).map_err(parquet_error)?;
        writer.close().map_err(parquet_error)?;
        Ok(())
    }
}
