use std::{fs::File, path::Path, sync::Arc};

use arrow::{
    array::{
        BooleanArray, Float32Array, Float64Array, RecordBatch, StringArray, UInt16Array,
        UInt32Array, UInt64Array, UInt8Array,
    },
    datatypes::{DataType, Field, Schema},
};
use parquet::arrow::arrow_writer::ArrowWriter;

use crate::{
    data::Pattern,
    errors::{MmrspaceError, Result},
    multimodal::cell_table::{CellSection, FusedCell},
    output::{CrossInteractionCurve, NeighborhoodEnrichmentResult},
};

mod loader;
mod row;
mod schema;
pub use loader::load_pattern_parquet_with_diagnostics;

pub fn write_fused_cells_parquet(cells: &[FusedCell], path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let schema = Arc::new(Schema::new(vec![
        Field::new("source_section", DataType::Utf8, false),
        Field::new("source_cell_id", DataType::Utf8, false),
        Field::new("x_um_registered", DataType::Float64, false),
        Field::new("y_um_registered", DataType::Float64, false),
        Field::new("mmr_mark", DataType::UInt8, true),
        Field::new("mmr_probability", DataType::Float64, true),
        Field::new("cell_type", DataType::Utf8, true),
        Field::new("cell_type_probability", DataType::Float64, true),
        Field::new("same_section", DataType::Boolean, false),
        Field::new("registration_error_um", DataType::Float64, true),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(StringArray::from(
                cells
                    .iter()
                    .map(|cell| match cell.source_section {
                        CellSection::He => "he",
                        CellSection::Ihc => "ihc",
                    })
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                cells
                    .iter()
                    .map(|cell| cell.source_cell_id.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                cells
                    .iter()
                    .map(|cell| cell.x_um_registered)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                cells
                    .iter()
                    .map(|cell| cell.y_um_registered)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt8Array::from(
                cells.iter().map(|cell| cell.mmr_mark).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                cells
                    .iter()
                    .map(|cell| cell.mmr_probability)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                cells
                    .iter()
                    .map(|cell| cell.cell_type.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                cells
                    .iter()
                    .map(|cell| cell.cell_type_probability)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(BooleanArray::from(
                cells
                    .iter()
                    .map(|cell| cell.same_section)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                cells
                    .iter()
                    .map(|cell| cell.registration_error_um)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                cells
                    .iter()
                    .map(|cell| cell.timepoint.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                cells
                    .iter()
                    .map(|cell| cell.case_id.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                cells
                    .iter()
                    .map(|cell| cell.protein.as_str())
                    .collect::<Vec<_>>(),
            )),
        ],
    )
    .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    write_record_batch(path, schema, &batch)
}

pub fn write_neighborhood_enrichment_parquet(
    rows: &[NeighborhoodEnrichmentResult],
    path: impl AsRef<Path>,
) -> Result<()> {
    let path = path.as_ref();
    let schema = Arc::new(Schema::new(vec![
        Field::new("label_a", DataType::Utf8, false),
        Field::new("label_b", DataType::Utf8, false),
        Field::new("observed_edges", DataType::UInt64, false),
        Field::new("expected_edges", DataType::Float64, false),
        Field::new("enrichment_ratio", DataType::Float64, false),
        Field::new("z_score", DataType::Float64, false),
        Field::new("p_value", DataType::Float64, true),
        Field::new("q_value", DataType::Float64, true),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.label_a.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.label_b.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.observed_edges as u64)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter()
                    .map(|row| row.expected_edges)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter()
                    .map(|row| row.enrichment_ratio)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.z_score).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.p_value).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.q_value).collect::<Vec<_>>(),
            )),
        ],
    )
    .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    write_record_batch(path, schema, &batch)
}

pub fn write_cross_interaction_curves_parquet(
    curves: &[CrossInteractionCurve],
    path: impl AsRef<Path>,
) -> Result<()> {
    let path = path.as_ref();
    let mut label_a = Vec::new();
    let mut label_b = Vec::new();
    let mut r_min_um = Vec::new();
    let mut r_max_um = Vec::new();
    let mut value = Vec::new();
    let mut lower_global_envelope = Vec::new();
    let mut upper_global_envelope = Vec::new();
    let mut count = Vec::new();
    let mut p_global = Vec::new();

    for curve in curves {
        for point in &curve.points {
            label_a.push(curve.label_a.as_str());
            label_b.push(curve.label_b.as_str());
            r_min_um.push(point.r_min_um);
            r_max_um.push(point.r_max_um);
            value.push(point.value);
            lower_global_envelope.push(point.lower_global_envelope);
            upper_global_envelope.push(point.upper_global_envelope);
            count.push(point.count as u64);
            p_global.push(curve.p_global);
        }
    }

    let schema = Arc::new(Schema::new(vec![
        Field::new("label_a", DataType::Utf8, false),
        Field::new("label_b", DataType::Utf8, false),
        Field::new("r_min_um", DataType::Float64, false),
        Field::new("r_max_um", DataType::Float64, false),
        Field::new("value", DataType::Float64, false),
        Field::new("lower_global_envelope", DataType::Float64, true),
        Field::new("upper_global_envelope", DataType::Float64, true),
        Field::new("count", DataType::UInt64, false),
        Field::new("p_global", DataType::Float64, true),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(StringArray::from(label_a)),
            Arc::new(StringArray::from(label_b)),
            Arc::new(Float64Array::from(r_min_um)),
            Arc::new(Float64Array::from(r_max_um)),
            Arc::new(Float64Array::from(value)),
            Arc::new(Float64Array::from(lower_global_envelope)),
            Arc::new(Float64Array::from(upper_global_envelope)),
            Arc::new(UInt64Array::from(count)),
            Arc::new(Float64Array::from(p_global)),
        ],
    )
    .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    write_record_batch(path, schema, &batch)
}

pub fn write_pattern_parquet(pattern: &Pattern, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let schema = Arc::new(required_schema());
    let n = pattern.len();
    let tumor_probability_values = optional_metric_values(
        pattern.tumor_probability.as_deref(),
        n,
        "tumor_probability",
        super::checked_probability,
    )?;
    let nucleus_area_values = optional_metric_values(
        pattern.nucleus_area_um2.as_deref(),
        n,
        "nucleus_area_um2",
        super::checked_positive,
    )?;
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(pattern.x_um.to_vec())),
            Arc::new(Float64Array::from(pattern.y_um.to_vec())),
            Arc::new(UInt8Array::from(pattern.mark.to_vec())),
            Arc::new(Float32Array::from(
                pattern
                    .mark_prob
                    .as_ref()
                    .map(|values| values.iter().copied().map(Some).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![None; n]),
            )),
            Arc::new(Float32Array::from(tumor_probability_values)),
            Arc::new(Float32Array::from(nucleus_area_values)),
            Arc::new(StringArray::from(vec![pattern.meta.case_id.as_str(); n])),
            Arc::new(StringArray::from(vec![pattern.meta.timepoint.as_str(); n])),
            Arc::new(StringArray::from(vec![pattern.meta.protein.as_str(); n])),
            Arc::new(StringArray::from(vec!["valid"; n])),
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
            Arc::new(BooleanArray::from(vec![false; n])),
            Arc::new(BooleanArray::from(vec![false; n])),
            Arc::new(BooleanArray::from(vec![false; n])),
            Arc::new(BooleanArray::from(vec![false; n])),
            Arc::new(BooleanArray::from(vec![false; n])),
            Arc::new(UInt16Array::from(
                pattern
                    .qc_bin
                    .as_ref()
                    .map(|values| values.to_vec())
                    .unwrap_or_else(|| vec![0; n]),
            )),
            Arc::new(UInt32Array::from(
                pattern
                    .component_id
                    .as_ref()
                    .map(|values| values.to_vec())
                    .unwrap_or_else(|| vec![0; n]),
            )),
            Arc::new(Float32Array::from(
                pattern
                    .local_dab_od
                    .as_ref()
                    .map(|values| values.iter().copied().map(Some).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![None; n]),
            )),
            Arc::new(Float32Array::from(
                pattern
                    .local_hematoxylin_od
                    .as_ref()
                    .map(|values| values.iter().copied().map(Some).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![None; n]),
            )),
        ],
    )
    .map_err(|err| MmrspaceError::Schema(err.to_string()))?;

    let file = File::create(path).map_err(|source| MmrspaceError::io(path, source))?;
    let mut writer = ArrowWriter::try_new(file, schema, None)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    writer
        .write(&batch)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    writer
        .close()
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;

    Ok(())
}

fn write_record_batch(path: &Path, schema: Arc<Schema>, batch: &RecordBatch) -> Result<()> {
    let file = File::create(path).map_err(|source| MmrspaceError::io(path, source))?;
    let mut writer = ArrowWriter::try_new(file, schema, None)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    writer
        .write(batch)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    writer
        .close()
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    Ok(())
}

fn required_schema() -> Schema {
    Schema::new(vec![
        Field::new("x_um", DataType::Float64, false),
        Field::new("y_um", DataType::Float64, false),
        Field::new("mark", DataType::UInt8, false),
        Field::new("mark_probability", DataType::Float32, true),
        Field::new("tumor_probability", DataType::Float32, true),
        Field::new("nucleus_area_um2", DataType::Float32, true),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
        Field::new("internal_control_local", DataType::Utf8, true),
        Field::new("slide_id", DataType::Utf8, true),
        Field::new("section_id", DataType::Utf8, true),
        Field::new("stain_batch", DataType::Utf8, true),
        Field::new("block_id", DataType::Utf8, true),
        Field::new("region_id", DataType::Utf8, true),
        Field::new("valid_tumor", DataType::Boolean, false),
        Field::new("valid_ihc", DataType::Boolean, false),
        Field::new("artifact", DataType::Boolean, true),
        Field::new("edge_artifact", DataType::Boolean, true),
        Field::new("fold_artifact", DataType::Boolean, true),
        Field::new("necrosis", DataType::Boolean, true),
        Field::new("nonviable_therapy_effect", DataType::Boolean, true),
        Field::new("qc_bin", DataType::UInt16, false),
        Field::new("component_id", DataType::UInt32, false),
        Field::new("local_dab_od", DataType::Float32, true),
        Field::new("local_hematoxylin_od", DataType::Float32, true),
    ])
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
        return Err(MmrspaceError::Schema(format!(
            "{column} length must match pattern length"
        )));
    }
    values
        .iter()
        .copied()
        .map(|value| validate(value, column).map(Some))
        .collect()
}
