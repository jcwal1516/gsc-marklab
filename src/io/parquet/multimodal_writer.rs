use std::{path::Path, sync::Arc};

use arrow::{
    array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt64Array, UInt8Array},
    datatypes::{DataType, Field, Schema},
};

use crate::{
    errors::{MarklabError, Result},
    multimodal::cells::{CellSection, FusedCell},
    output::{CrossInteractionCurve, NeighborhoodEnrichmentResult},
};

use super::writer::write_record_batch;

pub fn write_fused_cells_parquet(
    cells: &[FusedCell],
    case_id: &str,
    timepoint: &str,
    protein: &str,
    path: impl AsRef<Path>,
) -> Result<()> {
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
            Arc::new(StringArray::from(vec![timepoint; cells.len()])),
            Arc::new(StringArray::from(vec![case_id; cells.len()])),
            Arc::new(StringArray::from(vec![protein; cells.len()])),
        ],
    )
    .map_err(|err| MarklabError::Schema(err.to_string()))?;
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
        Field::new("enrichment_ratio", DataType::Float64, true),
        Field::new("enrichment_ratio_unavailable_reason", DataType::Utf8, true),
        Field::new("z_score", DataType::Float64, true),
        Field::new("z_score_unavailable_reason", DataType::Utf8, true),
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
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| {
                        row.enrichment_ratio_unavailable_reason
                            .map(|reason| reason.as_str())
                    })
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.z_score).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.z_score_unavailable_reason.map(|reason| reason.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.p_value).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.q_value).collect::<Vec<_>>(),
            )),
        ],
    )
    .map_err(|err| MarklabError::Schema(err.to_string()))?;
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
        Field::new("value", DataType::Float64, true),
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
    .map_err(|err| MarklabError::Schema(err.to_string()))?;
    write_record_batch(path, schema, &batch)
}
