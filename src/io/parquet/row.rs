use arrow::array::{Array, BooleanArray, Float32Array, StringArray, UInt16Array, UInt32Array};

use crate::errors::{MarklabError, Result};
use crate::io::{checked_finite, checked_positive, checked_probability};

use super::schema::BatchColumns;

pub(super) struct DecodedRow {
    pub x_um: f64,
    pub y_um: f64,
    pub mark: u8,
    pub mark_probability: Option<f32>,
    pub tumor_probability: Option<f32>,
    pub nucleus_area_um2: Option<f32>,
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub internal_control: Option<String>,
    pub slide_id: Option<String>,
    pub section_id: Option<String>,
    pub stain_batch: Option<String>,
    pub block_id: Option<String>,
    pub slide_region: Option<String>,
    pub histologic_compartment: Option<String>,
    pub region_id: Option<String>,
    pub valid_tumor: bool,
    pub valid_ihc: bool,
    pub artifact_excluded: bool,
    pub nonviable_excluded: bool,
    pub qc_bin: Option<u16>,
    pub component_id: Option<u32>,
    pub local_dab_od: Option<f32>,
    pub local_hematoxylin_od: Option<f32>,
}

impl DecodedRow {
    pub(super) fn decode(columns: &BatchColumns<'_>, row: usize) -> Result<Self> {
        let x_um = columns.x.value(row);
        let y_um = columns.y.value(row);
        if !x_um.is_finite() || !y_um.is_finite() {
            return Err(MarklabError::Schema(format!(
                "Parquet row {} has non-finite coordinates",
                row + 1
            )));
        }
        let mark = columns.mark.value(row);
        if mark > 1 {
            return Err(MarklabError::Schema(format!(
                "Parquet row {} mark must be 0 or 1",
                row + 1
            )));
        }

        Ok(Self {
            x_um,
            y_um,
            mark,
            mark_probability: optional_f32(columns.mark_probability, row)
                .map(|value| checked_probability(value, "mark_probability"))
                .transpose()?,
            tumor_probability: optional_f32(columns.tumor_probability, row)
                .map(|value| checked_probability(value, "tumor_probability"))
                .transpose()?,
            nucleus_area_um2: optional_f32(columns.nucleus_area_um2, row)
                .map(|value| checked_positive(value, "nucleus_area_um2"))
                .transpose()?,
            case_id: columns.case_id.value(row).to_owned(),
            timepoint: columns.timepoint.value(row).to_owned(),
            protein: columns.protein.value(row).to_owned(),
            internal_control: optional_string(columns.internal_control, row),
            slide_id: optional_string(columns.slide_id, row),
            section_id: optional_string(columns.section_id, row),
            stain_batch: optional_string(columns.stain_batch, row),
            block_id: optional_string(columns.block_id, row),
            slide_region: optional_string(columns.slide_region, row),
            histologic_compartment: optional_string(columns.histologic_compartment, row),
            region_id: optional_string(columns.region_id, row),
            valid_tumor: columns.valid_tumor.value(row),
            valid_ihc: columns.valid_ihc.value(row),
            artifact_excluded: optional_bool(columns.artifact, row)
                || optional_bool(columns.edge_artifact, row)
                || optional_bool(columns.fold_artifact, row),
            nonviable_excluded: optional_bool(columns.necrosis, row)
                || optional_bool(columns.nonviable_therapy_effect, row),
            qc_bin: optional_u16(columns.qc_bin, row),
            component_id: optional_u32(columns.component_id, row),
            local_dab_od: optional_f32(columns.local_dab_od, row)
                .map(|value| checked_finite(value, "local_dab_od"))
                .transpose()?,
            local_hematoxylin_od: optional_f32(columns.local_hematoxylin_od, row)
                .map(|value| checked_finite(value, "local_hematoxylin_od"))
                .transpose()?,
        })
    }

    pub(super) fn internal_control_is_valid(&self) -> bool {
        self.internal_control
            .as_deref()
            .is_none_or(|value| value.eq_ignore_ascii_case("valid"))
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

fn optional_bool(column: Option<&BooleanArray>, row: usize) -> bool {
    column.is_some_and(|column| !column.is_null(row) && column.value(row))
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
