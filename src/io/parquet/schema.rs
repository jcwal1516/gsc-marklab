use arrow::array::{
    Array, BooleanArray, Float32Array, Float64Array, RecordBatch, StringArray, UInt16Array,
    UInt32Array, UInt8Array,
};

use crate::errors::{MarklabError, Result};

pub(super) struct BatchColumns<'a> {
    pub x: &'a Float64Array,
    pub y: &'a Float64Array,
    pub mark: &'a UInt8Array,
    pub mark_probability: Option<&'a Float32Array>,
    pub tumor_probability: Option<&'a Float32Array>,
    pub nucleus_area_um2: Option<&'a Float32Array>,
    pub case_id: &'a StringArray,
    pub timepoint: &'a StringArray,
    pub protein: &'a StringArray,
    pub internal_control: Option<&'a StringArray>,
    pub slide_id: Option<&'a StringArray>,
    pub section_id: Option<&'a StringArray>,
    pub stain_batch: Option<&'a StringArray>,
    pub block_id: Option<&'a StringArray>,
    pub slide_region: Option<&'a StringArray>,
    pub histologic_compartment: Option<&'a StringArray>,
    pub region_id: Option<&'a StringArray>,
    pub valid_tumor: &'a BooleanArray,
    pub valid_ihc: &'a BooleanArray,
    pub artifact: Option<&'a BooleanArray>,
    pub edge_artifact: Option<&'a BooleanArray>,
    pub fold_artifact: Option<&'a BooleanArray>,
    pub necrosis: Option<&'a BooleanArray>,
    pub nonviable_therapy_effect: Option<&'a BooleanArray>,
    pub qc_bin: Option<&'a UInt16Array>,
    pub component_id: Option<&'a UInt32Array>,
    pub local_dab_od: Option<&'a Float32Array>,
    pub local_hematoxylin_od: Option<&'a Float32Array>,
}

impl<'a> BatchColumns<'a> {
    pub(super) fn try_new(batch: &'a RecordBatch) -> Result<Self> {
        Ok(Self {
            x: required(batch, "x_um")?,
            y: required(batch, "y_um")?,
            mark: required(batch, "mark")?,
            mark_probability: optional(batch, "mark_probability")?,
            tumor_probability: optional(batch, "tumor_probability")?,
            nucleus_area_um2: optional(batch, "nucleus_area_um2")?,
            case_id: required(batch, "case_id")?,
            timepoint: required(batch, "timepoint")?,
            protein: required(batch, "protein")?,
            internal_control: optional(batch, "internal_control_local")?,
            slide_id: optional(batch, "slide_id")?,
            section_id: optional(batch, "section_id")?,
            stain_batch: optional(batch, "stain_batch")?,
            block_id: optional(batch, "block_id")?,
            slide_region: optional(batch, "slide_region")?,
            histologic_compartment: optional(batch, "histologic_compartment")?,
            region_id: optional(batch, "region_id")?,
            valid_tumor: required(batch, "valid_tumor")?,
            valid_ihc: required(batch, "valid_ihc")?,
            artifact: optional(batch, "artifact")?,
            edge_artifact: optional(batch, "edge_artifact")?,
            fold_artifact: optional(batch, "fold_artifact")?,
            necrosis: optional(batch, "necrosis")?,
            nonviable_therapy_effect: optional(batch, "nonviable_therapy_effect")?,
            qc_bin: optional(batch, "qc_bin")?,
            component_id: optional(batch, "component_id")?,
            local_dab_od: optional(batch, "local_dab_od")?,
            local_hematoxylin_od: optional(batch, "local_hematoxylin_od")?,
        })
    }
}

fn required<'a, T: Array + 'static>(batch: &'a RecordBatch, name: &str) -> Result<&'a T> {
    let index = batch
        .schema()
        .index_of(name)
        .map_err(|error| MarklabError::Schema(error.to_string()))?;
    batch
        .column(index)
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| MarklabError::Schema(format!("column {name} has unexpected type")))
}

fn optional<'a, T: Array + 'static>(batch: &'a RecordBatch, name: &str) -> Result<Option<&'a T>> {
    let Ok(index) = batch.schema().index_of(name) else {
        return Ok(None);
    };
    batch
        .column(index)
        .as_any()
        .downcast_ref::<T>()
        .map(Some)
        .ok_or_else(|| MarklabError::Schema(format!("column {name} has unexpected type")))
}
