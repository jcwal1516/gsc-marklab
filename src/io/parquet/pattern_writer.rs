use std::{collections::HashMap, path::Path, sync::Arc};

use arrow::{
    array::{
        Array, BooleanArray, Float32Array, Float64Array, RecordBatch, StringArray, UInt16Array,
        UInt32Array, UInt8Array,
    },
    datatypes::{DataType, Field, Schema},
};

use crate::{
    data::Pattern,
    errors::{MarklabError, Result},
    io::{checked_finite, checked_positive, checked_probability},
};

use super::writer::write_record_batch;

/// Writes the retained cells in a `Pattern` as a canonical filtered export.
///
/// Every written row is retained. Source rows excluded during ingestion and
/// unavailable per-row QC states are not recoverable from `Pattern` and are
/// therefore omitted rather than fabricated. This is not an input round trip.
pub fn write_filtered_pattern_export_parquet(
    pattern: &Pattern,
    path: impl AsRef<Path>,
) -> Result<()> {
    let path = path.as_ref();
    let n = pattern.len();
    let mark_probability_values = optional_metric_values(
        pattern.mark_prob.as_deref(),
        n,
        "mark_probability",
        checked_probability,
    )?;
    let tumor_probability_values = optional_metric_values(
        pattern.tumor_probability.as_deref(),
        n,
        "tumor_probability",
        checked_probability,
    )?;
    let nucleus_area_values = optional_metric_values(
        pattern.nucleus_area_um2.as_deref(),
        n,
        "nucleus_area_um2",
        checked_positive,
    )?;
    let local_dab_values = optional_metric_values(
        pattern.local_dab_od.as_deref(),
        n,
        "local_dab_od",
        checked_finite,
    )?;
    let local_hematoxylin_values = optional_metric_values(
        pattern.local_hematoxylin_od.as_deref(),
        n,
        "local_hematoxylin_od",
        checked_finite,
    )?;

    let mut fields = vec![
        Field::new("x_um", DataType::Float64, false),
        Field::new("y_um", DataType::Float64, false),
        Field::new("mark", DataType::UInt8, false),
        Field::new("mark_probability", DataType::Float32, true),
        Field::new("tumor_probability", DataType::Float32, true),
        Field::new("nucleus_area_um2", DataType::Float32, true),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
        Field::new("slide_id", DataType::Utf8, true),
        Field::new("section_id", DataType::Utf8, true),
        Field::new("stain_batch", DataType::Utf8, true),
        Field::new("block_id", DataType::Utf8, true),
        Field::new("region_id", DataType::Utf8, true),
        Field::new("valid_tumor", DataType::Boolean, false),
        Field::new("valid_ihc", DataType::Boolean, false),
    ];
    let mut columns: Vec<Arc<dyn Array>> = vec![
        Arc::new(Float64Array::from(pattern.x_um.to_vec())),
        Arc::new(Float64Array::from(pattern.y_um.to_vec())),
        Arc::new(UInt8Array::from(pattern.mark.to_vec())),
        Arc::new(Float32Array::from(mark_probability_values)),
        Arc::new(Float32Array::from(tumor_probability_values)),
        Arc::new(Float32Array::from(nucleus_area_values)),
        Arc::new(StringArray::from(vec![pattern.meta.case_id.as_str(); n])),
        Arc::new(StringArray::from(vec![pattern.meta.timepoint.as_str(); n])),
        Arc::new(StringArray::from(vec![pattern.meta.protein.as_str(); n])),
        Arc::new(StringArray::from(vec![pattern.meta.slide_id.as_deref(); n])),
        Arc::new(StringArray::from(vec![
            pattern.meta.section_id.as_deref();
            n
        ])),
        Arc::new(StringArray::from(vec![
            pattern.meta.stain_batch.as_deref();
            n
        ])),
        Arc::new(StringArray::from(vec![pattern.meta.block_id.as_deref(); n])),
        Arc::new(StringArray::from(vec![
            pattern.meta.region_id.as_deref();
            n
        ])),
        Arc::new(BooleanArray::from(vec![true; n])),
        Arc::new(BooleanArray::from(vec![true; n])),
    ];
    if let Some(values) = optional_dense_values(pattern.qc_bin.as_deref(), n, "qc_bin")? {
        fields.push(Field::new("qc_bin", DataType::UInt16, false));
        columns.push(Arc::new(UInt16Array::from(values)));
    }
    if let Some(values) = optional_dense_values(pattern.component_id.as_deref(), n, "component_id")?
    {
        fields.push(Field::new("component_id", DataType::UInt32, false));
        columns.push(Arc::new(UInt32Array::from(values)));
    }
    fields.push(Field::new("local_dab_od", DataType::Float32, true));
    columns.push(Arc::new(Float32Array::from(local_dab_values)));
    fields.push(Field::new("local_hematoxylin_od", DataType::Float32, true));
    columns.push(Arc::new(Float32Array::from(local_hematoxylin_values)));

    let metadata = HashMap::from([
        (
            "marklab.export_kind".to_owned(),
            "filtered_canonical_pattern".to_owned(),
        ),
        (
            "marklab.export_semantics".to_owned(),
            "retained cells only; excluded source rows and unavailable per-row QC states are absent"
                .to_owned(),
        ),
    ]);
    let schema = Arc::new(Schema::new(fields).with_metadata(metadata));
    let batch = RecordBatch::try_new(Arc::clone(&schema), columns)
        .map_err(|error| MarklabError::Schema(error.to_string()))?;
    write_record_batch(path, schema, &batch)
}
fn optional_metric_values(
    values: Option<&[f32]>,
    n: usize,
    column: &str,
    validate: fn(f32, &str) -> Result<f32>,
) -> Result<Vec<Option<f32>>> {
    let Some(values) = values else {
        return Ok(vec![None; n]);
    };
    if values.len() != n {
        return Err(MarklabError::Schema(format!(
            "{column} length must match pattern length"
        )));
    }
    values
        .iter()
        .copied()
        .map(|value| validate(value, column).map(Some))
        .collect()
}

fn optional_dense_values<T: Copy>(
    values: Option<&[T]>,
    n: usize,
    column: &str,
) -> Result<Option<Vec<T>>> {
    let Some(values) = values else {
        return Ok(None);
    };
    if values.len() != n {
        return Err(MarklabError::Schema(format!(
            "{column} length must match pattern length"
        )));
    }
    Ok(Some(values.to_vec()))
}
