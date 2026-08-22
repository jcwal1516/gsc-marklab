use arrow::array::{Array, BooleanArray, Float32Array, StringArray, UInt16Array, UInt32Array};

use crate::io::row::{ArtifactFlags, DecodedCellRow, InternalControlState, NonviableFlags};

use super::schema::BatchColumns;

pub(super) fn decode_cell_row(columns: &BatchColumns<'_>, row: usize) -> DecodedCellRow {
    DecodedCellRow {
        x_um: columns.x.value(row),
        y_um: columns.y.value(row),
        mark: columns.mark.value(row),
        mark_probability: optional_f32(columns.mark_probability, row),
        tumor_probability: optional_f32(columns.tumor_probability, row),
        nucleus_area_um2: optional_f32(columns.nucleus_area_um2, row),
        case_id: columns.case_id.value(row).to_owned(),
        timepoint: columns.timepoint.value(row).to_owned(),
        protein: columns.protein.value(row).to_owned(),
        internal_control: columns
            .internal_control
            .map(|column| InternalControlState::from_optional(optional_string(Some(column), row))),
        slide_id: optional_string(columns.slide_id, row),
        section_id: optional_string(columns.section_id, row),
        stain_batch: optional_string(columns.stain_batch, row),
        block_id: optional_string(columns.block_id, row),
        region_id: optional_string(columns.region_id, row),
        slide_region: optional_string(columns.slide_region, row),
        histologic_compartment: optional_string(columns.histologic_compartment, row),
        valid_tumor: columns.valid_tumor.value(row),
        valid_ihc: columns.valid_ihc.value(row),
        artifact_flags: ArtifactFlags::new(
            columns.artifact.map(|column| bool_value(column, row)),
            columns.edge_artifact.map(|column| bool_value(column, row)),
            columns.fold_artifact.map(|column| bool_value(column, row)),
        ),
        nonviable_flags: NonviableFlags::new(
            columns.necrosis.map(|column| bool_value(column, row)),
            columns
                .nonviable_therapy_effect
                .map(|column| bool_value(column, row)),
        ),
        qc_bin: optional_u16(columns.qc_bin, row),
        component_id: optional_u32(columns.component_id, row),
        local_dab_od: optional_f32(columns.local_dab_od, row),
        local_hematoxylin_od: optional_f32(columns.local_hematoxylin_od, row),
    }
}

fn optional_string(column: Option<&StringArray>, row: usize) -> Option<String> {
    let column = column?;
    if column.is_null(row) {
        return None;
    }
    let value = column.value(row).trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn bool_value(column: &BooleanArray, row: usize) -> bool {
    !column.is_null(row) && column.value(row)
}

fn optional_f32(column: Option<&Float32Array>, row: usize) -> Option<f32> {
    column
        .filter(|column| !column.is_null(row))
        .map(|column| column.value(row))
}

fn optional_u16(column: Option<&UInt16Array>, row: usize) -> Option<u16> {
    column
        .filter(|column| !column.is_null(row))
        .map(|column| column.value(row))
}

fn optional_u32(column: Option<&UInt32Array>, row: usize) -> Option<u32> {
    column
        .filter(|column| !column.is_null(row))
        .map(|column| column.value(row))
}
